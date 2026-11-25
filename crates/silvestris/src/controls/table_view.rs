//! Table view (#32; HIG List Views p. 333, Scrolling Lists p. 328).
//!
//! A `TableView<T>` renders rows from an app-owned
//! [`ListController<T>`](crate::binding::ListController) entity: the
//! controller owns items, sort, filter and selection by id; the table owns
//! presentation, columns and input. Rows are virtualised with
//! `uniform_list`, so per-frame cost depends on the viewport, not the row
//! count.
//!
//! - Header cells sort on click (again to reverse), resize by dragging the
//!   divider, reorder by dragging the cell, and can be hidden.
//! - Column order, widths and hidden columns persist under an autosave key
//!   through a [`ColumnStore`].
//! - Selection: click, Shift-click extends from the anchor, Command/Control-
//!   click toggles, arrows, Shift-arrows, Home/End, Page Up/Down and
//!   Command/Control-A. Programmatic selection never calls the callback.
//! - Type-select matches the start of the sort column's text (or the first
//!   text column); the typed prefix resets after a pause.
//! - Return edits the lead row's first editable column in place with the
//!   text core; Return or leaving the field commits, Escape cancels.
//! - Double-click activates; secondary click reports the clicked selection
//!   for a contextual menu.
//! - The selection is blue while the table has focus in the active window,
//!   grey otherwise (`Lists::table_active`, `table_inactive`).
use std::cmp::Ordering;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    canvas, div, point, prelude::*, px, uniform_list, AnyElement, App, Axis, Bounds, Context,
    Entity, FocusHandle, Focusable, KeyDownEvent, ListHorizontalSizingBehavior, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point, SharedString,
    Subscription, Task, UniformListScrollHandle, Window,
};

use super::column_store::{ColumnStore, SavedColumns};
use super::scroll_source::{self, ScrollSource as _};
use super::scroll_view::{ScrollAxes, ScrollView};
use super::text_field::{TextField, TextFieldEvent};
use crate::binding::{ItemId, ListController};
use crate::convert::{hsla, rows};
use crate::{paint, ActiveTheme};

/// Estimate: the typed prefix resets after this pause (no corpus source
/// gives the interval).
pub const TYPE_SELECT_RESET: Duration = Duration::from_millis(1000);
/// Pointer distance from a divider that grabs it.
const RESIZE_ZONE: f32 = 3.0;
/// Horizontal travel before a header press becomes a column drag.
const REORDER_THRESHOLD: f32 = 4.0;
/// Label inset inside header and body cells (M: table.header.sorted: the
/// Finder "Name" label starts 5 px after the cell edge).
const CELL_INSET: f32 = 5.0;

type CellFn<T> = Rc<dyn Fn(&T, &CellContext, &mut Window, &mut App) -> AnyElement>;
type TextFn<T> = Rc<dyn Fn(&T) -> SharedString>;
type SortFn<T> = Rc<dyn Fn(&T, &T) -> Ordering>;
type CommitFn<T> = Rc<dyn Fn(&T, &str, &mut App) -> bool>;

/// Horizontal placement of a column's contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColumnAlign {
    #[default]
    Leading,
    Center,
    Trailing,
}

/// Which grid lines a table draws between cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GridLines {
    /// Alternating stripes only, as in the Finder list view.
    #[default]
    None,
    Vertical,
    Horizontal,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// What a cell renderer knows about its row.
#[derive(Clone, Copy, Debug)]
pub struct CellContext {
    pub row: usize,
    pub selected: bool,
    /// The selection is drawn in the active colour: text on it should use
    /// `text`.
    pub emphasized: bool,
    /// Text colour for this row.
    pub text: silvestris_core::Rgba8,
    pub width: f32,
}

/// One table column. Cells come from a text accessor or a custom renderer.
pub struct TableColumn<T> {
    id: SharedString,
    title: SharedString,
    width: f32,
    min_width: f32,
    max_width: f32,
    resizable: bool,
    hidden: bool,
    align: ColumnAlign,
    text: Option<TextFn<T>>,
    cell: Option<CellFn<T>>,
    sort: Option<SortFn<T>>,
    commit: Option<CommitFn<T>>,
}

