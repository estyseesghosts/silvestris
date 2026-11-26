//! Icon grid (#35): the third presentation in Open and Save panels.
//!
//! The grid renders an app-owned [`ListController<T>`] in fixed cells, with
//! the same selection rules as `TableView`: click, Shift-click extends,
//! Command/Control-click toggles, arrows move, Return activates and a
//! double-click activates. A press on the empty background starts a marquee;
//! cells it intersects become the selection, computed from the virtualized
//! cell pitch and the list's scroll offset. Rows of cells are virtualised with
//! `uniform_list`, so a 30,000-entry directory builds only the rows in view.
//!
//! The control does not read files or own a location. The consumer supplies
//! the icon and label closures; the panel uses the same full-colour 16 px icon
//! vocabulary as its list and sidebar. `set_draggable` offers an
//! [`IconGridDrag`] per cell without deciding what a drop means.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, div, point, prelude::*, px, size, uniform_list, AnyElement, App, Bounds, Context,
    Entity, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, ScrollStrategy, SharedString, Subscription,
    UniformListScrollHandle, Window,
};

use super::scroll_source::ScrollSource as _;
use super::scroll_view::{ScrollAxes, ScrollView};
use super::text_field::{TextField, TextFieldEvent};
use crate::binding::{ItemId, ListController};
use crate::convert::hsla;
use crate::{paint, ActiveTheme};

/// Icon-view cell pitch, measured from the 10.6 icon-view Save sheet
/// (`dialogs--save-sheet--icon-view.png`): icons start 184 px apart in a 349 px
/// browser, so two columns overlap the edges rather than sitting on a strict
/// grid. Silvestris keeps a whole number of cells across the browser, so the
/// pitch is narrowed to 172 px — still two columns in the same browser, within
/// 12 px of the measured pitch. The Finder reference uses the same artwork on a
/// 137 px pitch in a full window.
pub const CELL_WIDTH: f32 = 172.0;
/// Icon, two label lines and their spacing: the panel reference puts the next
/// row's artwork 109 px below the previous row's.
pub const CELL_HEIGHT: f32 = 109.0;
/// Full-colour file icon in an icon-view cell. 10.6 artwork is 48 px tall in
/// both the Finder and the panel icon view.
pub const ICON_SIZE: f32 = 48.0;
/// Leading and top gutter before the first logical cell, as the panel shows
/// (its first icon column starts 59 px into the browser).
pub const GRID_GUTTER: f32 = 8.0;

type IconFn<T> = Rc<dyn Fn(&T, bool, bool, &mut Window, &mut App) -> AnyElement>;
type LabelFn<T> = Rc<dyn Fn(&T) -> SharedString>;
type RenameFn<T> = Rc<dyn Fn(&T, &str, &mut App) -> bool>;
/// Marquee start and current pointer, in window coordinates.
type Marquee = Rc<Cell<Option<(Point<Pixels>, Point<Pixels>)>>>;

struct Editing {
    id: ItemId,
    field: Entity<TextField>,
    _subscription: Subscription,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconGridEvent {
    /// A user click, Shift-click, toggle or keyboard move changed the
    /// selection. Programmatic selection never emits this.
    SelectionChanged,
    /// Double-click or Return on the selection.
    Activated(Vec<ItemId>),
    /// Secondary click on a cell.
    ContextMenu {
        ids: Vec<ItemId>,
        position: Point<Pixels>,
    },
    /// Secondary click on the empty area.
    BackgroundContextMenu { position: Point<Pixels> },
}

/// The value a draggable cell offers to GPUI's drag system. A consumer matches
/// it with `on_drag_move::<IconGridDrag>` on a drop target and reads `ids` to
/// resolve its own payload; the grid never decides move, copy or alias.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconGridDrag {
    /// The cell the gesture started on.
    pub source: ItemId,
    /// The selection at drag start, or just `source` when it was not selected.
    pub ids: Vec<ItemId>,
}

