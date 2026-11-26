# Preferences windows

`silvestris::preferences` builds System Preferences-style windows (HIG
pp. 243-244 preferences windows, pp. 347-350 layout). Open the window with
`chrome::fixed_window_options` and render it in `WindowFrame::fixed_size()`:
no size control, zoom disabled, minimize active, matching the 10.6 System
Preferences capture in hig-09/images.

- `PreferencesNavigator`: current pane plus back/forward history. Show All is
  an entry like a pane. `title()` gives the window title, the selected pane's
  name per HIG p. 244. `retain()` drops panes that disappear.
- `PreferencesController`: owns the window-level behavior around the widgets.
  It projects the current pane title, stores and restores the last selection,
  measures natural pane content after layout, clamps the fixed window target to
  the display's usable region, and re-targets an outstanding resize instead of
  letting an older target win. `fit_pane` is the pure sizing rule;
  `PreferencesResize` is the cancellable linear transition. When the content is
  taller than the display, `pane_scrolls()` is true and
  `pane_viewport(content, true)` scrolls only the pane. `measure_content` and
  `pane_body_natural` are the measurement helpers.
- `PreferencesToolbar`: momentary back/forward segments, Show All, and a
  right-hand slot (System Preferences puts Spotlight search there). It is a
  fixed row: preferences toolbars are not customizable (HIG p. 244).
- `PreferencesOverview`: category bands of 32 px icon cells. Consumers supply
  their own artwork; no Apple icons are included.
- `TabView`: group box with a centred tab control straddling the top edge.
  `on_overflow` reports how much wider (for the tab strip) and taller (for the
  content) the view needs to be,
  so the window can resize to the pane as 10.6 does (HIG p. 244). Content that
  still does not fit scrolls, with a `Scrollbar` inside the box's right edge.
- `Form`/`FormRow`: right-aligned `Label:` column, controls in one column,
  label-less rows (checkboxes) aligned to the control column, optional note.
  Rows carry explicit `FormRowState` values: normal, disabled/dependent,
  informational note, warning and validation error. `.warning()` and `.error()`
  use one standard message slot below the control with a 12 px mark, so an
  error does not move the control or its label. `.dependent_on(Binding<bool>)`
  dims and disables the row from the model without rebuilding the form; the
  consumer still supplies the control's own disabled state for interaction.
  `Binding<T>` controls (`checkbox`, `radio`, `popup`, `slider`) and a
  `TextBinding<T>` in a row keep the value in the model; a validation error
  leaves the last valid value in place.
- `fit_content()` reports the laid-out height of the overview so the window can resize to it exactly; label wrapping makes a precomputed height unreliable.

Measured (reference/measurements.json): `prefs.overview.geometry`
(81 px cells, 32 px icons 32 px below the band top, labels from 68 px),
`prefs.overview.bands` (#ededed/#e6e6e6 alternation, #d8d8d8 rule; bands
are 99-103 px, 100 is used), `prefs.pane.geometry` (20 px side/bottom
margins, box top 34 px under the toolbar, 22 px centred tabs),
`groupbox.top`/`groupbox.left` (#ababab top edge, #c8c8c8 sides).
Estimates: 5 px grid inset, heading and label line offsets, box corner
radius, content padding, Aqua Dark and Geist band/edge colours.

Related controls: `controls::PopUpButton` (HIG pp. 279-281; current item
over the button, keyboard open/choose/dismiss; `PopupStyle::Rounded` and
`PopupStyle::Textured`; body/cap measured from `popup-button.regular`, see
[exceptions.md](exceptions.md) for the states without a 10.6 crop) and
`controls::Stepper` (HIG p. 301; sizes are estimates). `TextFieldEvent::EndedEditing` lets a
form commit a field's value when focus leaves it as well as on Return.

Not implemented: search with spotlight highlighting of matching icons, and a
captured reference for the stepper and dark-mode panes. Pane resizing uses a
linear 160 ms estimate because no 10.6 transition is captured (see
[exceptions.md](exceptions.md)).


## Preference storage

`silvestris_core::prefs` owns the document; `silvestris::prefs::Preferences`
adds the GPUI entity that views observe. The application chooses where the
bytes live.

- `Store::open(path)` and `Store::open_versioned(path, migrations)` use a JSON
  file. `Store::with_storage(...)` takes any `Storage` implementation, so a
  consumer can keep its own backend. `FileStorage` writes a sibling temporary
  file and renames it into place.
- `Loaded` distinguishes a missing file, a parsed document and a malformed one.
  A malformed read disables saving: `writable()` is false and `save()` refuses,
  so a broken file is never replaced with defaults.
- `Versioned` has `schema_version` and `set_schema_version`. `Migration` is an
  ordered step on the raw JSON object (`from`, `to`, `apply`). Migrations run
  from the document's version to the default type's version; a gap is an error
  and disables saving rather than writing a half-migrated document. A document
  newer than the code is left as written and stays writable.
- Unknown fields survive because every preferences type uses
  `#[serde(flatten)] extra`. They also survive migration, which rewrites the
  JSON object rather than a narrowed struct. lotus's `desktop.json`
  `silvestris` object with `schema_version=1` therefore loads unchanged, and a
  v1 type with no migrations loads at target 1.

`Preferences<T>` wraps the store as an entity:

```rust
let (prefs, loaded) = Preferences::<ShellPreferences>::open(path, &MIGRATIONS, cx);
prefs.update(cx, |prefs, cx| {
    prefs.set("theme", |value| value.theme = next, cx)
})?;
```

`set(key, change, cx)` clones the value, applies the change, writes through
when the store is writable, then calls the callbacks registered for `key`,
then every `subscribe` callback, then `cx.notify()`. An unchanged value does
not write or notify. The entity's observers are what bound controls (#61) use.

Theme changes stay live without recreating state (#70): the application
registers an `on_change("theme", ...)` callback that calls
`silvestris::set_theme_preference`, and every window observes the theme global
as it already does. The store does not own theme application.

Nothing here moves a consumer's file into a Silvestris-owned location; the
path is an argument.

## Storage tests

Core: a malformed file is never overwritten, unknown fields round-trip, widths
survive, ordered migrations rename fields and preserve unknown ones, a
migration gap disables saving and keeps the bytes, a future version is kept as
is, and a memory backend replaces the file. GPUI: one key notifies only its
subscribers plus all subscribers, an unchanged value does not notify, a change
writes through the application backend, a migration upgrades the entity and
keeps newer fields, and a malformed document disables writes.