impl<T: 'static> TableColumn<T> {
    /// A text column. Its text also drives type-select and, with
    /// [`sort_by_text`](Self::sort_by_text), sorting.
    pub fn text(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        text: impl Fn(&T) -> SharedString + 'static,
    ) -> Self {
        let mut column = Self::blank(id.into(), title.into());
        column.text = Some(Rc::new(text));
        column
    }

    /// A column drawn by `cell`: an icon and text (see [`icon_text`]), a
    /// `Checkbox`, `PopUpButton` or `ProgressIndicator`, or any element.
    pub fn custom(
        id: impl Into<SharedString>,
        title: impl Into<SharedString>,
        cell: impl Fn(&T, &CellContext, &mut Window, &mut App) -> AnyElement + 'static,
    ) -> Self {
        let mut column = Self::blank(id.into(), title.into());
        column.cell = Some(Rc::new(cell));
        column
    }

    fn blank(id: SharedString, title: SharedString) -> Self {
        Self {
            id,
            title,
            width: 100.0,
            min_width: 24.0,
            max_width: 10_000.0,
            resizable: true,
            hidden: false,
            align: ColumnAlign::Leading,
            text: None,
            cell: None,
            sort: None,
            commit: None,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    pub fn align(mut self, align: ColumnAlign) -> Self {
        self.align = align;
        self
    }

    /// Text accessor for a custom column, for type-select and sorting.
    pub fn text_value(mut self, text: impl Fn(&T) -> SharedString + 'static) -> Self {
        self.text = Some(Rc::new(text));
        self
    }

    /// Sort by `compare` when the header is clicked (ascending order).
    pub fn sortable(mut self, compare: impl Fn(&T, &T) -> Ordering + 'static) -> Self {
        self.sort = Some(Rc::new(compare));
        self
    }

    /// Sort by the column text, case-insensitively. The comparison folds each
    /// character in place, so it allocates no temporary strings. A model with
    /// a precomputed key should use [`sortable`](Self::sortable) instead.
    pub fn sort_by_text(self) -> Self {
        let text = self.text.clone();
        self.sortable(move |a, b| match &text {
            Some(text) => lower_order(&text(a), &text(b)),
            None => Ordering::Equal,
        })
    }

    /// Allow in-place editing. `commit` receives the row and the new text;
    /// it updates the app model and returns whether the value was accepted.
    pub fn editable(mut self, commit: impl Fn(&T, &str, &mut App) -> bool + 'static) -> Self {
        self.commit = Some(Rc::new(commit));
        self
    }

    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// An icon followed by text, the Finder Name cell.
pub fn icon_text(icon: impl IntoElement, label: impl Into<SharedString>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .overflow_hidden()
        .child(div().flex_none().child(icon))
        .child(div().truncate().child(label.into()))
        .into_any_element()
}

/// Case-insensitive string order without allocating a lowercase copy.
fn lower_order(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_lowercase)
        .cmp(b.chars().flat_map(char::to_lowercase))
}

/// What the user did, reported to the app.
#[derive(Clone, Debug, PartialEq)]
pub enum TableEvent {
    /// The user changed the selection (never sent for programmatic changes).
    SelectionChanged(Vec<ItemId>),
    /// Double-click or Enter on the selection.
    Activated(Vec<ItemId>),
    /// Secondary click: the selection to act on and the window position for
    /// a contextual menu.
    ContextMenu {
        ids: Vec<ItemId>,
        position: Point<Pixels>,
    },
    /// Secondary click on the empty area below the rows (or between them):
    /// the window position for a background contextual menu.
    BackgroundContextMenu { position: Point<Pixels> },
    /// The user sorted by `column`.
    Sorted {
        column: SharedString,
        direction: SortDirection,
    },
    /// An edit was committed and accepted.
    Edited { id: ItemId, column: SharedString },
}

impl<T: 'static> gpui::EventEmitter<TableEvent> for TableView<T> {}

struct Editing {
    id: ItemId,
    column: SharedString,
    field: Entity<TextField>,
    _subscription: Subscription,
}

#[derive(Clone, Copy, Debug)]
enum HeaderDrag {
    Resize {
        column: usize,
        start_x: Pixels,
        start_width: f32,
    },
    Press {
        column: usize,
        start_x: Pixels,
    },
    Reorder {
        column: usize,
    },
}

pub struct TableView<T: 'static> {
    controller: Entity<ListController<T>>,
    columns: Vec<TableColumn<T>>,
    /// Display order as indices into `columns`.
    order: Vec<usize>,
    sort: Option<(SharedString, SortDirection)>,
    /// When true the app owns sorting: a header click updates the indicator and
    /// emits [`TableEvent::Sorted`], but does not change the controller. The
    /// file panel sets this so its single sort model stays authoritative.
    external_sort: bool,
    scroll: UniformListScrollHandle,
    focus: FocusHandle,
    row_height: f32,
    header_height: f32,
    header_visible: bool,
    alternating: bool,
    fill_width: bool,
    multiple: bool,
    grid: GridLines,
    autosave: Option<(Rc<dyn ColumnStore>, String)>,
    editing: Option<Editing>,
    typed: String,
    typed_reset: Option<Task<()>>,
    header_drag: Option<HeaderDrag>,
    _observe: Subscription,
}

impl<T: 'static> Focusable for TableView<T> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl<T: 'static> TableView<T> {
    pub fn new(
        controller: Entity<ListController<T>>,
        columns: Vec<TableColumn<T>>,
        cx: &mut Context<Self>,
    ) -> Self {
        // Live model updates repaint without touching scroll or selection.
        let observe = cx.observe(&controller, |_, _, cx| cx.notify());
        let theme = cx.theme();
        let order = (0..columns.len()).collect();
        Self {
            controller,
            columns,
            order,
            sort: None,
            external_sort: false,
            scroll: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            row_height: theme.metrics.table_row_height,
            header_height: theme.metrics.table_header_height,
            header_visible: true,
            alternating: true,
            fill_width: false,
            multiple: true,
            grid: GridLines::None,
            autosave: None,
            editing: None,
            typed: String::new(),
            typed_reset: None,
            header_drag: None,
            _observe: observe,
        }
    }