pub struct IconGrid<T> {
    controller: Entity<ListController<T>>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    icon: IconFn<T>,
    label: LabelFn<T>,
    rename: Option<RenameFn<T>>,
    editing: Option<Editing>,
    multiple: bool,
    /// Whether cells offer an [`IconGridDrag`] to GPUI.
    draggable: bool,
    /// Columns in the last frame; keyboard movement needs the same pitch.
    columns: Rc<Cell<usize>>,
    /// Marquee start and current pointer, in window coordinates.
    marquee: Marquee,
}

impl<T: 'static> IconGrid<T> {
    pub fn new(
        controller: Entity<ListController<T>>,
        icon: impl Fn(&T, bool, bool, &mut Window, &mut App) -> AnyElement + 'static,
        label: impl Fn(&T) -> SharedString + 'static,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            controller,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
            icon: Rc::new(icon),
            label: Rc::new(label),
            rename: None,
            editing: None,
            multiple: false,
            draggable: false,
            columns: Rc::new(Cell::new(4)),
            marquee: Rc::new(Cell::new(None)),
        }
    }

    /// Let cells offer [`IconGridDrag`] values to GPUI. The consumer decides
    /// which drop targets exist and what a drop means.
    pub fn set_draggable(&mut self, draggable: bool, cx: &mut Context<Self>) {
        self.draggable = draggable;
        cx.notify();
    }

    /// Call before the first render; renaming uses the same provider contract
    /// as `TableView`.
    pub fn set_rename(
        &mut self,
        rename: impl Fn(&T, &str, &mut App) -> bool + 'static,
        cx: &mut Context<Self>,
    ) {
        self.rename = Some(Rc::new(rename));
        cx.notify();
    }

    /// Begin editing a cell's label. Returns false when the item is not
    /// visible or the grid has no rename callback.
    pub fn begin_editing(
        &mut self,
        id: ItemId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.rename.is_none() {
            return false;
        }
        let label = {
            let controller = self.controller.read(cx);
            let Some(item) = controller
                .visible()
                .find(|item| controller.id_of(item) == id)
            else {
                return false;
            };
            (self.label)(item)
        };
        let field = cx.new(|cx| {
            let mut field = TextField::new(window, cx);
            field.set_text(label.to_string(), cx);
            field
        });
        field.update(cx, |field, cx| window.focus(&field.focus_handle(cx), cx));
        let subscription = cx.subscribe_in(
            &field,
            window,
            |grid, _, event: &TextFieldEvent, window, cx| {
                if matches!(
                    event,
                    TextFieldEvent::Submitted | TextFieldEvent::EndedEditing
                ) {
                    grid.finish_editing(true, window, cx);
                }
            },
        );
        self.editing = Some(Editing {
            id,
            field,
            _subscription: subscription,
        });
        cx.notify();
        true
    }

    /// End an edit, committing through the rename callback or discarding it.
    pub fn finish_editing(&mut self, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.take() else {
            return;
        };
        if commit {
            let text = editing.field.read(cx).text().to_string();
            let rename = self.rename.clone();
            let controller = self.controller.clone();
            if let Some(rename) = rename {
                let found = {
                    let controller = controller.read(cx);
                    controller
                        .visible()
                        .any(|item| controller.id_of(item) == editing.id)
                };
                if found {
                    controller.update(cx, |controller, cx| {
                        if let Some(item) = controller
                            .visible()
                            .find(|item| controller.id_of(item) == editing.id)
                        {
                            rename(item, &text, cx);
                        }
                    });
                }
            }
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    #[must_use]
    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    pub fn set_multiple(&mut self, multiple: bool, cx: &mut Context<Self>) {
        self.multiple = multiple;
        cx.notify();
    }

    #[must_use]
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// The list controller the grid renders. Selection is the controller's.
    #[must_use]
    pub fn controller(&self) -> Entity<ListController<T>> {
        self.controller.clone()
    }

    #[must_use]
    pub fn is_selected(&self, id: &ItemId, cx: &App) -> bool {
        self.controller.read(cx).is_selected(id)
    }

    fn selected_ids(&self, cx: &App) -> Vec<ItemId> {
        self.controller.read(cx).selected_ids().to_vec()
    }

    fn click(
        &mut self,
        id: ItemId,
        modifiers: gpui::Modifiers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let multiple = self.multiple;
        let secondary = modifiers.secondary(cx);
        self.controller.update(cx, |controller, cx| {
            if modifiers.shift {
                controller.extend_to(id.clone());
            } else if multiple && secondary {
                controller.toggle(id.clone());
            } else {
                controller.select(id.clone());
            }
            cx.notify();
        });
        cx.emit(IconGridEvent::SelectionChanged);
        cx.notify();
    }

    fn activate(&mut self, cx: &mut Context<Self>) {
        let ids = self.selected_ids(cx);
        if !ids.is_empty() {
            cx.emit(IconGridEvent::Activated(ids));
        }
    }

    fn step(&mut self, dx: isize, dy: isize, extend: bool, cx: &mut Context<Self>) {
        let columns = self.columns.get().max(1);
        let controller = self.controller.read(cx);
        let len = controller.visible_len();
        if len == 0 {
            return;
        }
        let current = controller
            .selection()
            .and_then(|id| controller.visible_index_of(id));
        let index = match current {
            Some(index) => {
                let row = index / columns;
                let column = index % columns;
                let next_row = if dy < 0 {
                    row.saturating_sub(1)
                } else {
                    row + dy as usize
                };
                let next_column = if dx < 0 {
                    column.saturating_sub(1)
                } else {
                    (column + dx as usize).min(columns - 1)
                };
                (next_row * columns + next_column).min(len - 1)
            }
            None => 0,
        };
        let id = {
            let controller = self.controller.read(cx);
            controller
                .visible_item(index)
                .map(|item| controller.id_of(item))
        };
        let Some(id) = id else {
            return;
        };
        self.controller.update(cx, |controller, cx| {
            if extend && self.multiple {
                controller.extend_to(id.clone());
            } else {
                controller.select(id.clone());
            }
            cx.notify();
        });
        let row = index / columns;
        self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        cx.emit(IconGridEvent::SelectionChanged);
        cx.notify();
    }

    fn marquee_started(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.marquee.set(Some((position, position)));
        self.controller.update(cx, |controller, cx| {
            controller.clear_selection();
            cx.notify();
        });
        cx.notify();
    }

    fn marquee_moved(
        &mut self,
        position: Point<Pixels>,
        pressed: Option<MouseButton>,
        cx: &mut Context<Self>,
    ) {
        if pressed != Some(MouseButton::Left) {
            return;
        }
        let Some((start, _)) = self.marquee.get() else {
            return;
        };
        self.marquee.set(Some((start, position)));
        self.select_marquee(start, position, cx);
        cx.notify();
    }

    fn marquee_ended(&mut self, cx: &mut Context<Self>) {
        if self.marquee.take().is_some() {
            cx.notify();
        }
    }

    /// Top-left of the virtualized content in window coordinates, after the
    /// current scroll offset. The list's own bounds already contain the grid
    /// gutter; the offset is GPUI's negated scroll position.
    fn content_origin(&self) -> Point<Pixels> {
        let state = self.scroll.0.borrow();
        let handle = &state.base_handle;
        let bounds = handle.bounds();
        let offset = handle.offset();
        point(bounds.origin.x + offset.x, bounds.origin.y + offset.y)
    }

    /// The logical cell containing a window point, when an item exists there.
    fn cell_index_at(&self, position: Point<Pixels>, cx: &App) -> Option<usize> {
        let origin = self.content_origin();
        let columns = self.columns.get().max(1);
        let x = (position.x - origin.x).as_f32();
        let y = (position.y - origin.y).as_f32();
        if x < 0.0 || y < 0.0 {
            return None;
        }
        let column = (x / CELL_WIDTH).floor() as usize;
        let row = (y / CELL_HEIGHT).floor() as usize;
        let index = row.saturating_mul(columns).saturating_add(column);
        (index < self.controller.read(cx).visible_len()).then_some(index)
    }

    fn select_marquee(&mut self, start: Point<Pixels>, end: Point<Pixels>, cx: &mut Context<Self>) {
        let rect = Bounds::new(
            point(start.x.min(end.x), start.y.min(end.y)),
            size((start.x - end.x).abs(), (start.y - end.y).abs()),
        );
        let origin = self.content_origin();
        let columns = self.columns.get().max(1);
        let span = |start: f32, end: f32, pitch: f32| {
            let first = (start / pitch).floor().max(0.0) as usize;
            let last = ((end - 0.01).max(0.0) / pitch).floor().max(0.0) as usize;
            (first.min(last), last)
        };
        let (first_column, last_column) = span(
            (rect.left() - origin.x).as_f32(),
            (rect.right() - origin.x).as_f32(),
            CELL_WIDTH,
        );
        let (first_row, last_row) = span(
            (rect.top() - origin.y).as_f32(),
            (rect.bottom() - origin.y).as_f32(),
            CELL_HEIGHT,
        );
        let last_column = last_column.min(columns - 1);
        let ids = {
            let controller = self.controller.read(cx);
            let len = controller.visible_len();
            let mut ids = Vec::new();
            for row in first_row..=last_row {
                for column in first_column..=last_column {
                    let index = row.saturating_mul(columns).saturating_add(column);
                    if index >= len {
                        break;
                    }
                    if let Some(item) = controller.visible_item(index) {
                        ids.push(controller.id_of(item));
                    }
                }
            }
            ids
        };
        self.controller.update(cx, |controller, cx| {
            controller.set_selection(ids);
            cx.notify();
        });
        cx.emit(IconGridEvent::SelectionChanged);
    }

    fn jump(&mut self, last: bool, extend: bool, cx: &mut Context<Self>) {
        let columns = self.columns.get().max(1);
        let (len, id) = {
            let controller = self.controller.read(cx);
            let len = controller.visible_len();
            if len == 0 {
                return;
            }
            let index = if last { len - 1 } else { 0 };
            (
                len,
                controller
                    .visible_item(index)
                    .map(|item| controller.id_of(item)),
            )
        };
        let Some(id) = id else {
            return;
        };
        self.controller.update(cx, |controller, cx| {
            if extend && self.multiple {
                controller.extend_to(id.clone());
            } else {
                controller.select(id.clone());
            }
            cx.notify();
        });
        let row = if last { (len - 1) / columns } else { 0 };
        self.scroll.scroll_to_item(row, ScrollStrategy::Nearest);
        cx.emit(IconGridEvent::SelectionChanged);
        cx.notify();
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let modifiers = event.keystroke.modifiers;
        if modifiers.secondary(cx) && event.keystroke.key == "a" && self.multiple {
            self.controller.update(cx, |controller, cx| {
                controller.select_all();
                cx.notify();
            });
            cx.emit(IconGridEvent::SelectionChanged);
            cx.notify();
            return;
        }
        match event.keystroke.key.as_str() {
            "left" => self.step(-1, 0, modifiers.shift, cx),
            "right" => self.step(1, 0, modifiers.shift, cx),
            "up" => self.step(0, -1, modifiers.shift, cx),
            "down" => self.step(0, 1, modifiers.shift, cx),
            "home" => self.jump(false, modifiers.shift, cx),
            "end" => self.jump(true, modifiers.shift, cx),
            "enter" => self.activate(cx),
            "space" => {
                if let Some(id) = self.selected_ids(cx).first().cloned() {
                    self.controller.update(cx, |controller, cx| {
                        controller.toggle(id);
                        cx.notify();
                    });
                    cx.emit(IconGridEvent::SelectionChanged);
                    cx.notify();
                }
            }
            _ => {}
        }
        let _ = window;
    }
}

impl<T: 'static> Focusable for IconGrid<T> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl<T: 'static> EventEmitter<IconGridEvent> for IconGrid<T> {}

impl<T: 'static> gpui::Render for IconGrid<T> {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let controller_entity = self.controller.clone();
        let count = controller_entity.read(cx).visible_len();
        let draggable = self.draggable;

        let viewport = self.scroll.viewport().width;
        let columns = if viewport > px(0.) {
            (f32::from(viewport) / CELL_WIDTH).floor().max(1.0) as usize
        } else {
            4
        };
        self.columns.set(columns);
        let rows = count.div_ceil(columns);
        let icon = self.icon.clone();
        let label = self.label.clone();
        let focus = self.focus.clone();
        let view = cx.entity();
        let editing = self
            .editing
            .as_ref()
            .map(|editing| (editing.id.clone(), editing.field.clone()));
        let list_theme = theme.clone();
        let marquee = self.marquee.clone();
        let band = theme.lists.table_active;
        let list = uniform_list("icon-rows", rows, move |range, window, cx| {
            range
                .map(|row| {
                    let mut cells = Vec::with_capacity(columns);
                    for column in 0..columns {
                        let index = row * columns + column;
                        let active =
                            window.is_window_active() && focus.contains_focused(window, cx);
                        let icon = icon.clone();
                        // Materialise only the cells this range actually
                        // builds; the controller stays the single source of
                        // truth and no second item snapshot exists.
                        let cell = controller_entity.update(cx, |controller, cx| {
                            let item = controller.visible_item(index)?;
                            let id = controller.id_of(item);
                            let is_selected = controller.is_selected(&id);
                            let drag_ids = if is_selected {
                                controller.selected_ids().to_vec()
                            } else {
                                vec![id.clone()]
                            };
                            let label = (label)(item);
                            let icon_element = (icon)(item, is_selected, active, window, cx);
                            Some((id, is_selected, label, icon_element, drag_ids))
                        });
                        let Some((id, is_selected, label, icon_element, drag_ids)) = cell else {
                            break;
                        };
                        let drag = draggable.then(|| IconGridDrag {
                            source: id.clone(),
                            ids: drag_ids,
                        });
                        let selection = if active {
                            list_theme.lists.table_active
                        } else {
                            list_theme.lists.table_inactive
                        };
                        let text = if is_selected {
                            selection.text
                        } else {
                            list_theme.colors.text
                        };
                        let click_id = id.clone();
                        let menu_id = id.clone();
                        let view = view.clone();
                        let click_view = view.clone();
                        let menu_view = view.clone();
                        // The click target is the whole cell, but the selection
                        // artwork hugs the filename text block (Finder icon
                        // view, M: panel.icon.selection). The cell keeps its
                        // full CELL_WIDTH x CELL_HEIGHT hit area through the
                        // absolute overlay below.
                        let editing_this = match &editing {
                            Some((editing_id, field)) if *editing_id == id => Some(field.clone()),
                            _ => None,
                        };
                        let label_block = div()
                            .debug_selector({
                                let id = id.clone();
                                move || format!("icon-selection-{id}")
                            })
                            .relative()
                            .flex_none()
                            .max_w(px(CELL_WIDTH - 8.0))
                            .px(px(3.))
                            .py(px(1.))
                            .when(is_selected && editing_this.is_none(), |el| {
                                el.child(
                                    canvas(
                                        |_, _, _| (),
                                        move |bounds, _, window, _| {
                                            paint::selection_pill(window, bounds, &selection);
                                        },
                                    )
                                    .absolute()
                                    .inset_0(),
                                )
                            })
                            .child(match &editing_this {
                                Some(field) => div()
                                    .w(px(CELL_WIDTH - 14.0))
                                    .child(field.clone())
                                    .into_any_element(),
                                None => div().text_center().child(label).into_any_element(),
                            });
                        cells.push(
                            div()
                                .id(SharedString::from(format!("icon-cell-{id}")))
                                .debug_selector({
                                    let id = id.clone();
                                    move || format!("icon-cell-{id}")
                                })
                                .relative()
                                .w(px(CELL_WIDTH))
                                .h(px(CELL_HEIGHT))
                                .flex_none()
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_start()
                                .pt(px(4.))
                                .gap(px(3.))
                                .text_size(px(list_theme.type_.size_small))
                                .text_color(hsla(text))
                                .child(icon_element)
                                .child(label_block)
                                .child(
                                    div()
                                        .id(SharedString::from(format!("icon-hit-{id}")))
                                        .absolute()
                                        .inset_0()
                                        .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .when_some(drag, |el, drag| {
                                            el.on_drag(drag, |_, _, _, cx| {
                                                cx.new(|_| gpui::EmptyView)
                                            })
                                        })
                                        .on_click(move |event, window, cx| {
                                            let id = click_id.clone();
                                            let modifiers = event.modifiers();
                                            let click_count = event.click_count();
                                            click_view.update(cx, |grid, cx| {
                                                grid.click(id.clone(), modifiers, window, cx);
                                                if click_count == 2 {
                                                    grid.activate(cx);
                                                }
                                            });
                                        })
                                        .on_mouse_down(
                                            MouseButton::Right,
                                            move |event, window, cx| {
                                                let id = menu_id.clone();
                                                menu_view.update(cx, |grid, cx| {
                                                    window.focus(&grid.focus, cx);
                                                    if !grid.controller.read(cx).is_selected(&id) {
                                                        grid.controller.update(cx, |c, cx| {
                                                            c.select(id.clone());
                                                            cx.notify();
                                                        });
                                                    }
                                                    let ids = grid
                                                        .controller
                                                        .read(cx)
                                                        .selected_ids()
                                                        .to_vec();
                                                    cx.emit(IconGridEvent::ContextMenu {
                                                        ids,
                                                        position: event.position,
                                                    });
                                                    cx.notify();
                                                });
                                                cx.stop_propagation();
                                            },
                                        ),
                                )
                                .into_any_element(),
                        );
                    }
                    div()
                        .flex()
                        .flex_row()
                        .h(px(CELL_HEIGHT))
                        .children(cells)
                        .into_any_element()
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll)
        .size_full();

        div()
            .id("icon-grid")
            .debug_selector(|| "icon-grid".into())
            .key_context("SilvestrisIconGrid")
            .track_focus(&self.focus)
            .size_full()
            .bg(hsla(theme.colors.content))
            .on_key_down(cx.listener(|grid, event: &KeyDownEvent, window, cx| {
                grid.key_down(event, window, cx);
            }))
            .capture_key_down(cx.listener(|grid, event: &KeyDownEvent, window, cx| {
                if grid.is_editing()
                    && event.keystroke.key == "escape"
                    && !event.keystroke.modifiers.modified()
                {
                    cx.stop_propagation();
                    grid.finish_editing(false, window, cx);
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|grid, event: &MouseDownEvent, window, cx| {
                    if grid.is_editing() {
                        grid.finish_editing(true, window, cx);
                    }
                    cx.emit(IconGridEvent::BackgroundContextMenu {
                        position: event.position,
                    });
                    cx.stop_propagation();
                }),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|grid, event: &MouseDownEvent, window, cx| {
                    // A cell's overlay stops propagation, but the logical
                    // index is the authoritative test: a press on an icon must
                    // never start a marquee that clears the selection.
                    if grid.cell_index_at(event.position, cx).is_some() {
                        return;
                    }
                    grid.marquee_started(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(|grid, event: &MouseMoveEvent, _, cx| {
                grid.marquee_moved(event.position, event.pressed_button, cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|grid, _: &MouseUpEvent, _, cx| grid.marquee_ended(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|grid, _: &MouseUpEvent, _, cx| grid.marquee_ended(cx)),
            )
            .child(
                ScrollView::new("icon-grid-scroll", &self.scroll)
                    .axes(ScrollAxes::Vertical)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .size_full()
                            .pt(px(GRID_GUTTER))
                            .pl(px(GRID_GUTTER))
                            .child(list),
                    ),
            )
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let Some((start, end)) = marquee.get() else {
                            return;
                        };
                        let rect = Bounds::new(
                            point(
                                start.x.min(end.x) - bounds.origin.x,
                                start.y.min(end.y) - bounds.origin.y,
                            ),
                            size((start.x - end.x).abs(), (start.y - end.y).abs()),
                        );
                        window.paint_quad(gpui::fill(rect, hsla(band.fill.from).opacity(0.22)));
                        let h = crate::convert::hairline(window);
                        let line = hsla(band.line);
                        window.paint_quad(gpui::fill(
                            Bounds::new(rect.origin, size(rect.size.width, h)),
                            line,
                        ));
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(rect.origin.x, rect.origin.y + rect.size.height - h),
                                size(rect.size.width, h),
                            ),
                            line,
                        ));
                        window.paint_quad(gpui::fill(
                            Bounds::new(rect.origin, size(h, rect.size.height)),
                            line,
                        ));
                        window.paint_quad(gpui::fill(
                            Bounds::new(
                                point(rect.origin.x + rect.size.width - h, rect.origin.y),
                                size(h, rect.size.height),
                            ),
                            line,
                        ));
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}
