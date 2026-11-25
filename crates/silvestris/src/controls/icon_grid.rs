//! Icon grid (#35): the third presentation in Open and Save panels.
//!
//! The grid renders an app-owned [`ListController<T>`] in fixed cells, with
//! the same selection rules as `TableView`: click, Shift-click extends,
//! Command/Control-click toggles, arrows move, Return activates and a
//! double-click activates. Rows of cells are virtualised with `uniform_list`,
//! so a 30,000-entry directory builds only the rows in view.
//!
//! The control does not read files or own a location. The consumer supplies
//! the icon and label closures; the panel uses the same full-colour 16 px icon
//! vocabulary as its list and sidebar.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, uniform_list, AnyElement, App, Context, Entity, EventEmitter,
    FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Point,
    ScrollStrategy, SharedString, Subscription, UniformListScrollHandle, Window,
};

use super::scroll_source::ScrollSource as _;
use super::scroll_view::{ScrollAxes, ScrollView};
use super::text_field::{TextField, TextFieldEvent};
use crate::binding::{ItemId, ListController};
use crate::convert::hsla;
use crate::{paint, ActiveTheme};

/// Measured panel icon cell: four columns fit the 367 px browser at 1x.
pub const CELL_WIDTH: f32 = 88.0;
/// Icon, two label lines and the selection inset.
pub const CELL_HEIGHT: f32 = 76.0;
/// Full-colour file icon in an icon-view cell.
pub const ICON_SIZE: f32 = 32.0;

type IconFn<T> = Rc<dyn Fn(&T, bool, bool, &mut Window, &mut App) -> AnyElement>;
type LabelFn<T> = Rc<dyn Fn(&T) -> SharedString>;
type RenameFn<T> = Rc<dyn Fn(&T, &str, &mut App) -> bool>;

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

pub struct IconGrid<T> {
    controller: Entity<ListController<T>>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    icon: IconFn<T>,
    label: LabelFn<T>,
    rename: Option<RenameFn<T>>,
    editing: Option<Editing>,
    multiple: bool,
    /// Columns in the last frame; keyboard movement needs the same pitch.
    columns: Rc<Cell<usize>>,
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
            columns: Rc::new(Cell::new(4)),
        }
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
            "home" => self.step(-(i16::MAX as isize), 0, false, cx),
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
                            let label = (label)(item);
                            let icon_element = (icon)(item, is_selected, active, window, cx);
                            Some((id, is_selected, label, icon_element))
                        });
                        let Some((id, is_selected, label, icon_element)) = cell else {
                            break;
                        };
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
                                .child(
                                    canvas(
                                        |_, _, _| (),
                                        move |bounds, _, window, _| {
                                            if is_selected {
                                                paint::selection(window, bounds, &selection);
                                            }
                                        },
                                    )
                                    .absolute()
                                    .inset_0(),
                                )
                                .child(icon_element)
                                .child(match &editing {
                                    Some((editing_id, field)) if *editing_id == id => div()
                                        .w_full()
                                        .px(px(4.))
                                        .child(field.clone())
                                        .into_any_element(),
                                    _ => div()
                                        .w_full()
                                        .px(px(4.))
                                        .text_center()
                                        .child(label)
                                        .into_any_element(),
                                })
                                .child(
                                    div()
                                        .id(SharedString::from(format!("icon-hit-{id}")))
                                        .absolute()
                                        .inset_0()
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
            .child(
                ScrollView::new("icon-grid-scroll", &self.scroll)
                    .axes(ScrollAxes::Vertical)
                    .child(list),
            )
    }
}