    /// Fixed row height; views with icons use taller rows (the Finder list
    /// view uses 19 px).
    pub fn set_row_height(&mut self, height: f32, cx: &mut Context<Self>) {
        self.row_height = height;
        cx.notify();
    }

    pub fn set_header_height(&mut self, height: f32, cx: &mut Context<Self>) {
        self.header_height = height;
        cx.notify();
    }

    /// `false` removes the header row and its hit-testing. Column-view
    /// columns and the Open panel's simple lists use a headerless table.
    pub fn set_header_visible(&mut self, visible: bool, cx: &mut Context<Self>) {
        self.header_visible = visible;
        cx.notify();
    }

    /// `false` paints every row on `colors.content`; the Finder column view
    /// is unstriped.
    pub fn set_alternating(&mut self, alternating: bool, cx: &mut Context<Self>) {
        self.alternating = alternating;
        cx.notify();
    }

    /// Stretch the last visible column to the viewport. The horizontal
    /// scroller then has nothing to scroll, so it stays hidden.
    pub fn set_fill_width(&mut self, fill: bool, cx: &mut Context<Self>) {
        self.fill_width = fill;
        if fill {
            // Fill-width content is anchored at x = 0; a stale horizontal
            // offset would separate headers and rows.
            let position = self.scroll.position();
            self.scroll.set_position(point(px(0.), position.y));
        }
        cx.notify();
    }

