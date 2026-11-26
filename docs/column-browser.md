# Column browser

`controls::ColumnBrowser` is the column view of HIG p. 334 and the Open panel
of HIG p. 245: a row of single-column lists. Selecting a branch opens its
children in a new column, which scrolls into view. Each column is a headerless
`TableView`, so selection, keyboard movement, type-select and virtualisation
are the table's (#32), and the browser owns only the hierarchy and the column
strip.

```rust
let browser = cx.new(|cx| {
    ColumnBrowser::new(
        vec![
            BrowserNode::branch("home", "terezi"),
            BrowserNode::branch("desktop", "Desktop"),
        ],
        cx,
    )
});
cx.subscribe(&browser, |_, _, event: &BrowserEvent, cx| match event {
    BrowserEvent::NeedsChildren(parent) => {
        let nodes = provider.children_now(parent);
        browser.update(cx, |browser, cx| browser.set_children(parent, nodes, cx));
    }
    BrowserEvent::PathChanged(path) => { /* update the panel's location */ }
    BrowserEvent::Activated(id) => { /* open the file */ }
})
.detach();
```

## Children

The browser never reads a filesystem. It emits `NeedsChildren(id)` when a
branch opens without a cached answer. `set_children(parent, nodes)` fills the
open column in place, when called now or on a later frame. A late answer for a
column the user has since left is cached (`children_of`) and does not re-open
the column.

`set_children` replaces the items through `ListController::set_items`, so the
selection survives by id when the answer is a reload. Columns to the left do
not move or change selection while a column loads; the column shows
"Loading…" until its answer arrives.

## Navigation

- A click on a branch selects it and requests its children; a click on a leaf
  truncates every column to its right.
- Right moves to the next column and selects its first child when it has no
  selection; if there is no next column and the focused row is a branch, Right
  opens it. Left clears the selection of the column it leaves and focuses the
  parent column; the cleared column remains open.
- The focused column's selection is blue; columns to its left are unfocused
  and their selection is grey, the 10.6 column-view appearance.
- `set_path` selects a path programmatically without sending `PathChanged`; it
  stops at the first id that is not already present. `path()` returns the
  selected ids, root first.
- `focus_first` and `focus_column` move keyboard focus from the panel's Tab
  order.

## Shared file views

`ColumnBrowser` is one presentation of a shared browser state, not a second
navigation model. The Open panel keeps one `FileBrowserModel`: a branch
selection emits `PathChanged` and the panel calls `model.navigate`, while a
leaf selection stays in the current location. When the panel switches to
List or Icon, it carries the deepest selected leaf into the model controller
by stable id, so the same file stays selected where that id still exists.
`FileViewHost` wraps the same model for consumers that want one switch entry
point.

## Drag source

`set_draggable(true)` forwards `TableView::set_draggable` to every column, so
each row offers a `TableDrag { source, ids }`. A branch row is still just a
row for drag purposes; the consumer resolves its own payload and decides
whether dropping it means move, copy or navigation.

## Preview

`set_preview(f)` renders a final column for the selected leaf only; a branch
has no preview. The preview is arbitrary panel content: file metadata, a
thumbnail, or nothing.

## Inline rename

`set_rename(f)` installs the name column's commit function:
`f(parent, id, new_name) -> bool`. Call it before the browser is first
rendered; replacing it rebuilds the root column and clears the selection.
`begin_editing(id, window, cx)` starts an edit in whichever column shows the
row, and `is_editing(cx)` reports whether a column is editing. The Open panel
uses this for New Folder in column view, passing the column's parent location
so the provider renames the right child.

## Geometry

- Column pitch is 186 px, of which the trailing 15 px are the column's
  scroller track (M: `browser.columns`, `browser.divider`). The track is the
  divider, matching the Open panel capture; its colours come from the shared
  `controls.scroller` tokens.
- Rows are 19 px (`browser.geometry`, and the Finder list measurement behind
  `table_row_height`).
- The branch triangle is 7 x 6 px at the row's trailing edge (M:
  `browser.arrow`).
- The resize grip is two 1 px lines, 3 px apart, 7 px tall, starting 5 px
  above the browser's bottom edge (M: `browser.grip`). Its 15 px hit area is
  the bottom of the divider.
- Dragging the grip resizes that column within 100 px and three default
  widths. Column widths are per-browser, not persisted; #34 leaves that to a
  consumer that asks for it.

The 100 px minimum is an estimate; see `docs/exceptions.md`. The grip colour
reuses `colors.text_secondary` (the measured line is `#5a5a5a`; Aqua's
secondary text is `#4c4c4c`), and the divider reuses the scroller track border
rather than adding a browser-only token.
