//! Column browser (#34; HIG Column Views p. 334, The Open Dialog p. 245).
//!
//! Hierarchical navigation as a row of single-column lists: selecting a
//! branch opens its children in a new column to the right, which scrolls
//! into view. Each column is a headerless [`TableView`], so selection,
//! keyboard movement, type-select and virtualisation are the table's.
//!
//! The browser never reads a filesystem or any other store. It asks for a
//! branch's children with [`BrowserEvent::NeedsChildren`]; the app (or an
//! Open panel's file provider) answers with
//! [`set_children`](ColumnBrowser::set_children), now or later. Columns to
//! the left never move while a column loads.
//!
//! - Left and Right move between columns; Right enters the first child.
//! - The focused column's selection is blue; columns left of it keep their
//!   selection in grey, as in 10.6.
//! - A leaf's preview column comes from [`set_preview`](ColumnBrowser::set_preview).
//! - Each column has a resize handle at the bottom of its divider
//!   (M: browser.grip: two 7 px lines 3 px apart, 5 px above the divider's
//!   bottom; M: browser.arrow: the 7 x 6 px branch triangle).
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{
    canvas, div, point, prelude::*, px, AnyElement, App, Axis, Context, Entity, EventEmitter,
    FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PathBuilder, Pixels, ScrollHandle, SharedString, Subscription, Window,
};

use super::scroll_source;
use super::scroll_view::{ScrollAxes, ScrollView};
use super::table_view::{TableColumn, TableEvent, TableView};
use crate::binding::{ItemId, ListController};
use crate::convert::hsla;
use crate::{paint, ActiveTheme};

type IconFn = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;
type PreviewFn = Rc<dyn Fn(&BrowserNode, &mut Window, &mut App) -> AnyElement>;
type RenameFn = Rc<dyn Fn(&ItemId, &ItemId, &str, &mut App) -> bool>;

/// One entry in a column.
#[derive(Clone)]
pub struct BrowserNode {
    pub id: ItemId,
    pub label: SharedString,
    /// Has children: selecting it opens a column, and the row shows the
    /// branch arrow.
    pub branch: bool,
    pub icon: Option<IconFn>,
}