    /// Request a repaint from outside the view. Views that re-render from a
    /// borrowed model use this after changing it.
    pub fn repaint(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    /// Whether the header row is shown.
    pub fn header_visible(&self) -> bool {
        self.header_visible
    }

    /// Whether rows alternate.
    pub fn alternating(&self) -> bool {
        self.alternating
    }

    /// Whether the last visible column fills the viewport.
    pub fn fill_width(&self) -> bool {
        self.fill_width
    }

    /// `false` restricts the user to one selected row.
    pub fn set_allows_multiple(&mut self, multiple: bool) {
        self.multiple = multiple;
    }

    pub fn set_grid(&mut self, grid: GridLines, cx: &mut Context<Self>) {
        self.grid = grid;
        cx.notify();
    }

    /// Restore and then persist the column layout under `key`.
    pub fn set_autosave(&mut self, store: Rc<dyn ColumnStore>, key: impl Into<String>, cx: &App) {
        let key = key.into();
        if let Some(saved) = store.load(&key, cx) {
            self.apply_saved(&saved);
        }
        self.autosave = Some((store, key));
    }

    fn apply_saved(&mut self, saved: &SavedColumns) {
        let mut order: Vec<usize> = saved
            .order
            .iter()
            .filter_map(|id| self.columns.iter().position(|c| c.id.as_ref() == id))
            .collect();
        for index in 0..self.columns.len() {
            if !order.contains(&index) {
                order.push(index);
            }
        }
        self.order = order;
        for column in &mut self.columns {
            if let Some(width) = saved.widths.get(column.id.as_ref()) {
                column.width = width.clamp(column.min_width, column.max_width);
            }
            column.hidden = saved.hidden.iter().any(|id| id == column.id.as_ref());
        }
    }

    /// The current column layout, as it would be saved.
    pub fn saved_columns(&self) -> SavedColumns {
        SavedColumns {
            order: self
                .order
                .iter()
                .map(|i| self.columns[*i].id.to_string())
                .collect(),
            widths: self
                .columns
                .iter()
                .map(|c| (c.id.to_string(), c.width))
                .collect(),
            hidden: self
                .columns
                .iter()
                .filter(|c| c.hidden)
                .map(|c| c.id.to_string())
                .collect(),
        }
    }

    fn persist(&self, cx: &mut App) {
        if let Some((store, key)) = &self.autosave {
            store.save(key, self.saved_columns(), cx);
        }
    }

    pub fn set_column_hidden(&mut self, id: &str, hidden: bool, cx: &mut Context<Self>) {
        if let Some(column) = self.columns.iter_mut().find(|c| c.id.as_ref() == id) {
            column.hidden = hidden;
            self.persist(cx);
            cx.notify();
        }
    }

    pub fn column_width(&self, id: &str) -> Option<f32> {
        self.columns
            .iter()
            .find(|c| c.id.as_ref() == id)
            .map(|c| c.width)
    }

    /// Column ids in display order, hidden ones included.
    pub fn column_order(&self) -> Vec<SharedString> {
        self.order
            .iter()
            .map(|i| self.columns[*i].id.clone())
            .collect()
    }

    pub fn sort(&self) -> Option<(SharedString, SortDirection)> {
        self.sort.clone()
    }

    /// Let the app own sorting. A header click then updates only this table's
    /// indicator and emits [`TableEvent::Sorted`]; the app applies the same
    /// policy to the controller and to every other view.
    pub fn set_external_sort(&mut self, external: bool, cx: &mut Context<Self>) {
        self.external_sort = external;
        cx.notify();
    }

    /// Show `column` as sorted without touching the controller or emitting an
    /// event. The panel calls it when its sort model changes from a menu.
    pub fn show_sort(&mut self, column: &str, direction: SortDirection, cx: &mut Context<Self>) {
        if self.columns.iter().any(|c| c.id.as_ref() == column) {
            self.sort = Some((column.to_string().into(), direction));
            cx.notify();
        }
    }

    /// Screen `column` as unsorted without touching the controller.
    pub fn clear_sort(&mut self, cx: &mut Context<Self>) {
        if self.sort.take().is_some() {
            cx.notify();
        }
    }

    /// Sort by `column` programmatically (no event). With
    /// [`set_external_sort`](Self::set_external_sort) this only updates the
    /// presentation, because the app owns the controller's comparator.
    pub fn set_sort(&mut self, column: &str, direction: SortDirection, cx: &mut Context<Self>) {
        let Some(compare) = self
            .columns
            .iter()
            .find(|c| c.id.as_ref() == column)
            .and_then(|c| c.sort.clone())
        else {
            return;
        };
        self.sort = Some((column.to_string().into(), direction));
        if self.external_sort {
            self.reveal_lead(cx);
            return;
        }
        self.controller.update(cx, |controller, cx| {
            match direction {
                SortDirection::Ascending => {
                    controller.set_sort(Some(move |a: &T, b: &T| compare(a, b)))
                }
                SortDirection::Descending => {
                    controller.set_sort(Some(move |a: &T, b: &T| compare(b, a)))
                }
            }
            cx.notify();
        });
        self.reveal_lead(cx);
    }

    pub fn scroll_handle(&self) -> &UniformListScrollHandle {
        &self.scroll
    }

    pub fn is_editing(&self) -> bool {
        self.editing.is_some()
    }

    fn visible_columns(&self) -> Vec<usize> {
        self.order
            .iter()
            .copied()
            .filter(|i| !self.columns[*i].hidden)
            .collect()
    }

    fn total_width(&self) -> f32 {
        self.visible_columns()
            .iter()
            .map(|i| self.columns[*i].width)
            .sum()
    }

    /// Scroll the lead row into view, moving as little as possible
    /// (HIG p. 227).
    pub fn reveal_lead(&mut self, cx: &mut Context<Self>) {
        let controller = self.controller.read(cx);
        let Some(index) = controller
            .selection()
            .and_then(|id| controller.visible_index_of(id))
        else {
            return;
        };
        let top = px(index as f32 * self.row_height);
        scroll_source::reveal(&self.scroll, Axis::Vertical, top, top + px(self.row_height));
        cx.notify();
    }

    fn emit_selection(&self, cx: &mut Context<Self>) {
        let ids = self.controller.read(cx).selected_ids().to_vec();
        cx.emit(TableEvent::SelectionChanged(ids));
    }

    fn click_row(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        self.finish_editing(true, window, cx);
        let Some(id) = self.id_at(index, cx) else {
            return;
        };
        let modifiers = event.modifiers;
        let secondary = modifiers.secondary(cx);
        let multiple = self.multiple;
        if event.button == MouseButton::Right {
            let already = self.controller.read(cx).is_selected(&id);
            if !already {
                self.controller.update(cx, |c, cx| {
                    c.select(id.clone());
                    cx.notify();
                });
                self.emit_selection(cx);
            }
            let ids = self.controller.read(cx).selected_ids().to_vec();
            cx.emit(TableEvent::ContextMenu {
                ids,
                position: event.position,
            });
            return;
        }
        let before = self.controller.read(cx).selected_ids().to_vec();
        self.controller.update(cx, |c, cx| {
            if multiple && modifiers.shift {
                c.extend_to(id.clone());
            } else if multiple && secondary {
                c.toggle(id.clone());
            } else {
                c.select(id.clone());
            }
            cx.notify();
        });
        if self.controller.read(cx).selected_ids() != before.as_slice() {
            self.emit_selection(cx);
        }
        if event.click_count == 2 && !modifiers.shift && !secondary {
            let ids = self.controller.read(cx).selected_ids().to_vec();
            cx.emit(TableEvent::Activated(ids));
        }
        cx.notify();
    }

    fn id_at(&self, index: usize, cx: &App) -> Option<ItemId> {
        let controller = self.controller.read(cx);
        controller
            .visible_item(index)
            .map(|item| controller.id_of(item))
    }

    /// Move the lead by `delta` rows (clamped); `extend` grows the range
    /// from the anchor.
    fn step(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let controller = self.controller.read(cx);
        let count = controller.visible_len();
        if count == 0 {
            return;
        }
        let current = controller
            .selection()
            .and_then(|id| controller.visible_index_of(id));
        let target = match current {
            None if delta >= 0 => 0,
            None => count - 1,
            Some(index) => (index as isize + delta).clamp(0, count as isize - 1) as usize,
        };
        self.select_index(target, extend, cx);
    }

    fn select_index(&mut self, index: usize, extend: bool, cx: &mut Context<Self>) {
        let Some(id) = self.id_at(index, cx) else {
            return;
        };
        let multiple = self.multiple;
        let before = self.controller.read(cx).selected_ids().to_vec();
        self.controller.update(cx, |c, cx| {
            if extend && multiple {
                c.extend_to(id);
            } else {
                c.select(id);
            }
            cx.notify();
        });
        if self.controller.read(cx).selected_ids() != before.as_slice() {
            self.emit_selection(cx);
        }
        self.reveal_lead(cx);
    }

    fn page_rows(&self) -> isize {
        let viewport = self.scroll.viewport().height.as_f32();
        ((viewport / self.row_height).floor() as isize - 1).max(1)
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            return;
        }
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        let extend = modifiers.shift;
        let handled = match keystroke.key.as_str() {
            "up" => {
                self.step(-1, extend, cx);
                true
            }
            "down" => {
                self.step(1, extend, cx);
                true
            }
            "home" => {
                self.select_index(0, extend, cx);
                true
            }
            "end" => {
                let last = self.controller.read(cx).visible_len().saturating_sub(1);
                self.select_index(last, extend, cx);
                true
            }
            "pageup" => {
                self.step(-self.page_rows(), extend, cx);
                true
            }
            "pagedown" => {
                self.step(self.page_rows(), extend, cx);
                true
            }
            "a" if modifiers.secondary(cx) && self.multiple => {
                self.controller.update(cx, |c, cx| {
                    c.select_all();
                    cx.notify();
                });
                self.emit_selection(cx);
                true
            }
            "enter" if !modifiers.modified() => {
                if !self.begin_editing_lead(window, cx) {
                    let ids = self.controller.read(cx).selected_ids().to_vec();
                    if !ids.is_empty() {
                        cx.emit(TableEvent::Activated(ids));
                    }
                }
                true
            }
            _ => false,
        };
        if handled {
            cx.stop_propagation();
            return;
        }
        if modifiers.control || modifiers.platform || modifiers.alt || modifiers.function {
            return;
        }
        let Some(typed) = keystroke.key_char.as_ref() else {
            return;
        };
        if typed.chars().any(char::is_control) {
            return;
        }
        cx.stop_propagation();
        self.type_select(typed, cx);
    }

