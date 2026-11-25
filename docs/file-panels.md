# Open and Save panels

`SavePanel` and `OpenPanel` (#29) are compositions, not bespoke dialogs. The
panel owns policy; `FileBrowserModel` owns the location, history, selection and
view mode; a `FileProvider` owns all file access. The panel never reads or
writes a file. It returns the chosen host paths and location ids and the
consumer does the I/O.

```
SavePanel / OpenPanel
├── Sheet or application-modal Open window   (sheet.rs, modal.rs, window_role.rs)
├── FileBrowserModel                          (location, history, selection)
│   └── FileProvider                          (children, places, create, exists)
├── SourceList sidebar                        (DEVICES / PLACES / SEARCH FOR)
├── toolbar                                   (history, view, PathPopUp, search)
├── TableView list, ColumnBrowser columns or IconGrid icons
├── name field, Where/Format pop-ups, disclosure, accessory
└── New Folder / Cancel / Save or Open
```

## Consumer recipe

```rust
use std::rc::Rc;
use silvestris::file_panel::{OpenPanel, SaveDisposition, SavePanel, StdFsProvider};

let provider = Rc::new(StdFsProvider::new());
let home = provider.home().expect("a home directory");

// Save As is a document sheet (HIG p. 247).
let panel = SavePanel::new(provider.clone())
    .directory(home.clone())
    .name("Untitled.txt")
    .message("Choose a destination for this document.")
    .allowed_types(["txt"])
    .allows_other_types(false)
    .state_store(panel_state_store.clone())
    .on_finish(|outcome, cx| {
        if let Some(path) = outcome.first_path() {
            if outcome.disposition == Some(SaveDisposition::ReplaceConfirmed) {
                my_document.save_replacing(path, cx);
            } else {
                my_document.save_new(path, cx);
            }
        }
    })
    .begin_sheet(owner_window, window, cx);

// File > Open is application-modal (HIG p. 245).
OpenPanel::new(provider)
    .directory(home_location)
    .title("TextEdit: Open")
    .allows_multiple(true)
    .can_choose_files(true)
    .can_choose_directories(false)
    .state_store(panel_state_store)
    .frame_store(window_frames)
    .on_finish(|outcome, cx| {
        for path in &outcome.paths {
            my_app.open_document(path, cx);
        }
    })
    .run_modal(window, cx);
```

`SavePanel::begin_sheet(owner, …)` attaches Save/Save As to a document
(`ModalScope::Document`). `OpenPanel::run_modal(window, …)` opens the framed
`WindowRole::OpenPanel` window with `ModalScope::Application` and the measured
527 x 398 outer frame; its title defaults to "Open" and accepts an app-supplied
"TextEdit: Open". `OpenPanel::begin_sheet` is the Choose-style presentation
(HIG p. 251). `SavePanel::run_modal` exists only for an application without a
document window; it is named so it cannot be reached by accident.

Both builders take `Rc<dyn FileProvider>` and must be called from code that has
a `Window` (the name field is created there). `.directory(location)` takes a
provider id; `.directory_path(path)` is the shorthand for a host path and is a
no-op when the provider cannot map it.

`PanelStateStore` remembers the view, expanded state, Hide extension and
sidebar width. `MemoryPanelStateStore` serves tests and the gallery;
`PreferencePanelStates<T>` adapts the application's versioned preferences
entity. `FileProvider` frame persistence uses `window_role::FrameStore` so the
next Open reopens at the user's size.

## Result

`PanelOutcome` carries:

- `result`: `ModalResult::Accepted` for Save/Open, `Rejected` for Cancel or
  Escape, `Cancelled` when the owner window closed the session.
- `paths`: host paths, when the provider implements `resolve`. Empty for a
  provider that has no host paths.
- `locations`: provider location ids, always one per accepted target when the
  provider can address children. A location id is an opaque handle owned by the
  provider instance; do not parse or persist it as a path.
- `hide_extension`: the Save panel's checkbox.
- `disposition`: `Some(SaveDisposition::New)` when the provider reported that
  the target did not exist, `Some(ReplaceConfirmed)` when the user explicitly
  chose Replace, and `None` for Open or a cancelled Save. A consumer writes a
  new file with create-new semantics and a replacement with overwrite
  semantics; this closes the check-then-write race.

`FileEntry` carries both `id` (selection and rename identity) and `location`
(provider identity for navigation and `resolve`). The panel never joins a
parent path and a display name. A filename, including one that cannot be
represented in UTF-8, stays exact in the provider; the label may be lossy.

A Save panel returns only after the replace-existing alert resolves to
Replace. The alert is application-modal (HIG p. 251), not a second sheet over
the Save sheet. Cancel on the alert returns to the panel, not to the document.

## FileProvider

The trait is asynchronous where it can block:

```rust
trait FileProvider {
    fn children(&self, location: &LocationId, cx: &mut App)
        -> Task<Result<FileListing, FileError>>;
    fn ancestors(&self, location: &LocationId, cx: &App) -> Vec<FileAncestor>;
    fn places(&self, cx: &mut App)
        -> Task<Result<Vec<FilePlace>, FileError>>;
    fn create_folder(&self, parent: &LocationId, name: &str, cx: &mut App)
        -> Task<Result<LocationId, FileError>>;
    fn exists(&self, parent: &LocationId, name: &str, cx: &mut App)
        -> Task<Result<bool, FileError>>;
    fn validate_name(&self, parent: &LocationId, name: &str, cx: &App)
        -> Result<(), FileError>;                 // pure, no IO
    fn child_location(&self, parent: &LocationId, name: &str)
        -> Option<LocationId>;
    fn rename(&self, parent: &LocationId, id: &ItemId, name: &str, cx: &App)
        -> Task<Result<(), FileError>>;
    fn trash(&self, parent: &LocationId, id: &ItemId, cx: &mut App)
        -> Task<Result<(), FileError>>;
    fn delete_permanently(&self, parent: &LocationId, id: &ItemId, cx: &mut App)
        -> Task<Result<(), FileError>>;
    fn capabilities(&self) -> FileProviderCapabilities;
    fn resolve(&self, location: &LocationId) -> Option<PathBuf>;
    fn location_of(&self, path: &Path) -> Option<LocationId>;
    fn label(&self, location: &LocationId, cx: &App) -> SharedString;
    fn location_icon(&self, location: &LocationId) -> Icon;
}
```

`FileListing` separates the entries from non-fatal `warnings`: an entry that
disappears during enumeration or whose metadata cannot be read is reported,
not silently turned into a zero-byte file. An unreadable directory is an
`Err`.

`exists` is fallible. An IO error is never reinterpreted as "the target does
not exist"; the Save panel stays open and shows the error.

`validate_name` belongs to the provider. The panel enforces only the
structural rules (non-empty, not `.`/`..`); `StdFsProvider` applies Windows
reserved names, trailing dots/spaces and reserved characters, or Unix's much
smaller rule set. A custom or remote provider can use its own syntax.

`capabilities` is the menu contract. The panel adds Rename, Move to Trash,
Delete Immediately, Copy Path and New Folder only when the provider says it can
perform them. `trash` is reversible and needs no confirmation; a local-disk
provider implements it with the Windows Recycle Bin or the freedesktop trash
directory. `delete_permanently` is the seam for a file-manager application
that owns its storage policy: the panel asks a destructive alert before
calling it, and `StdFsProvider` leaves the capability off.

`places` is asynchronous because host discovery may touch redirected profiles,
mount tables or removable volumes. Memory places are a ready task; the local
adapter fills the sidebar after construction rather than blocking the first
frame.

`label` and `location_icon` feed the path pop-up's display name and icon, so a
provider can return "Macintosh HD" rather than its raw id. The defaults fall
back to the first ancestor's label and a folder icon.

`MemoryProvider` is deterministic and belongs in tests and the gallery:
`add_location`, `set_places`, `push_entry`, `set_entries`, `entries`. Its
locations keep distinct ids; two sidebar places may share a location but not a
duplicate id.

`StdFsProvider` is behind the `std-fs` feature so the core crate stays free of
`std::fs`:

```toml
silvestris = { version = "0.1", features = ["std-fs"] }
```

It keeps exact `PathBuf`s in an `Arc<Mutex<...>>` map. `LocationId`s are opaque
handles (`fs-location-00000001`) owned by that provider instance; the display
name is only a label. `home()`, `root()` and `location_for(path)` create
handles, and `FileProvider::resolve` returns the exact host path. Keep one
provider alive for the panel and for the code that acts on its outcome; a
handle from one instance does not resolve in another.

Directory listing runs on GPUI's background executor. It reports unreadable
entries as warnings, leaves the size/date unknown when metadata is missing, and
classifies symlinks by their target: a link to a directory is navigable, a
broken link is an `Alias`. Windows hidden attributes and Unix dotfiles both set
the hidden flag. Name validation is platform-specific (Windows reserved
devices, trailing dots/spaces and reserved characters; Unix only `/` and NUL).
`trash` uses `SHFileOperationW` with `FOF_ALLOWUNDO` on Windows and the
freedesktop `files/` + `info/*.trashinfo` layout on Linux; macOS and other Unix
systems use their home trash. `delete_permanently` is not enabled for local
disk.

Host places are discovered on the background executor. Windows asks
`SHGetKnownFolderPath` for Home, Desktop, Documents, Downloads, Pictures, Music
and Videos, and enumerates logical drives. Linux reads `user-dirs.dirs` rather
than assuming English folder names, and lists removable mounts under
`/media`, `/run/media` and `/mnt`. The provider never invents search
locations; `PlaceSection::SearchFor` exists for a provider that actually
implements search.

`MemoryProvider` is deterministic and belongs in tests, the gallery fixtures
and the capture samples: `add_location`, `set_places`, `push_entry`,
`set_entries`, `entries`. Its locations keep distinct ids; two sidebar places
may share a location but not a duplicate id. It overrides `child_location`,
`resolve` is absent, and its `trash`/`delete_permanently` mutate only the
in-memory tree.

`FileEntry` carries the row metadata: id, location, name, kind,
directory/package/link flags, whether metadata was available, size,
modification epoch and hidden state. `size_label` and `modified_label` format
for the table; modification labels are UTC because the core has no local-time
dependency. Fixtures must supply names, dates, sizes and kinds for every entry,
including folders; a row of `--` makes a working control look broken unless it
really means "metadata unavailable".

## Views

- **Columns** is the default on first open, as in the 4.15.47 AM capture. The
  last-used view persists through `PanelStateStore`. Selecting a branch opens
  its children; selecting a file leaves the directory alone. The Open panel
  fills the preview column with the selected leaf's icon, name, kind, size and
  date; a consumer accessory replaces the default selection band below the
  browser.
- **List** shows Name and Date Modified by default. Size and Kind are available
  columns, hidden by default. Name and Date fill the browser; no horizontal
  scroller appears unless the user widens a column past the viewport.
- **Icons** is `IconGrid`: fixed 88 x 76 cells, full-colour 32 px icons, click,
  Shift-click, Command/Control-click, arrows and Return, virtualised rows.

The toolbar's view selector has all three segments; a disabled or placeholder
segment is not acceptable.

## Selection and filters

- Hidden files are excluded unless `FileBrowserModel::set_show_hidden(true)`.
- `allowed_types` without the dot plus `allows_other_types(false)`:
  Save dims a non-matching file, Open keeps it visible but refuses to choose
  it. Directories always pass.
- Selection is by id. Reloads, sorts and live model updates keep the selected
  entries; navigating to another directory clears the selection.
- In column view, selecting a file leaves the current directory alone; only a
  branch navigates. `allowed_types` and the choose-selection flags apply to the
  selected row in the last column, and Open accepts that folder when
  `can_choose_directories(true)`.
- An Open panel with `allows_multiple(true)` lets list and icon views build a
  multiple selection; Open reports every chosen location.
- Double-clicking a folder enters it. Double-clicking a file accepts it.
  `Open` on a folder enters it when `can_choose_directories(false)`.

## Save details

- The minimal state (HIG p. 247) is the Save As field, Where pop-up,
  disclosure, Cancel and Save. It has no toolbar, sidebar, browser or New
  Folder. Application content goes between Where and the buttons.
- The disclosure is the bevel button at the trailing edge of the Save As
  field, as in the NSSavePanel references. `SavePanel::message` draws an
  application line centred above that row. An `accessory` is an inset,
  bordered box above the button row and is present in both states.
- The expanded state adds the toolbar, sidebar, browser, Format slot, Hide
  extension and New Folder (p. 248). It is the same panel, not a second view:
  the chooser unfolds from the Where row in one 220 ms eased transition, and
  folds back into it. The sheet/card is content-sized, so it grows and shrinks
  with the panel. The expanded state and Hide extension persist through
  `PanelStateStore`.
- `extension_policy` decides what happens to the typed extension. `Any` is
  the default: the name is saved as typed and `allowed_types` only filters the
  browser. `Fixed("txt")` replaces or appends that extension. `Required`
  appends the first allowed extension when none was typed and rejects a
  different one. A consumer that wants a free-form extension simply does not
  set a policy.
- Right-click a row for Open/Rename/Copy Path/Move to Trash/New Folder, plus
  Delete Immediately only when the provider implements the permanent-delete
  seam. Right-click the empty browser area for New Folder, Sort By, View and
  Show Hidden Files. The menu is the shared `PopupMenu`; actions go through the
  provider or the panel model, never directly to `std::fs`.
- The name field is preselected from the start to the last dot, so typing
  replaces the base name and leaves the extension. `.hidden` keeps the whole
  name selected. The selection is renewed on every open.
- Save is disabled while the name is empty or `.`/`..`, while the extension
  policy rejects it, or while the provider's `validate_name` rejects it.
  Host-specific rules (Windows `CON`, trailing dots, reserved characters;
  Unix's much smaller set) live in the provider, not in the panel.
- Move to Trash is reversible and immediate. Delete Immediately, when a
  provider offers it, is a destructive alert first; `StdFsProvider` does not
  enable it.
- "Hide extension" travels in the outcome; the panel does not rename the file.
- New Folder asks the provider for a unique `untitled folder`, reloads, selects
  the new row and starts an in-place rename. The list uses
  `TableView::begin_editing`; the column view uses
  `ColumnBrowser::begin_editing`. Returning commits through
  `FileProvider::rename`.
- The accessory slot is any `Fn(&mut Window, &mut App) -> AnyElement`, drawn
  above the button row.

## Keyboard

- Expanded Save: Tab cycles Save As field → source list → visible columns →
  the field (HIG p. 248). Minimal Save cycles field → Cancel → Save. Open
  cycles sidebar → browser → search → Cancel → Open.
- Return accepts the default button while the name field is focused (the text
  field submits). Escape closes the sheet as `Rejected`.
- Typing in the name field never reaches the table's type-select.

Not implemented in this version, and listed as deferred: Cmd-Shift-G go to
folder, drag and drop (#36) and Spotlight search. The column browser's preview
column is installed for Open; it shows file metadata, not a document renderer.

## Geometry and evidence

From the 4.15.47 AM Open capture (M: `panel.outer`, `panel.geometry`): the
outer window is 527 x 398 px with a 22 px title band, 42 px toolbar, 160 px
sidebar to the divider and a 46 px button row. The history pair is 43 px, the
three-segment view selector 69 px, the path pop-up 219 px, the search field
108 px, the controls 22 rows and the regular buttons 21 rows. The column pitch,
branch arrow, grip and divider are `browser.*` in
`reference/measurements.json`.

The collapsed Save content height (160 px), the Save sheet's shadow, its
200/160 ms emerge/retract timings and the 220 ms expand/contract timing, the
560 x 360 Open minimum and the grow-to-fit adaptation are estimates; they are
logged in `docs/exceptions.md`.

The NSSavePanel references supplied for this pass are used for row order, not
for theme pixels: a centred message line, Save As with a trailing disclosure
button, the toolbar/sidebar/browser block, an inset accessory box and a bottom
row with New Folder at the leading edge and Cancel/Save at the trailing edge.
The Open reference adds the selected-file preview column and a metadata band
above Cancel/Open. `docs/file-panel-comparison.md` records the files and their
hashes.

Goldens: `open-dialog-columns`, `open-dialog-list`, `save-sheet-minimal`,
`save-sheet-expanded` and `save-sheet-grow` are rendered through the capture
harness. The appearance/scale matrix and sign-off follow the same gate as the
rest of Silvestris; this document does not claim goldens that have not been
rendered.
