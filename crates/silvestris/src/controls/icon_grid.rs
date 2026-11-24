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
    FocusHandle, Focusable, KeyDownEvent, ScrollStrategy, SharedString, UniformListScrollHandle,
    Window,
};

use super::scroll_source::ScrollSource as _;
use super::scroll_view::{ScrollAxes, ScrollView};
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconGridEvent {
    /// A user click, Shift-click, toggle or keyboard move changed the
    /// selection. Programmatic selection never emits this.
    SelectionChanged,
    /// Double-click or Return on the selection.
    Activated(Vec<ItemId>),
}

pub struct IconGrid<T> {
    controller: Entity<ListController<T>>,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    icon: IconFn<T>,
    label: LabelFn<T>,
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
            multiple: false,
            columns: Rc::new(Cell::new(4)),
        }
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
        let controller = controller_entity.read(cx);
        let count = controller.visible_len();
        let items: Vec<(ItemId, SharedString)> = controller
            .visible_items()
            .into_iter()
            .map(|item| (controller.id_of(item), (self.label)(item)))
            .collect();

        let viewport = self.scroll.viewport().width;
        let columns = if viewport > px(0.) {
            (f32::from(viewport) / CELL_WIDTH).floor().max(1.0) as usize
        } else {
            4
        };
        self.columns.set(columns);
        let rows = count.div_ceil(columns);
        let icon = self.icon.clone();
        let focus = self.focus.clone();
        let view = cx.entity();
        let list_theme = theme.clone();
        let list = uniform_list("icon-rows", rows, move |range, window, cx| {
            range
                .map(|row| {
                    let mut cells = Vec::with_capacity(columns);
                    for column in 0..columns {
                        let index = row * columns + column;
                        let Some((id, label)) = items.get(index) else {
                            break;
                        };
                        let id = id.clone();
                        let is_selected = controller_entity.read(cx).is_selected(&id);
                        let active =
                            window.is_window_active() && focus.contains_focused(window, cx);
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
                        let icon = icon.clone();
                        let icon_element = controller_entity
                            .update(cx, |controller, cx| {
                                controller
                                    .visible_item(index)
                                    .map(|item| (icon)(item, is_selected, active, window, cx))
                            })
                            .unwrap_or_else(|| div().size(px(ICON_SIZE)).into_any_element());
                        let cell_id = id.clone();
                        let view = view.clone();
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
                                .child(div().w_full().px(px(4.)).text_center().child(label.clone()))
                                .child(
                                    div()
                                        .id(SharedString::from(format!("icon-hit-{id}")))
                                        .absolute()
                                        .inset_0()
                                        .on_click(move |event, window, cx| {
                                            let id = cell_id.clone();
                                            let modifiers = event.modifiers();
                                            let click_count = event.click_count();
                                            view.update(cx, |grid, cx| {
                                                grid.click(id.clone(), modifiers, window, cx);
                                                if click_count == 2 {
                                                    grid.activate(cx);
                                                }
                                            });
                                        }),
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
            .child(
                ScrollView::new("icon-grid-scroll", &self.scroll)
                    .axes(ScrollAxes::Vertical)
                    .child(list),
            )
    }
}
