# Model bindings and list state

Two small pieces remove the glue a preferences pane or list view otherwise
writes: a typed `Binding<T>` from a model field to one control, and a
`ListController<T>` with stable ids, selection, sorting and filtering.

They are plain structs and closures. There is no reactive graph, no derive and
no macro.

## Binding

```rust
let level = Binding::field(
    &settings,
    |settings: &Settings| settings.level,
    |settings: &mut Settings, value| settings.level = value,
);
let checkbox = primary.checkbox("primary", "Primary");
let slider = level.slider("level", "Level");
```

`Binding::field` holds weak references to the entity, so a dropped model stops
writing instead of panicking. `Binding::new(get, set)` takes arbitrary closures
for a model that is not an entity.

Adapters: `checkbox` (`Binding<bool>`), `radio` and `popup`
(`Binding<usize>`), `slider` (`Binding<f32>`). Each renders the real control
with the current value and calls `binding.set` from the control's user
callback.

The feedback rule is structural: `set` changes the model and asks observers to
repaint, and a render never calls `set`. A programmatic update therefore cannot
arrive back as a user change. The test writes the model, draws twice and sees
one setter call; clicking the checkbox adds exactly one more.

### Text with a parser and formatter

```rust
let text = TextBinding::new(
    field.clone(),
    count_binding,
    |text| text.trim().parse::<u32>().ok(),
    |value| value.to_string(),
);
text.sync(cx);                       // model -> field, silent
let subscription = text.subscribe(cx, |changed, _| { /* status */ });
```

`TextField::set_text` does not emit `Changed` (AppKit only sends text-change
notifications for user edits), so `sync` cannot loop. `commit` parses the field
text and writes the model; an unparsable value leaves the model alone.
`subscribe` wires the field's `Changed` event to `commit`, and the returned
subscription must be kept alive by the owner.

## ListController

```rust
let mut list = ListController::new(|item: &Document| item.id.clone());
list.set_items(documents);
list.set_sort(Some(|a, b| a.title.cmp(&b.title)));
list.set_filter(Some(|item| !item.hidden));
list.select("doc-7");                // programmatic, no callback
for id in list.visible_ids() { /* feed a source list or table */ }
```

- Identity is the `ItemId` returned by the constructor closure, not the index.
  Selection survives sort, filter and `set_items` (a reload); if the item is
  gone, `selected()` returns `None` but the id is retained.
- `visible()` / `visible_ids()` are in sort order after the filter.
- `select` is programmatic and returns whether the selection changed; it does
  not call an app callback. The app wires its list or source-list `on_select`
  separately, which is where user intent is handled.
- `source_nodes(label)` produces `SourceNode`s for a source list; `index_of`,
  `selected`, `len`, `visible_len`, `clear_selection` cover the rest.

The gallery binds a checkbox and a slider to one model, and shows a
`ListController` whose ids can be sorted and filtered while the selection stays
on id 2.

## Tests

`binding.rs` covers: a programmatic write repaints without a second setter call
and a click adds exactly one; selection survives sort, filter and reload, and a
new filter applies; `TextBinding` formats a programmatic value into the field,
parses a user edit into the model, and rejects an unparsable one. The gallery is
the visual sample; no goldens are involved because the rows use existing
controls.

## What this is not

This is not a two-way binding framework. It does not observe arbitrary property
paths, does not cache derived values and does not schedule. A preferences pane
that needs several fields builds several `Binding`s from the same entity.
