//! Outline view (#33).
//!
//! `OutlineView` is the hierarchical sibling of [`TableView`], not a source
//! list. It renders a flattened row list from an app-owned
//! [`ListController<OutlineRow<T>>`], so stable ids, selection, type-select,
//! inline rename, virtualization, scrolling and columns are the table's.
//!
//! The consumer owns the tree and the expansion set. It flattens rows and
//! supplies each row's depth and `has_children`/`expanded` state; the
//! disclosure callback reports a requested expansion change. This control
//! never reads a filesystem and is never capped at the source list's two
//! levels.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable, Point, Render,
    SharedString, Subscription, Window,
};

use super::disclosure::Disclosure;
use super::table_view::{CellContext, TableColumn, TableEvent, TableView};
use crate::binding::{ItemId, ListController};
use crate::ActiveTheme;

/// One flattened outline row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineRow<T> {
    pub id: ItemId,
    /// Zero for a root row.
    pub depth: usize,
    pub expanded: bool,
    pub has_children: bool,
    pub item: T,
}

impl<T> OutlineRow<T> {
    pub fn new(id: impl Into<ItemId>, depth: usize, item: T) -> Self {
        Self {
            id: id.into(),
            depth,
            expanded: false,
            has_children: false,
            item,
        }
    }

    pub fn branch(mut self, expanded: bool) -> Self {
        self.has_children = true;
        self.expanded = expanded;
        self
    }
}

/// Events produced by user interaction. Programmatic selection and expansion
/// never emit.
#[derive(Clone, Debug, PartialEq)]
pub enum OutlineEvent {
    SelectionChanged(Vec<ItemId>),
    Activated(Vec<ItemId>),
    ContextMenu {
        ids: Vec<ItemId>,
        position: Point<gpui::Pixels>,
    },
}

type ToggleFn = Rc<dyn Fn(&ItemId, bool, &mut Window, &mut App)>;
type RenameFn<T> = Rc<dyn Fn(&T, &str, &mut App) -> bool>;

pub struct OutlineView<T: 'static> {
    controller: Entity<ListController<OutlineRow<T>>>,
    table: Entity<TableView<OutlineRow<T>>>,
    _events: Subscription,
}