    /// Select the first row whose type-select text starts with what has been
    /// typed since the last pause.
    fn type_select(&mut self, typed: &str, cx: &mut Context<Self>) {
        self.typed.push_str(typed);
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TYPE_SELECT_RESET).await;
            let _ = this.update(cx, |this, _| this.typed.clear());
        });
        self.typed_reset = Some(task);
        let Some(text) = self.type_select_column() else {
            return;
        };
        let prefix = self.typed.to_lowercase();
        let controller = self.controller.read(cx);
        let found = controller
            .visible()
            .position(|item| text(item).to_lowercase().starts_with(&prefix));
        if let Some(index) = found {
            self.select_index(index, false, cx);
        }
    }

    fn type_select_column(&self) -> Option<TextFn<T>> {
        let sorted = self.sort.as_ref().and_then(|(id, _)| {
            self.columns
                .iter()
                .find(|c| &c.id == id)
                .and_then(|c| c.text.clone())
        });
        sorted.or_else(|| {
            self.visible_columns()
                .into_iter()
                .find_map(|i| self.columns[i].text.clone())
        })
    }

    fn begin_editing_lead(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.controller.read(cx).selection().cloned() else {
            return false;
        };
        let Some(column) = self
            .visible_columns()
            .into_iter()
            .find(|i| self.columns[*i].commit.is_some() && self.columns[*i].text.is_some())
        else {
            return false;
        };
        let column_id = self.columns[column].id.clone();
        self.begin_editing(id, &column_id, window, cx)
    }

    /// Edit `column` of row `id` in place. Returns false when the column is
    /// not editable or the row is gone.
    pub fn begin_editing(
        &mut self,
        id: ItemId,
        column: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(col) = self.columns.iter().find(|c| c.id.as_ref() == column) else {
            return false;
        };
        let (Some(text), Some(_)) = (col.text.clone(), col.commit.as_ref()) else {
            return false;
        };
        let controller = self.controller.read(cx);
        let Some(item) = controller
            .visible()
            .find(|item| controller.id_of(item) == id)
        else {
            return false;
        };
        let initial = text(item);
        let field = cx.new(|cx| {
            let mut field = TextField::new(window, cx);
            field.set_text(initial.to_string(), cx);
            field
        });
        let subscription =
            cx.subscribe_in(&field, window, |this, _, event, window, cx| match event {
                TextFieldEvent::Submitted | TextFieldEvent::EndedEditing => {
                    this.finish_editing(true, window, cx);
                }
                TextFieldEvent::Changed => {}
            });
        field.update(cx, |field, cx| window.focus(&field.focus_handle(cx), cx));
        self.editing = Some(Editing {
            id,
            column: col.id.clone(),
            field,
            _subscription: subscription,
        });
        self.reveal_lead(cx);
        cx.notify();
        true
    }

    /// End an in-place edit: `commit` hands the text to the column's commit
    /// function; otherwise the edit is discarded.
    pub fn finish_editing(&mut self, commit: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.take() else {
            return;
        };
        if commit {
            let text = editing.field.read(cx).text().to_string();
            let accept = self
                .columns
                .iter()
                .find(|c| c.id == editing.column)
                .and_then(|c| c.commit.clone());
            let controller = self.controller.clone();
            let accepted = accept.is_some_and(|accept| {
                let item_found = {
                    let c = controller.read(cx);
                    c.visible().any(|item| c.id_of(item) == editing.id)
                };
                item_found && {
                    // The commit function may update the controller, so the
                    // row is looked up again inside the update.
                    let mut result = false;
                    controller.update(cx, |c, cx| {
                        if let Some(item) = c.visible().find(|item| c.id_of(item) == editing.id) {
                            result = accept(item, &text, cx);
                        }
                    });
                    result
                }
            });
            if accepted {
                cx.emit(TableEvent::Edited {
                    id: editing.id.clone(),
                    column: editing.column.clone(),
                });
            }
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn header_down(
        &mut self,
        event: &MouseDownEvent,
        origin_x: Pixels,
        width: Pixels,
        cx: &mut Context<Self>,
    ) {
        let x = event.position.x - origin_x;
        let visible = self.visible_columns();
        let mut left = px(0.);
        for (position, index) in visible.iter().copied().enumerate() {
            let column = &self.columns[index];
            let last = position + 1 == visible.len();
            let right = if last && self.fill_width {
                width
            } else {
                left + px(column.width)
            };
            if !(last && self.fill_width)
                && column.resizable
                && (x - right).abs() <= px(RESIZE_ZONE)
            {
                self.header_drag = Some(HeaderDrag::Resize {
                    column: index,
                    start_x: event.position.x,
                    start_width: column.width,
                });
                return;
            }
            if x >= left && x < right {
                self.header_drag = Some(HeaderDrag::Press {
                    column: index,
                    start_x: event.position.x,
                });
                return;
            }
            left = right;
        }
        let _ = cx;
    }

    fn header_move(&mut self, event: &MouseMoveEvent, origin_x: Pixels, cx: &mut Context<Self>) {
        match self.header_drag {
            Some(HeaderDrag::Resize {
                column,
                start_x,
                start_width,
            }) => {
                let c = &mut self.columns[column];
                c.width = (start_width + (event.position.x - start_x).as_f32())
                    .clamp(c.min_width, c.max_width);
                cx.notify();
            }
            Some(HeaderDrag::Press { column, start_x }) => {
                if (event.position.x - start_x).abs() > px(REORDER_THRESHOLD) {
                    self.header_drag = Some(HeaderDrag::Reorder { column });
                    self.header_move(event, origin_x, cx);
                }
            }
            Some(HeaderDrag::Reorder { column }) => {
                // Move the dragged column to the slot under the pointer.
                let x = event.position.x - origin_x;
                let visible = self.visible_columns();
                let mut left = px(0.);
                for slot in visible {
                    let right = left + px(self.columns[slot].width);
                    if x >= left && x < right && slot != column {
                        let from = self.order.iter().position(|i| *i == column);
                        let to = self.order.iter().position(|i| *i == slot);
                        if let (Some(from), Some(to)) = (from, to) {
                            self.order.remove(from);
                            self.order.insert(to, column);
                            cx.notify();
                        }
                        break;
                    }
                    left = right;
                }
            }
            None => {}
        }
    }

    fn header_up(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.header_drag.take() else {
            return;
        };
        match drag {
            HeaderDrag::Resize { .. } | HeaderDrag::Reorder { .. } => self.persist(cx),
            HeaderDrag::Press { column, .. } => {
                let id = self.columns[column].id.clone();
                if self.columns[column].sort.is_none() {
                    return;
                }
                let direction = match &self.sort {
                    Some((current, SortDirection::Ascending)) if *current == id => {
                        SortDirection::Descending
                    }
                    _ => SortDirection::Ascending,
                };
                self.set_sort(&id, direction, cx);
                cx.emit(TableEvent::Sorted {
                    column: id,
                    direction,
                });
            }
        }
    }

    fn render_header(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let header_height = self.header_height;
        let offset = if self.fill_width {
            px(0.)
        } else {
            self.scroll.position().x
        };
        let visible = self.visible_columns();
        let mut cells = div()
            .flex()
            .h_full()
            .when(self.fill_width, |el| el.w_full());
        if !self.fill_width {
            cells = cells.flex_none().ml(-offset);
        }
        for (position, index) in visible.iter().copied().enumerate() {
            let column = &self.columns[index];
            let last = position + 1 == visible.len();
            let fills = last && self.fill_width;
            let sorted = self
                .sort
                .as_ref()
                .filter(|(id, _)| *id == column.id)
                .map(|(_, d)| *d);
            let bevel = if sorted.is_some() {
                theme.controls.table_header_sorted
            } else {
                theme.controls.table_header
            };
            let width = column.width;
            let title = column.title.clone();
            let align = column.align;
            let arrow = theme.controls.sort_arrow;
            let active = window.is_window_active();
            cells = cells.child(
                div()
                    .id(SharedString::from(format!("table-header-{}", column.id)))
                    .debug_selector({
                        let id = column.id.clone();
                        move || format!("table-header-{id}")
                    })
                    .role(gpui::Role::ColumnHeader)
                    .aria_label(title.clone())
                    .relative()
                    .h_full()
                    .when(fills, |el| el.flex_1().min_w_0())
                    .when(!fills, |el| el.flex_none().w(px(width)))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                paint_header_cell(window, bounds, &bevel, sorted, arrow, active);
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .child(
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .h(px(header_height - 1.))
                            .px(px(CELL_INSET))
                            .when(sorted.is_some(), |el| el.pr(px(CELL_INSET + 14.)))
                            .map(|el| match align {
                                ColumnAlign::Leading => el.justify_start(),
                                ColumnAlign::Center => el.justify_center(),
                                ColumnAlign::Trailing => el.justify_end(),
                            })
                            .text_size(px(theme.type_.size_small))
                            .text_color(hsla(bevel.text))
                            .child(div().truncate().child(title)),
                    ),
            );
        }
        let view = cx.entity();
        let down_view = view.clone();
        let move_view = view.clone();
        let up_view = view.clone();
        div()
            .id("table-header")
            .debug_selector(|| "table-header".into())
            .flex_none()
            .w_full()
            .h(px(header_height))
            .overflow_hidden()
            .bg(hsla(theme.controls.table_header.upper.from))
            .child(cells)
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let origin_x = bounds.origin.x - offset;
                        let view = down_view.clone();
                        window.on_mouse_event(move |e: &MouseDownEvent, phase, _, cx| {
                            if phase != gpui::DispatchPhase::Bubble
                                || e.button != MouseButton::Left
                                || !bounds.contains(&e.position)
                            {
                                return;
                            }
                            cx.stop_propagation();
                            view.update(cx, |t, cx| {
                                t.header_down(e, origin_x, bounds.size.width, cx)
                            });
                        });
                        let view = move_view.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                            if phase != gpui::DispatchPhase::Capture {
                                return;
                            }
                            let dragging = view.read(cx).header_drag.is_some();
                            if dragging {
                                view.update(cx, |t, cx| t.header_move(e, origin_x, cx));
                            }
                        });
                        let view = up_view.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                            if phase == gpui::DispatchPhase::Capture
                                && view.read(cx).header_drag.is_some()
                            {
                                view.update(cx, |t, cx| t.header_up(cx));
                            }
                        });
                    },
                )
                .absolute()
                .inset_0(),
            )
            .into_any_element()
    }

    fn render_rows(
        &mut self,
        range: Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = cx.theme().clone();
        let focused = window.is_window_active() && self.focus.contains_focused(window, cx);
        let selection = if focused {
            theme.lists.table_active
        } else {
            theme.lists.table_inactive
        };
        let columns = self.visible_columns();
        let total = self.total_width();
        let row_height = self.row_height;
        let grid = self.grid;
        let grid_color = theme.controls.table_grid;
        let fill = self.fill_width;
        let viewport = self.scroll.viewport().width.as_f32();
        let editing = self
            .editing
            .as_ref()
            .map(|e| (e.id.clone(), e.column.clone(), e.field.clone()));
        let mut out = Vec::with_capacity(range.len());
        for index in range {
            let (id, selected) = {
                let controller = self.controller.read(cx);
                let Some(item) = controller.visible_item(index) else {
                    continue;
                };
                let id = controller.id_of(item);
                let selected = controller.is_selected(&id);
                (id, selected)
            };
            let text = if selected {
                selection.text
            } else {
                theme.colors.text
            };
            let context_base = CellContext {
                row: index,
                selected,
                emphasized: selected && focused,
                text,
                width: 0.0,
            };
            let mut row = div()
                .id(SharedString::from(format!("table-row-{id}")))
                .debug_selector({
                    let id = id.clone();
                    move || format!("table-row-{id}")
                })
                .role(gpui::Role::Row)
                .aria_selected(selected)
                .relative()
                .flex()
                .flex_none()
                .when(fill, |el| el.w_full())
                .when(!fill, |el| el.w(px(total)))
                .h(px(row_height))
                .text_size(px(theme.type_.size_view))
                .text_color(hsla(text))
                .bg(hsla(if self.alternating && index % 2 == 1 {
                    theme.colors.row_alternate
                } else {
                    theme.colors.content
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.click_row(index, e, window, cx);
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, e: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.click_row(index, e, window, cx);
                    }),
                );
            if selected {
                row = row.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| paint::selection(window, bounds, &selection),
                    )
                    .absolute()
                    .inset_0(),
                );
            }
            for (position, column_index) in columns.iter().enumerate() {
                let column = &self.columns[*column_index];
                let last = position + 1 == columns.len();
                let fills = last && fill;
                let mut width = column.width;
                if fills && viewport > 0.0 {
                    width = (viewport - (total - column.width)).max(0.0);
                }
                let context = CellContext {
                    width,
                    ..context_base
                };
                let edited = editing
                    .as_ref()
                    .filter(|(eid, ecol, _)| *eid == id && *ecol == column.id)
                    .map(|(_, _, field)| field.clone());
                let content: AnyElement = if let Some(field) = edited {
                    div().w_full().child(field).into_any_element()
                } else {
                    // Cells render while the controller is leased, so a
                    // cell renderer must not read the controller entity.
                    let cell = column.cell.clone();
                    let text = column.text.clone();
                    let rendered = self.controller.update(cx, |controller, cx| {
                        let item = controller.visible_item(index)?;
                        Some(match (&cell, &text) {
                            (Some(cell), _) => cell(item, &context, window, cx),
                            (None, Some(text)) => {
                                div().truncate().child(text(item)).into_any_element()
                            }
                            (None, None) => div().into_any_element(),
                        })
                    });
                    let Some(rendered) = rendered else {
                        continue;
                    };
                    rendered
                };
                let vertical_line = matches!(grid, GridLines::Vertical | GridLines::Both)
                    && position + 1 < columns.len();
                row = row.child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .overflow_hidden()
                        .h_full()
                        .when(fills, |el| el.flex_1().min_w_0())
                        .when(!fills, |el| el.flex_none().w(px(width)))
                        .px(px(CELL_INSET))
                        .map(|el| match column.align {
                            ColumnAlign::Leading => el.justify_start(),
                            ColumnAlign::Center => el.justify_center(),
                            ColumnAlign::Trailing => el.justify_end(),
                        })
                        .when(vertical_line, |el| {
                            el.border_r_1().border_color(hsla(grid_color))
                        })
                        .child(content),
                );
            }
            if matches!(grid, GridLines::Horizontal | GridLines::Both) {
                row = row.border_b_1().border_color(hsla(grid_color));
            }
            out.push(row.into_any_element());
        }
        out
    }
}