impl BrowserNode {
    pub fn leaf(id: impl Into<ItemId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            branch: false,
            icon: None,
        }
    }

    pub fn branch(id: impl Into<ItemId>, label: impl Into<SharedString>) -> Self {
        Self {
            branch: true,
            ..Self::leaf(id, label)
        }
    }

    pub fn icon(mut self, icon: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> Self {
        self.icon = Some(Rc::new(icon));
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrowserEvent {
    /// Show this branch's children with `set_children`.
    NeedsChildren(ItemId),
    /// The user changed the selected path (root first).
    PathChanged(Vec<ItemId>),
    /// Double-click or Return on a leaf.
    Activated(ItemId),
    /// Secondary click on a row: the selection to act on and the window
    /// position for a contextual menu.
    ContextMenu {
        ids: Vec<ItemId>,
        position: gpui::Point<gpui::Pixels>,
    },
    /// Secondary click on a column's empty area.
    BackgroundContextMenu { position: gpui::Point<gpui::Pixels> },
}

struct Column {
    /// `None` for the root column.
    parent: Option<ItemId>,
    list: Entity<ListController<BrowserNode>>,
    table: Entity<TableView<BrowserNode>>,
    width: f32,
    loaded: bool,
    _events: Subscription,
}

pub struct ColumnBrowser {
    columns: Vec<Column>,
    /// Children already supplied, by parent id (the root is ""). Shared
    /// immutably; a column's controller is the only per-column copy.
    children: HashMap<ItemId, Rc<[BrowserNode]>>,
    /// Recency for [`children`](Self::children); the current path and the most
    /// recent branches stay, cold ones are dropped.
    children_used: HashMap<ItemId, u64>,
    clock: u64,
    preview: Option<PreviewFn>,
    rename: Option<RenameFn>,
    column_width: f32,
    scroll: ScrollHandle,
    focus: FocusHandle,
    resizing: Option<(usize, Pixels, f32)>,
    /// Forwarded to every column table so rows offer `TableDrag`.
    draggable: bool,
}

impl EventEmitter<BrowserEvent> for ColumnBrowser {}

impl Focusable for ColumnBrowser {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

pub const ROOT: &str = "";
/// M: browser.columns: 186 px from column edge to column edge; the trailing
/// 15 px are the column's scroller track.
pub const COLUMN_WIDTH: f32 = 186.0;
/// Estimate: narrowest column that keeps a readable label.
const MIN_WIDTH: f32 = 100.0;
/// M: browser.grip: the two resize lines sit in the bottom 15 px of the
/// divider.
const GRIP_HIT: f32 = 15.0;
/// M: browser.grip: each line is 7 rows tall, their gap is 3 px.
const GRIP_LINE: f32 = 7.0;
const GRIP_GAP: f32 = 3.0;
/// M: browser.grip: the lines start 5 rows above the browser's bottom edge.
const GRIP_INSET: f32 = 5.0;
/// M: browser.rows: 19 px rows, as in the Finder list view.
const ROW_HEIGHT: f32 = 19.0;
/// Branch listings kept in memory: the current path plus a small recent set.
/// Repeated browsing must not retain every directory ever visited.
pub const MAX_CACHED_BRANCHES: usize = 32;

impl ColumnBrowser {
    pub fn new(roots: Vec<BrowserNode>, cx: &mut Context<Self>) -> Self {
        let mut browser = Self {
            columns: Vec::new(),
            children: HashMap::new(),
            children_used: HashMap::new(),
            clock: 0,
            preview: None,
            rename: None,
            column_width: COLUMN_WIDTH,
            scroll: ScrollHandle::new(),
            focus: cx.focus_handle(),
            resizing: None,
            draggable: false,
        };
        browser.children.insert(ROOT.into(), Rc::from(roots));
        browser.touch(ROOT.into());
        browser.push_column(None, cx);
        browser
    }

    /// Render a preview column for the selected leaf.
    pub fn set_preview(
        &mut self,
        preview: impl Fn(&BrowserNode, &mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.preview = Some(Rc::new(preview));
        cx.notify();
    }

    /// Let the name column commit an inline edit through `rename(parent, id,
    /// new_name)`. Call it before the browser is first rendered; replacing the
    /// callback rebuilds the root column and drops any selection.
    pub fn set_rename(
        &mut self,
        rename: impl Fn(&ItemId, &ItemId, &str, &mut App) -> bool + 'static,
        cx: &mut Context<Self>,
    ) {
        self.rename = Some(Rc::new(rename));
        if !self.columns.is_empty() {
            let roots = self.children.get(ROOT).cloned().unwrap_or_default();
            self.children.insert(ROOT.into(), roots);
            self.columns.clear();
            self.push_column(None, cx);
        }
        cx.notify();
    }

    /// Let every column's rows offer [`crate::controls::TableDrag`] values to
    /// GPUI. The consumer owns the drop targets and the resulting operation.
    pub fn set_draggable(&mut self, draggable: bool, cx: &mut Context<Self>) {
        self.draggable = draggable;
        for column in &self.columns {
            column
                .table
                .update(cx, |table, cx| table.set_draggable(draggable, cx));
        }
    }

    /// Start editing `id`'s name in whichever column shows it. Returns whether
    /// the row was found and the table accepted the edit.
    pub fn begin_editing(&self, id: &ItemId, window: &mut Window, cx: &mut App) -> bool {
        for column in &self.columns {
            if column.list.read(cx).visible_index_of(id).is_some() {
                return column.table.update(cx, |table, cx| {
                    table.begin_editing(id.clone(), "name", window, cx)
                });
            }
        }
        false
    }

    /// Whether any column is editing a name.
    pub fn is_editing(&self, cx: &App) -> bool {
        self.columns
            .iter()
            .any(|column| column.table.read(cx).is_editing())
    }

    /// Supply (or replace) `parent`'s children. An open column for `parent`
    /// fills in place; its selection survives by id. A late answer for a
    /// column that has since been truncated is cached only and does not
    /// re-open it.
    pub fn set_children(&mut self, parent: &str, nodes: Vec<BrowserNode>, cx: &mut Context<Self>) {
        let key: ItemId = parent.to_string().into();
        let shared: Rc<[BrowserNode]> = Rc::from(nodes);
        self.children.insert(key.clone(), shared.clone());
        self.touch(key.clone());
        for column in &mut self.columns {
            let matches = match &column.parent {
                None => parent == ROOT,
                Some(id) => *id == key,
            };
            if matches {
                column.loaded = true;
                column.list.update(cx, |list, cx| {
                    list.set_items(shared.to_vec());
                    cx.notify();
                });
            }
        }
        self.prune_children();
        cx.notify();
    }

    /// Children supplied for `parent`, if any.
    pub fn children_of(&self, parent: &str) -> Option<&[BrowserNode]> {
        self.children.get(parent).map(|nodes| nodes.as_ref())
    }

    /// How many branch listings the cache currently holds. Bounded by
    /// [`MAX_CACHED_BRANCHES`] plus the open path.
    #[must_use]
    pub fn cached_branch_count(&self) -> usize {
        self.children.len()
    }

    /// Ids currently shown in any column, used to bound consumer-side entry
    /// maps to what can still be acted on.
    #[must_use]
    pub fn visible_ids(&self, cx: &App) -> Vec<ItemId> {
        let mut ids = Vec::new();
        for column in &self.columns {
            let list = column.list.read(cx);
            ids.extend(list.visible().map(|node| node.id.clone()));
        }
        ids
    }

    fn touch(&mut self, key: ItemId) {
        self.clock = self.clock.wrapping_add(1);
        self.children_used.insert(key, self.clock);
    }

    /// Keep the root, every open column's parent and a small recent set.
    /// A late answer for a truncated column may still be cached, but the cache
    /// cannot grow with the session.
    fn prune_children(&mut self) {
        if self.children.len() <= MAX_CACHED_BRANCHES {
            return;
        }
        let mut keep: std::collections::HashSet<ItemId> = std::collections::HashSet::new();
        keep.insert(ROOT.into());
        for column in &self.columns {
            keep.insert(column.parent.clone().unwrap_or_else(|| ROOT.into()));
        }
        let mut recent: Vec<(u64, ItemId)> = self
            .children
            .keys()
            .filter(|key| !keep.contains(*key))
            .map(|key| {
                (
                    self.children_used.get(key).copied().unwrap_or(0),
                    key.clone(),
                )
            })
            .collect();
        recent.sort_by_key(|(used, _)| std::cmp::Reverse(*used));
        let room = MAX_CACHED_BRANCHES.saturating_sub(keep.len());
        for (_, key) in recent.into_iter().take(room) {
            keep.insert(key);
        }
        self.children.retain(|key, _| keep.contains(key));
        self.children_used.retain(|key, _| keep.contains(key));
    }

    /// The selected path, root first.
    pub fn path(&self, cx: &App) -> Vec<ItemId> {
        self.columns
            .iter()
            .filter_map(|c| c.list.read(cx).selection().cloned())
            .collect()
    }

    /// Select `path` programmatically (no `PathChanged`). Stops at the first
    /// id that is not present; missing children are requested.
    pub fn set_path(&mut self, path: &[ItemId], cx: &mut Context<Self>) {
        self.columns.truncate(1);
        for (depth, id) in path.iter().enumerate() {
            let Some(column) = self.columns.get(depth) else {
                break;
            };
            let found = column.list.read(cx).visible_index_of(id).is_some();
            if !found {
                break;
            }
            column.list.update(cx, |list, cx| {
                list.select(id.clone());
                cx.notify();
            });
            self.open_after(depth, cx);
        }
        self.reveal_last(cx);
        cx.notify();
    }

    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Whether column `index` has its children.
    pub fn is_loaded(&self, index: usize) -> bool {
        self.columns.get(index).is_some_and(|column| column.loaded)
    }

    pub fn set_column_width(&mut self, width: f32, cx: &mut Context<Self>) {
        self.column_width = width.max(MIN_WIDTH);
        for column in &mut self.columns {
            column.width = self.column_width;
        }
        cx.notify();
    }

    /// The table showing column `index`, for focus and tests.
    pub fn column_table(&self, index: usize) -> Option<Entity<TableView<BrowserNode>>> {
        self.columns.get(index).map(|c| c.table.clone())
    }

    /// Focus column `index`, creating no column that does not exist.
    pub fn focus_column(&self, index: usize, window: &mut Window, cx: &mut App) -> bool {
        let Some(column) = self.columns.get(index) else {
            return false;
        };
        let handle = column.table.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
        true
    }

    /// Focus the first column.
    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        self.focus_column(0, window, cx);
    }

    fn push_column(&mut self, parent: Option<ItemId>, cx: &mut Context<Self>) {
        let key: ItemId = parent.clone().unwrap_or_else(|| ROOT.into());
        let known = self.children.get(&key).cloned();
        let loaded = known.is_some();
        let list = cx.new(|_| {
            let mut list = ListController::new(|n: &BrowserNode| n.id.clone());
            list.set_items(known.map(|nodes| nodes.to_vec()).unwrap_or_default());
            list
        });
        let rename_parent: ItemId = parent.clone().unwrap_or_else(|| ROOT.into());
        let rename = self.rename.clone();
        let mut name_column =
            TableColumn::custom("name", "", |n: &BrowserNode, cell, window, cx| {
                browser_row(n, cell.text, window, cx)
            })
            .text_value(|n: &BrowserNode| n.label.clone())
            .width(10_000.);
        if let Some(rename) = rename {
            name_column = name_column.editable(move |node: &BrowserNode, new_name, cx| {
                rename(&rename_parent, &node.id, new_name, cx)
            });
        }
        let columns = vec![name_column];
        let draggable = self.draggable;
        let table = cx.new(|cx| {
            let mut table = TableView::new(list.clone(), columns, cx);
            table.set_header_visible(false, cx);
            table.set_alternating(false, cx);
            table.set_allows_multiple(false);
            table.set_row_height(ROW_HEIGHT, cx);
            table.set_fill_width(true, cx);
            table.set_draggable(draggable, cx);
            table
        });
        let depth = self.columns.len();
        let events = cx.subscribe(&table, move |browser, _, event: &TableEvent, cx| {
            browser.table_event(depth, event, cx);
        });
        self.columns.push(Column {
            parent: parent.clone(),
            list,
            table,
            width: self.column_width,
            loaded,
            _events: events,
        });
        // Cached children paint immediately, but the app is asked to refresh
        // them: reopening a branch must not resurrect stale rows, and the
        // panel rebuilds its id-to-entry map from the answer.
        if let Some(parent) = parent {
            cx.emit(BrowserEvent::NeedsChildren(parent));
        }
        self.prune_children();
    }

    /// Rebuild the columns right of `depth` from its selection.
    fn open_after(&mut self, depth: usize, cx: &mut Context<Self>) {
        self.columns.truncate(depth + 1);
        let selected = self.columns[depth].list.read(cx).selected().cloned();
        if let Some(node) = selected {
            if node.branch {
                self.push_column(Some(node.id.clone()), cx);
            }
        }
    }

    fn table_event(&mut self, depth: usize, event: &TableEvent, cx: &mut Context<Self>) {
        if depth >= self.columns.len() {
            return;
        }
        match event {
            TableEvent::SelectionChanged(_) => {
                self.open_after(depth, cx);
                self.reveal_last(cx);
                let path = self.path(cx);
                cx.emit(BrowserEvent::PathChanged(path));
                cx.notify();
            }
            TableEvent::Activated(ids) => {
                let leaf = self.columns[depth]
                    .list
                    .read(cx)
                    .selected()
                    .filter(|n| !n.branch)
                    .map(|n| n.id.clone());
                if let (Some(id), Some(_)) = (leaf, ids.first()) {
                    cx.emit(BrowserEvent::Activated(id));
                }
            }
            TableEvent::ContextMenu { ids, position } => {
                cx.emit(BrowserEvent::ContextMenu {
                    ids: ids.clone(),
                    position: *position,
                });
            }
            TableEvent::BackgroundContextMenu { position } => {
                cx.emit(BrowserEvent::BackgroundContextMenu {
                    position: *position,
                });
            }
            _ => {}
        }
    }

    /// Scroll so the newest column (and preview) is visible. The content
    /// width is known after layout, so this repeats on the next turn.
    fn reveal_last(&mut self, cx: &mut Context<Self>) {
        let mut right = 0.0;
        for column in &self.columns {
            right += column.width;
        }
        if self.preview_node(cx).is_some() {
            right += self.column_width;
        }
        let start = (right - self.column_width).max(0.0);
        let scroll = self.scroll.clone();
        scroll_source::reveal(&scroll, Axis::Horizontal, px(start), px(right));
        cx.defer(move |_| {
            scroll_source::reveal(&scroll, Axis::Horizontal, px(start), px(right));
        });
    }

    fn preview_node(&self, cx: &App) -> Option<BrowserNode> {
        self.preview.as_ref()?;
        let last = self.columns.last()?;
        last.list.read(cx).selected().filter(|n| !n.branch).cloned()
    }

    fn focused_column(&self, window: &Window, cx: &App) -> Option<usize> {
        self.columns.iter().position(|c| {
            c.table
                .read(cx)
                .focus_handle(cx)
                .contains_focused(window, cx)
        })
    }

    fn key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.keystroke.modifiers.modified() {
            return;
        }
        let Some(depth) = self.focused_column(window, cx) else {
            return;
        };
        match event.keystroke.key.as_str() {
            "right" => {
                cx.stop_propagation();
                if depth + 1 >= self.columns.len() {
                    self.open_after(depth, cx);
                }
                let Some(next) = self.columns.get(depth + 1) else {
                    return;
                };
                let first = next.list.read(cx).visible_item(0).map(|n| n.id.clone());
                let table = next.table.clone();
                let list = next.list.clone();
                window.focus(&table.read(cx).focus_handle(cx), cx);
                if let Some(first) = first {
                    if list.read(cx).selection().is_none() {
                        list.update(cx, |l, cx| {
                            l.select(first);
                            cx.notify();
                        });
                        self.open_after(depth + 1, cx);
                        let path = self.path(cx);
                        cx.emit(BrowserEvent::PathChanged(path));
                    }
                }
                self.reveal_last(cx);
                cx.notify();
            }
            "left" => {
                if depth == 0 {
                    return;
                }
                cx.stop_propagation();
                // Leave the column: its selection clears and the parent's
                // stays (10.6 column view).
                self.columns[depth].list.update(cx, |l, cx| {
                    l.clear_selection();
                    cx.notify();
                });
                self.columns.truncate(depth + 1);
                let table = self.columns[depth - 1].table.clone();
                window.focus(&table.read(cx).focus_handle(cx), cx);
                let path = self.path(cx);
                cx.emit(BrowserEvent::PathChanged(path));
                cx.notify();
            }
            _ => {}
        }
    }
}

/// A browser row: icon, label, and the branch arrow at the right edge
/// (M: browser.arrow: 7 x 6 px triangle).
fn browser_row(
    node: &BrowserNode,
    text: silvestris_core::Rgba8,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let arrow = text;
    let icon = node.icon.as_ref().map(|icon| icon(window, cx));
    div()
        .flex()
        .items_center()
        .w_full()
        .gap(px(4.))
        .children(icon.map(|icon| div().flex_none().child(icon)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(node.label.clone()),
        )
        .when(node.branch, |el| {
            el.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let x = bounds.right() - px(1.);
                        let mid = bounds.center().y;
                        let mut path = PathBuilder::fill();
                        path.move_to(point(x, mid));
                        path.line_to(point(x - px(6.), mid - px(3.)));
                        path.line_to(point(x - px(6.), mid + px(3.)));
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, hsla(arrow));
                        }
                    },
                )
                .flex_none()
                .w(px(7.))
                .h(px(6.)),
            )
        })
        .into_any_element()
}

