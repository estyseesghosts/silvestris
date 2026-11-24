# Table view

`controls::TableView<T>` is the list view of HIG pp. 328 and 333: column
headers, alternating rows, single or multiple selection, sorting, in-place
editing and virtualised rows. It renders from an app-owned
`Entity<ListController<T>>` (#61); the controller holds the items, the sort,
the filter and the selection by id. The table never owns data.

```rust
let files = cx.new(|_| {
    let mut c = ListController::new(|f: &FileEntry| f.id.clone());
    c.set_items(entries);
    c
});
let table = cx.new(|cx| {
    let columns = vec![
        TableColumn::custom("name", "Name", |f: &FileEntry, cell, _, _| {
            icon_text(file_icon(f), f.name.clone())
        })
        .text_value(|f| f.name.clone())
        .width(220.)
        .sort_by_text()
        .editable(|f, name, cx| rename(f, name, cx)),
        TableColumn::text("modified", "Date Modified", |f| f.modified_label())
            .width(160.)
            .sortable(|a, b| a.modified.cmp(&b.modified)),
        TableColumn::text("size", "Size", |f| f.size_label())
            .width(70.)
            .align(ColumnAlign::Trailing),
    ];
    let mut table = TableView::new(files.clone(), columns, cx);
    table.set_autosave(column_store, "files.list", cx);
    table
});
cx.subscribe(&table, |_, _, event: &TableEvent, cx| { /* … */ }).detach();
```

## Columns

- `TableColumn::text` takes a text accessor; `TableColumn::custom` takes a
  cell renderer. Put existing controls in custom cells (`Checkbox`,
  `PopUpButton`, `ProgressIndicator`) or use `icon_text` for the Finder
  Name cell. `CellContext` says whether the row is selected and gives the
  text colour on the selection.
- Cell renderers run while the controller entity is leased: they must not
  read the controller.
- Clicking a sortable header sorts ascending, clicking again sorts
  descending. The sorted header is blue with a triangle
  (M: `table.header.sorted`, `table.header.sort_arrow`). Sorting goes
  through the controller, so the selection stays on the same ids, and the
  lead row is revealed.
- Drag a header divider to resize within `min_width`..`max_width`. Drag a
  header cell more than 4 px to reorder. `set_column_hidden` hides a column.
- `set_autosave(store, key)` restores and then persists order, widths and
  hidden columns through a `ColumnStore`. `PreferenceColumns` writes a
  `SavedTables` field in the app's preferences document (the same seam as
  window frames); `MemoryColumns` keeps layouts in memory.
- `set_grid` draws vertical, horizontal or both grid lines
  (`Controls::table_grid`). The default is stripes only.
- `set_row_height` / `set_header_height` override the theme metrics. The
  Finder list view uses 19 px rows and a 16 px header.

## Selection and keyboard

- Click selects; Shift-click extends from the anchor; Command-click (Control
  on Windows and Linux) toggles. `set_allows_multiple(false)` restricts to
  one row.
- Up/Down, Home/End and Page Up/Down move the lead; with Shift they extend.
  Command/Control-A selects all.
- Typing selects the first row whose text starts with the typed prefix
  (sort column first, else the first text column). The prefix resets after
  `TYPE_SELECT_RESET` without typing.
- Return edits the lead row's first editable column in a `TextField`.
  Return or leaving the field commits through the column's `editable`
  function; Escape cancels. A row with nothing editable sends `Activated`.
- Double-click sends `Activated`; a secondary click selects the row when it
  is not already selected and sends `ContextMenu { ids, position }` so the
  app can open a `PopupMenu`.
- `TableEvent::SelectionChanged` is sent only for user changes.
  Programmatic `ListController::select` and `set_selection` send nothing.
- The selection is `Lists::table_active` while the table has focus in the
  active window, `table_inactive` otherwise.

## Scale

Rows are built by `uniform_list` for the visible range only; a 100,000-row
table builds the same number of row elements as a 20-row one. Horizontal
scrolling covers the total column width, and the header follows the
horizontal position. Live updates through `set_items` keep the selection
and scroll position.

Not yet: drag source and destination hooks wait for #36.