/// Header cell: two gradient bands, bottom line and right divider, plus the
/// sort triangle (M: table.header.sorted, table.header.plain,
/// table.header.sort_arrow, table.header.divider).
fn paint_header_cell(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    bevel: &silvestris_core::tokens::Bevel,
    sorted: Option<SortDirection>,
    arrow: silvestris_core::Rgba8,
    _active: bool,
) {
    let body_height = (bounds.size.height - px(1.)).max(px(0.));
    let upper_rows = (body_height.as_f32() * bevel.split).round();
    let upper = Bounds::new(bounds.origin, gpui::size(bounds.size.width, px(upper_rows)));
    let lower = Bounds::new(
        point(bounds.origin.x, bounds.origin.y + px(upper_rows)),
        gpui::size(bounds.size.width, body_height - px(upper_rows)),
    );
    window.paint_quad(gpui::fill(upper, rows(bevel.upper, upper_rows)));
    window.paint_quad(gpui::fill(
        lower,
        rows(bevel.lower, (body_height.as_f32() - upper_rows).max(1.0)),
    ));
    paint::hline(
        window,
        bounds.origin.x,
        bounds.bottom() - px(1.),
        bounds.size.width,
        bevel.border.to,
    );
    paint::vline(
        window,
        bounds.right() - px(1.),
        bounds.origin.y,
        body_height,
        bevel.border.from,
    );
    let Some(direction) = sorted else {
        return;
    };
    // 7 x 6 px, centred 7 px before the divider, rows 4..9 of the body.
    let centre_x = bounds.right() - px(1.) - px(7.) + px(0.5);
    let (apex, base) = match direction {
        SortDirection::Ascending => (bounds.origin.y + px(4.), bounds.origin.y + px(10.)),
        SortDirection::Descending => (bounds.origin.y + px(10.), bounds.origin.y + px(4.)),
    };
    let mut path = PathBuilder::fill();
    path.move_to(point(centre_x, apex));
    path.line_to(point(centre_x + px(3.5), base));
    path.line_to(point(centre_x - px(3.5), base));
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, hsla(arrow));
    }
}

