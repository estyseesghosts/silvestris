# Open and Save panels

`SavePanel` and `OpenPanel` (#29) are compositions, not bespoke dialogs. The
panel owns policy; `FileBrowserModel` owns the location, history, selection and
view mode; a `FileProvider` owns all file access. The panel never reads or
writes a file. It returns the chosen host paths and location ids and the
consumer does the I/O.

```
SavePanel / OpenPanel
├── Sheet or application-modal dialog   (sheet.rs, modal.rs)
├── FileBrowserModel                    (location, history, selection)
│   └── FileProvider                    (children, places, create, exists)
├── SourceList sidebar                  (DEVICES / PLACES / SEARCH FOR)
├── toolbar                             (history, view, PathPopUp, search)
├── TableView list or ColumnBrowser columns
├── name field, disclosure, accessory
└── New Folder / Cancel / Save or Open
```

## Consumer recipe

```rust
use std::rc::Rc;
use silvestris::file_panel::{OpenPanel, SavePanel, StdFsProvider};
use silvestris::window_role::WindowRole;

let provider = Rc::new(StdFsProvider::new());

// Save As in the current document window.
let panel = SavePanel::new(provider.clone())
    .directory(home_location)
    .name("Untitled.txt")
    .allowed_types(["txt"])
    .allows_other_types(false)
    .on_finish(|outcome, cx| {
        if let Some(path) = outcome.first_path() {
            my_document.save_to(path, cx);
        }
    })
    .present(Some(owner_window), window, cx);

// Open, application-modal when there is no document window.
OpenPanel::new(provider)
    .directory(home_location)
    .allows_multiple(true)
    .can_choose_files(true)
    .can_choose_directories(false)
    .on_finish(|outcome, cx| {
        for path in &outcome.paths {
            my_app.open_document(path, cx);
        }
    })
    .present(None, window, cx);
```

`present(Some(owner), window, cx)` attaches the panel as a document sheet
(`sheet::Sheet`, `ModalScope::Document`). `present(None, window, cx)` opens an
application-modal `WindowRole::Dialog` window and runs its session with
`ModalScope::Application`. Both return the panel entity, so a consumer can
inspect it, but the callback is the supported result path.

`SavePanel::new` and `OpenPanel::new` take `Rc<dyn FileProvider>` and must be
called from code that has a `Window` (the name field is created there).

## Result

`PanelOutcome` carries:

- `result`: `ModalResult::Accepted` for Save/Open, `Rejected` for Cancel or
  Escape, `Cancelled` when the owner window closed the session.
- `paths`: host paths, when the provider implements `resolve`. Empty for a
  provider that has no host paths.
- `locations`: provider location ids, always one per accepted target.
- `hide_extension`: the Save panel's checkbox.

A Save panel returns only after the replace-existing alert resolves to
Replace. Cancel on the alert returns to the panel, not to the document.

## FileProvider

The trait is asynchronous where it can block:

```rust
trait FileProvider {
    fn children(&self, location: &LocationId, cx: &mut App)
        -> Task<Result<Vec<FileEntry>, FileError>>;
    fn ancestors(&self, location: &LocationId, cx: &App) -> Vec<FileAncestor>;
    fn places(&self, cx: &App) -> Vec<FilePlace>;             // sections + eject
    fn create_folder(&self, parent: &LocationId, name: &str, cx: &mut App)
        -> Task<Result<LocationId, FileError>>;
    fn exists(&self, parent: &LocationId, name: &str, cx: &mut App) -> Task<bool>;
    fn rename(&self, parent: &LocationId, id: &ItemId, name: &str, cx: &App)
        -> Task<Result<(), FileError>>;                      // default: error
    fn resolve(&self, location: &LocationId) -> Option<PathBuf>; // optional
    fn location_of(&self, path: &Path) -> Option<LocationId>;    // optional
}
```

`MemoryProvider` is deterministic and belongs in tests and the gallery:
`add_location`, `set_places`, `push_entry`, `set_entries`, `entries`.

`StdFsProvider` is behind the `std-fs` feature so the core crate stays free of
`std::fs`:

```toml
silvestris = { version = "0.1", features = ["std-fs"] }
```

`FileEntry` carries the row metadata: id, name, kind, directory/package flags,
size, modification epoch and hidden state. `size_label` and `modified_label`
format for the table; modification labels are UTC because the core has no
local-time dependency.

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
- An Open panel with `allows_multiple(true)` lets the table build a multiple
  selection; Open reports every chosen location.
- Double-clicking a folder enters it. Double-clicking a file accepts it.
  `Open` on a folder enters it when `can_choose_directories(false)`.

## Save details

- The name field is preselected from the start to the last dot, so typing
  replaces the base name and leaves the extension. `.hidden` keeps the whole
  name selected.
- Save is disabled while the name is empty, `.`, `..`, or contains a path
  separator or a colon.
- The disclosure hides and shows the sidebar and browser. `expanded(false)`
  starts collapsed.
- "Hide extension" travels in the outcome; the panel does not rename the file.
- New Folder asks the provider for a unique `untitled folder`, reloads, selects
  the new row and starts an in-place rename. The list uses
  `TableView::begin_editing`; the column view uses
  `ColumnBrowser::begin_editing`. Returning commits through
  `FileProvider::rename`.
- The accessory slot is any `Fn(&mut Window, &mut App) -> AnyElement`, drawn
  above the button row.

## Keyboard

- Tab and Shift-Tab cycle sidebar, browser (the active column in column mode),
  name field, New Folder, Cancel and Save/Open. The list is the modal session's
  focus stops, updated when the view mode changes.
- Return accepts the default button while the name field is focused (the text
  field submits). Escape closes the sheet as `Rejected`.
- Typing in the name field never reaches the table's type-select.

Not implemented in this version, and listed as deferred: Cmd-Shift-G go to
folder, the icon view, drag and drop (#36/#35), Spotlight search, and the
column browser's preview column in the panel. The column browser itself
supports a preview; the panel does not install one.

## Geometry and evidence

From the 4.15.47 AM Open capture: the panel is 527 x 398 px, the toolbar 30 px,
the sidebar 157 px and the button row 37 px. Column pitch, branch arrow, grip
and divider measurements are `browser.*` in `reference/measurements.json`.
The collapsed Save height (160 px) and the 180 px search field are estimates;
they are logged in `docs/exceptions.md`.

Goldens: the panel samples are listed in the gallery and capture harness. The
appearance/scale matrix and any wet-signoff follow the same gate as the rest of
Silvestris; this document does not claim goldens that have not been rendered.
