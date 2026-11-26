# Outline view

`controls::OutlineView` is the hierarchical sibling of `TableView` (#33), not a
`SourceList`. A source list is a navigation sidebar capped at two levels and
owns blue/white navigation semantics; an outline is a content view of arbitrary
depth whose rows are table rows.

The consumer owns the tree and the expansion set. It flattens the visible
branches into `OutlineRow<T>` values and the view renders them through
`TableView`, so stable ids, selection, type-select, inline rename, columns,
sorting suppression, virtualisation and scrolling are the table's contracts.

```rust
let outline = cx.new(|cx| {
    let mut outline = OutlineView::new(
        controller,                       // Entity<ListController<OutlineRow<T>>>
        |node: &Node| node.name.clone(),  // label, type-select and rename text
        Some(|node: &Node, new_name: &str, cx: &mut App| rename(node, new_name, cx)),
        |id, expanded, window, cx| expand_in_the_consumer(id, expanded, window, cx),
        vec![TableColumn::text("kind", "Kind", |row: &OutlineRow<Node>| {
            row.item.kind.clone()
        })],
        cx,
    );
    outline.set_rows(flatten(&tree, &expanded));
    outline
});
```

## Rows and disclosure

`OutlineRow::new(id, depth, item)` is a leaf; `.branch(expanded)` marks a row
with children and sets the triangle direction. The view adds a leading `Name`
column: a disclosure triangle when `has_children`, then the label. `depth`
indents by the theme's `source_indent` (14 px in Aqua), with the same 5 px
leading edge as a table cell.

`on_toggle(id, requested_expanded, window, cx)` reports the requested state. It
does not mutate the tree, does not select the row and does not emit an outline
event. The consumer changes its expansion set and calls `set_rows`, which
routes through `ListController::set_items`; selection survives by stable id.

## Table behaviour

- Columns after `Name` are ordinary `TableColumn<OutlineRow<T>>` values.
- `set_external_sort(true)` is always on: the consumer flattens each level in
  its own order, so a header click cannot reorder the tree.
- `OutlineEvent::SelectionChanged`, `Activated` and `ContextMenu` mirror the
  table's events. Programmatic selection and `set_rows` never emit.
- `begin_editing`, `is_editing` and the table's Return/Escape commit rules give
  inline rename through the `rename` callback.
- `set_row_height`, `focus_handle` and `controller` expose the table's host
  hooks.

The view never reads a filesystem and is not capped at a fixed depth. It is
distinct from `SourceList`; see `docs/source-list.md` for the sidebar
boundary and `hig-09/specs/source-list.md:8-84` for the two-level rule.

## DnD

`OutlineView` forwards the table's drag contract. A consumer that needs to
drag rows matches `TableDrag` with `on_drag_move::<TableDrag>` on its drop
target.