impl<T: 'static> Render for TableView<T> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count = self.controller.read(cx).visible_len();
        let header_visible = self.header_visible;
        let fill = self.fill_width;
        let header = header_visible.then(|| self.render_header(window, cx));
        let content = cx.theme().colors.content;
        let list = uniform_list(
            "table-rows",
            count,
            cx.processor(|this, range: Range<usize>, window, cx| {
                this.render_rows(range, window, cx)
            }),
        )
        .with_horizontal_sizing_behavior(if fill {
            ListHorizontalSizingBehavior::FitList
        } else {
            ListHorizontalSizingBehavior::Unconstrained
        })
        .track_scroll(&self.scroll)
        .size_full();
        div()
            .id("table-view")
            .debug_selector(|| "table-view".into())
            .role(gpui::Role::Table)
            .key_context("SilvestrisTable")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .bg(hsla(content))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.key_down(event, window, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    // A click below the last row clears the selection.
                    window.focus(&this.focus, cx);
                    this.finish_editing(true, window, cx);
                    let had = !this.controller.read(cx).selected_ids().is_empty();
                    if had {
                        this.controller.update(cx, |c, cx| {
                            c.clear_selection();
                            cx.notify();
                        });
                        this.emit_selection(cx);
                    }
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    window.focus(&this.focus, cx);
                    this.finish_editing(true, window, cx);
                    cx.emit(TableEvent::BackgroundContextMenu {
                        position: event.position,
                    });
                    cx.stop_propagation();
                }),
            )
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.editing.is_some()
                    && event.keystroke.key == "escape"
                    && !event.keystroke.modifiers.modified()
                {
                    cx.stop_propagation();
                    this.finish_editing(false, window, cx);
                }
            }))
            .children(header)
            .child(
                div().flex_1().min_h_0().child(
                    ScrollView::new("table-scroll", &self.scroll)
                        .axes(ScrollAxes::Both)
                        .line(self.row_height)
                        .child(list),
                ),
            )
    }
}