impl Render for ColumnBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let divider = theme.controls.scroller.track_border;
        let grip = theme.colors.text_secondary;
        let mut strip = div().flex().h_full();
        for (index, column) in self.columns.iter().enumerate() {
            let width = column.width;
            let loading = !column.loaded;
            strip = strip.child(
                div()
                    .id(("browser-column", index))
                    .debug_selector(move || format!("browser-column-{index}"))
                    .relative()
                    .flex_none()
                    .w(px(width))
                    .h_full()
                    .border_r_1()
                    .border_color(hsla(divider))
                    .child(column.table.clone())
                    .when(loading, |el| {
                        el.child(
                            div()
                                .absolute()
                                .top(px(4.))
                                .left(px(8.))
                                .text_size(px(theme.type_.size_small))
                                .text_color(hsla(theme.colors.text_secondary))
                                .child("Loading\u{2026}"),
                        )
                    })
                    .child(
                        // Resize grip: two short lines at the bottom of the
                        // divider (M: browser.grip).
                        div()
                            .id(("browser-grip", index))
                            .debug_selector(move || format!("browser-grip-{index}"))
                            .absolute()
                            .right(px(0.))
                            .bottom(px(0.))
                            .w(px(11.))
                            .h(px(GRIP_HIT))
                            .cursor(gpui::CursorStyle::ResizeLeftRight)
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, _, window, _| {
                                        let y = bounds.bottom() - px(GRIP_INSET + GRIP_LINE);
                                        let x = bounds.right() - px(5.);
                                        paint::vline(window, x, y, px(GRIP_LINE), grip);
                                        paint::vline(
                                            window,
                                            x - px(GRIP_GAP),
                                            y,
                                            px(GRIP_LINE),
                                            grip,
                                        );
                                    },
                                )
                                .size_full(),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |b, e: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    b.resizing =
                                        Some((index, e.position.x, b.columns[index].width));
                                }),
                            ),
                    ),
            );
        }
        if let Some(node) = self.preview_node(cx) {
            if let Some(preview) = self.preview.clone() {
                strip = strip.child(
                    div()
                        .debug_selector(|| "browser-preview".into())
                        .flex_none()
                        .w(px(self.column_width))
                        .h_full()
                        .p(px(8.))
                        .child(preview(&node, window, cx)),
                );
            }
        }
        let content = theme.colors.content;
        div()
            .id("column-browser")
            .debug_selector(|| "column-browser".into())
            .key_context("SilvestrisColumnBrowser")
            .track_focus(&self.focus)
            .size_full()
            .bg(hsla(content))
            .on_key_down(cx.listener(|b, e: &KeyDownEvent, window, cx| b.key_down(e, window, cx)))
            .on_mouse_move(cx.listener(|b, e: &MouseMoveEvent, _, cx| {
                if let Some((index, start_x, start_width)) = b.resizing {
                    let width = (start_width + (e.position.x - start_x).as_f32())
                        .clamp(MIN_WIDTH, COLUMN_WIDTH * 3.0);
                    if let Some(column) = b.columns.get_mut(index) {
                        column.width = width;
                        cx.notify();
                    }
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|b, _: &MouseUpEvent, _, _| b.resizing = None),
            )
            .child(
                ScrollView::new("browser-scroll", &self.scroll)
                    .axes(ScrollAxes::Horizontal)
                    .child(
                        div()
                            .id("browser-strip")
                            .size_full()
                            .overflow_x_scroll()
                            .track_scroll(&self.scroll)
                            .child(strip),
                    ),
            )
    }
}