impl<T: 'static> OutlineView<T> {
    /// Build an outline. `label` names the first column and drives type-select
    /// and rename. `rename` commits an edited name and returns whether the
    /// value was accepted. `on_toggle` reports a disclosure request; the
    /// consumer changes its expansion set and calls [`set_rows`](Self::set_rows).
    pub fn new(
        controller: Entity<ListController<OutlineRow<T>>>,
        label: impl Fn(&T) -> SharedString + 'static,
        rename: Option<impl Fn(&T, &str, &mut App) -> bool + 'static>,
        on_toggle: impl Fn(&ItemId, bool, &mut Window, &mut App) + 'static,
        columns: Vec<TableColumn<OutlineRow<T>>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let label: Rc<dyn Fn(&T) -> SharedString> = Rc::new(label);
        let toggle: ToggleFn = Rc::new(on_toggle);
        let rename: Option<RenameFn<T>> = rename.map(|rename| Rc::new(rename) as RenameFn<T>);
        let indent = cx.theme().metrics.source_indent;

        let name_label = label.clone();
        let name_toggle = toggle.clone();
        let mut name = TableColumn::custom(
            "name",
            "Name",
            move |row: &OutlineRow<T>, cell: &CellContext, _window, _cx| {
                let row_label = name_label(&row.item);
                let disclosure = row.has_children.then(|| {
                    let row_id = row.id.clone();
                    let toggle = name_toggle.clone();
                    Disclosure::new(
                        SharedString::from(format!("outline-disclosure-{}", row.id)),
                        row_label.clone(),
                    )
                    .expanded(row.expanded)
                    .selected(cell.selected)
                    .on_toggle(move |next, window, cx| toggle(&row_id, *next, window, cx))
                });
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .pl(px(5. + row.depth as f32 * indent))
                    .overflow_hidden()
                    .children(disclosure)
                    .child(div().truncate().child(row_label))
                    .into_any_element()
            },
        )
        .text_value({
            let label = label.clone();
            move |row: &OutlineRow<T>| label(&row.item)
        });
        if let Some(rename) = rename.clone() {
            name = name.editable(move |row: &OutlineRow<T>, text, cx| rename(&row.item, text, cx));
        }

        let mut all_columns = Vec::with_capacity(columns.len() + 1);
        all_columns.push(name);
        all_columns.extend(columns);
        let table = cx.new(|cx| {
            let mut table = TableView::new(controller.clone(), all_columns, cx);
            // The consumer flattens and sorts each level; the table must not
            // reorder the tree.
            table.set_external_sort(true, cx);
            table
        });
        let events = cx.subscribe(&table, |_, _, event: &TableEvent, cx| match event {
            TableEvent::SelectionChanged(ids) => {
                cx.emit(OutlineEvent::SelectionChanged(ids.clone()));
            }
            TableEvent::Activated(ids) => cx.emit(OutlineEvent::Activated(ids.clone())),
            TableEvent::ContextMenu { ids, position } => {
                cx.emit(OutlineEvent::ContextMenu {
                    ids: ids.clone(),
                    position: *position,
                });
            }
            TableEvent::BackgroundContextMenu { .. }
            | TableEvent::Sorted { .. }
            | TableEvent::Edited { .. } => {}
        });
        Self {
            controller,
            table,
            _events: events,
        }
    }

    /// Replace the flattened rows. Selection survives by stable id.
    pub fn set_rows(&mut self, rows: Vec<OutlineRow<T>>, cx: &mut Context<Self>) {
        self.controller.update(cx, |controller, cx| {
            controller.set_items(rows);
            cx.notify();
        });
        cx.notify();
    }

    pub fn controller(&self) -> Entity<ListController<OutlineRow<T>>> {
        self.controller.clone()
    }

    pub fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.table.read(cx).focus_handle(cx)
    }

    pub fn set_row_height(&mut self, height: f32, cx: &mut Context<Self>) {
        self.table
            .update(cx, |table, cx| table.set_row_height(height, cx));
    }

    /// Let rows offer `TableDrag` values to GPUI.
    pub fn set_draggable(&mut self, draggable: bool, cx: &mut Context<Self>) {
        self.table
            .update(cx, |table, cx| table.set_draggable(draggable, cx));
    }

    pub fn begin_editing(
        &mut self,
        id: ItemId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.table.update(cx, |table, cx| {
            table.begin_editing(id, &SharedString::from("name"), window, cx)
        })
    }

    pub fn is_editing(&self, cx: &App) -> bool {
        self.table.read(cx).is_editing()
    }
}

impl<T: 'static> EventEmitter<OutlineEvent> for OutlineView<T> {}

impl<T: 'static> Render for OutlineView<T> {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.table.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{TestAppContext, VisualTestContext};

    #[derive(Clone, PartialEq)]
    struct Item {
        id: &'static str,
        label: &'static str,
    }

    struct Harness {
        outline: Entity<OutlineView<Item>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.outline.clone())
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }

    #[test]
    fn rows_render_with_disclosure_and_table_columns() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let outline = app.update(|cx| {
            let controller =
                cx.new(|_| ListController::new(|row: &OutlineRow<Item>| row.id.clone()));
            cx.new(|cx| {
                let mut view = OutlineView::new(
                    controller,
                    |item: &Item| item.label.into(),
                    Some(|_: &Item, _: &str, _: &mut App| false),
                    |_, _, _, _| {},
                    vec![TableColumn::text(
                        "kind",
                        "Kind",
                        |_row: &OutlineRow<Item>| "Item".into(),
                    )],
                    cx,
                );
                view.set_rows(
                    vec![
                        OutlineRow::new(
                            "root",
                            0,
                            Item {
                                id: "root",
                                label: "Root",
                            },
                        )
                        .branch(true),
                        OutlineRow::new(
                            "child",
                            1,
                            Item {
                                id: "child",
                                label: "Child",
                            },
                        ),
                    ],
                    cx,
                );
                view
            })
        });
        let (_harness, cx) = app.add_window_view(move |_, _| Harness { outline });
        draw(cx);
        assert!(cx.debug_bounds("table-view").is_some());
        assert!(cx.debug_bounds("outline-disclosure-root").is_some());
    }
}
