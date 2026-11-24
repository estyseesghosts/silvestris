# Open and Save panel completion

This is the ordered local task list for finishing `SavePanel` and
`OpenPanel` as components that consumers can use. Each task names the GitHub
issue it closes or advances. The GitHub issues hold the wider scope; this
list holds only what the panels need.

The starting point is `566ef17` on `devel/sil-cocoa-fixture`:

- #40 scrollers are done (`controls/scroll_source.rs`, `controls/scrollbar.rs`,
  `controls/scroll_view.rs`).
- #32 table view is done (`controls/table_view.rs`, `controls/column_store.rs`,
  `binding.rs` `ListController`).
- #21 modal sessions are done (`modal.rs`).

The panel is a composition. It owns no filesystem code and no geometry that
a generic control could own (NSSAVEPANEL.md, "The payoff").

Every task inherits these rules from AGENTS.md:

- Cite a HIG page, a corpus capture or a measurement ID for every visual
  value. Log estimates in `docs/exceptions.md`.
- A visual change needs goldens in 4 appearances × 5 scales
  (`tools/metrology/render.ps1`, then `gate.py --accept`).
- The human user must sign off on goldens.
- A behaviour change needs scenario tests in their own file under
  `crates/silvestris/tests/`.
- A public API change updates `docs/coverage.md`, `docs/consumers.md` and
  `CHANGELOG.md`.
- Do not change shared tokens to fix one control. Do not branch interaction
  code on theme. Do not mix rustfmt changes of existing code with behaviour
  changes.
- Gates: `cargo fmt --all -- --check`,
  `cargo clippy --workspace --all-targets --locked -- -D warnings` and
  `cargo test --workspace --locked`.
- Commit as terezi with no attribution trailer.

Golden traps that apply to every visual task:

- `aqua-light/200` and `geist-light/200` `source-list-secondary.png` are
  flaky (#106). Copy them from `goldens/renders/` over the candidates before
  accepting.
- `geist-light/150/window-frame-edited.png` has flaked once in the title
  text. Re-render it alone before you believe it.
- `render.ps1` renders `$Pages = 5` capture pages. When `SAMPLES` in
  `crates/silvestris-demo/src/bin/capture.rs` outgrows them, raise `$Pages`.
  Otherwise new samples silently go unrendered.

---

## T1. TableView options needed by embedders (prerequisite for T2)

**Files:** `crates/silvestris/src/controls/table_view.rs`,
`crates/silvestris/tests/table_view.rs`.

**Deliverables**

- `set_header_visible(bool)`: no header row and no header hit-testing.
- `set_alternating(bool)`: a plain `colors.content` background when false.
- `set_fill_width(bool)`: the last visible column stretches to the viewport,
  and the horizontal bar never shows.
- `TableView::focus_handle` is usable for programmatic focus (it already
  implements `Focusable`). Write a test that moves focus between two tables.
- Tests for each option, added to `tests/table_view.rs`.

**Avoid**

- A separate "simple list" control. Column browser columns and the panel's
  list mode are both `TableView`.
- New tokens. Hiding the header and stripes uses existing colours.

**Regression traps**

- The header offset uses `self.scroll.position().x`. With fill-width, the
  position must stay 0, or headers and rows drift apart.
- `render_rows` leases the controller while calling cell renderers. A
  renderer that reads the controller panics (this is documented in
  `docs/table.md`).
- `TableEvent::SelectionChanged` must stay user-only. Panels rely on this to
  tell typing apart from navigation.

---

## T2. Column browser (#34)

**Files:** new `crates/silvestris/src/controls/column_browser.rs`. A draft
is at `%TEMP%/silv/keep/column_browser.rs`; move it in and compile it after
T1. Also new `crates/silvestris/tests/column_browser.rs` and
`docs/column-browser.md`. Export from `controls/mod.rs`. Add a `column-browser`
sample to `capture.rs` and a gallery cell to `gallery.rs`.

**Deliverables**

- `ColumnBrowser`, `BrowserNode` and `BrowserEvent::{NeedsChildren,
  PathChanged, Activated}`.
- `set_children(parent, nodes)` answers late without disturbing the columns
  to its left.
- `set_path` (no event), `path()` and `set_preview`.
- Left and Right move between columns. Right selects the first child. Left
  clears the column being left.
- In the focused column the selection is blue; in the columns to its left it
  is grey.
- The resize grip is in the bottom 15 px of each divider. Horizontal scroll
  reveals the newest column.
- Measurements in `tools/metrology/measure.py` from the 4.15.47 AM Open
  capture: column width, branch arrow, grip and divider colour. These are
  `browser.*` IDs and they replace the draft's estimated constants. Run
  `measure.py > reference/measurements.json` and check that it only adds
  entries.
- Tests for:
  - the NeedsChildren and late-answer flow;
  - the path after clicks and arrow keys;
  - selection kept after `set_children` reloads;
  - a column with 10,000 rows building only the rows in view;
  - the preview column appearing for a leaf only.

**Avoid**

- Reading `std::fs`. Every child comes from the app.
- A second scrolling implementation. Use `ScrollView` over a `ScrollHandle`
  (`scroll_source.rs`).
- Persisting column widths through `ColumnStore` until a consumer asks for
  it.

**Regression traps**

- A late `set_children` for a parent whose column has been truncated must be
  cached only, not re-open the column.
- In the draft, subscriptions capture `depth`. After a truncate and re-push,
  confirm that the stale subscription is dropped with its `Column`.
- `reveal_last` defers to the next frame. Tests must call
  `run_until_parked` before they assert the scroll position.
- The `BrowserNode.icon` closure runs inside a table cell while the
  controller is leased (see the T1 trap).

---

## T3. Path control and path pop-up (#41, with only the panel's subset of #48)

**Files:** new `crates/silvestris/src/controls/path_control.rs` and
`tests/path_control.rs`. Reuse `controls/popup_button.rs` for the pop-up
form. Add a `PanelIcon` (or the #48 `Icon` subset) in
`crates/silvestris/src/sprites.rs` or a new `icons.rs`.

**Deliverables**

- `PathPopUp`: the "Documents" pop-up in the panel toolbar. It lists the
  ancestors of the current location, then Recent Places. Choosing an entry
  emits `PathEvent::Navigate(LocationId)`.
- `PathControl`: the breadcrumb form, which Finder will use later.
- An icon vocabulary for what the panel needs: folder, home, desktop,
  documents, volume, document, application, back and forward. Every icon is
  selected or unselected through a token, not through per-control SVG
  colours.
- Goldens of the pop-up closed and open.

**Avoid**

- The control owning the location. It renders the
  `FileBrowserModel.location` it is given (T6).
- Filesystem path parsing. Ancestors come from the provider as
  `(LocationId, label, icon)`.

**Regression traps**

- The `PopUpButton` menu metrics are shared. Do not touch
  `aqua.rs`/`geist.rs` pop-up tokens to make the icons fit. Add a field
  instead.

---

## T4. Sidebar for the panel (#3 remainder)

**Files:** `crates/silvestris/src/controls/source_list.rs`,
`crates/silvestris/src/sidebar.rs`, `tests/source_list*.rs`,
`docs/source-list.md`.

**Deliverables**

- The DEVICES / PLACES / SEARCH FOR groups, measured against the 4.15.47 AM
  Open capture, including the eject accessory (`RowAccessory`).
- `SourceListEvent` selection gives a location id only.
- Tab and Shift-Tab move focus between the sidebar and the browser.
- Focused, unfocused and inactive-window selection appearances, with goldens.

**Avoid**

- Sidebar code that navigates. The panel maps the id to the provider.

**Regression traps**

- The #106 flaky goldens are source-list samples. Any change here makes
  them harder to tell apart from real regressions. Compare them by hand.

---

## T5. Sheet presentation (#22) and alerts (#23)

**Files:**

- Sheet: `crates/silvestris/src/modal.rs` (policy, done) and a new
  `crates/silvestris/src/sheet.rs` (presentation). Its tests go in
  `tests/sheet.rs`.
- Alert: `controls/confirmation.rs`, which is today's document-modal
  confirmation. Factor `Alert` out of it into `controls/alert.rs`, with
  tests in `tests/alert.rs`.
- Also `window_role.rs` and `chrome.rs` (title-bar attachment point).

**Deliverables**

- `Sheet::present(owner_window, content, ModalSession)`: attached under the
  title bar, centred, with a measured shadow, and resized with the parent.
  The open and close animation can be a reduced-motion-aware slide, with its
  duration logged in exceptions if it is estimated.
- `Alert` with icon, message, informative text, up to 3 buttons and a
  suppression checkbox. Presets:
  - `Alert::replace_existing(name)` ("“X” already exists. Do you want to
    replace it?", with Replace and Cancel; HIG pp. 233-239);
  - the existing confirmation presets, rebuilt on top of `Alert`.
- Stacking: an alert over a sheet stacks (the `modal.rs` stacking rules).
  Return and Escape map to the default and cancel buttons.

**Avoid**

- Re-implementing focus cycling or command blocking. Use `modal::present`,
  `modal::cycle_focus` and `modal::command_allowed`.
- Breaking `confirmation.rs` callers. Keep its public API as thin wrappers.

**Regression traps**

- `modal::cancel_owned`: closing the owner window must deliver `Cancelled`
  to both the alert and the sheet below it, in that order.
- Sheet goldens depend on window chrome. A token change in `chrome.rs`
  changes every `window-frame-*` golden.

---

## T6. FileProvider and FileBrowserModel (core of #29)

**Files:** new `crates/silvestris/src/file_panel/mod.rs`,
`file_panel/provider.rs` and `file_panel/model.rs`. Tests go in
`tests/file_panel_model.rs`.

**Deliverables**

- `trait FileProvider`:
  - `children(LocationId) -> Task<Result<Vec<FileEntry>>>`;
  - `ancestors(LocationId)`;
  - `places()`;
  - `create_folder(parent, name)`;
  - `exists(parent, name)`;
  - `resolve(LocationId) -> PathBuf`, which is optional.
- `FileEntry`: id, name, kind, is_dir, is_package, size, modified, hidden,
  icon.
- `MemoryProvider` for tests and the gallery. `StdFsProvider` is gated
  behind a feature, so the silvestris core stays free of `std::fs`.
- `FileBrowserModel` entity. It owns:
  - `location`;
  - the back/forward history;
  - `ListController<FileEntry>` for the current directory;
  - the hidden-files and allowed-types filter (non-matching files are
    dimmed for Save and hidden or disabled for Open, as in 10.6);
  - the view mode (List or Columns; Icons once #35 lands).
- Cancellation: navigating away drops the in-flight `children` task. A stale
  result must never overwrite a newer location.

**Avoid**

- UI in this module.
- Blocking I/O on the main thread. `StdFsProvider` uses the background
  executor.

**Regression traps**

- Navigation must keep the selection by id. Use
  `ListController::set_selection`, not index restore.
- Sort state lives in the `TableView`, but the model re-sorts when it
  reloads. Do not sort twice with conflicting comparators.

---

## T7. SavePanel and OpenPanel views (#29)

**Files:** `file_panel/save.rs`, `file_panel/open.rs` and
`file_panel/chrome.rs` (the toolbar row). Tests go in
`tests/save_panel.rs` and `tests/open_panel.rs`, with
`docs/file-panels.md`. Add `capture.rs` samples `save-panel-collapsed`,
`save-panel-expanded`, `open-panel-columns` and `open-panel-list`, and a
gallery button that opens each panel.

**Deliverables**

- Layout measured from the 4.15.47 AM Open capture and the Save captures:
  - the toolbar: back/forward segmented control, view-mode segmented
    control, `PathPopUp` and search field;
  - the sidebar (T4);
  - the browser (list = `TableView` with Name, Date Modified, Size and Kind;
    columns = `ColumnBrowser`);
  - the accessory view slot;
  - the button row: New Folder, Cancel, Save/Open (default).
- Save:
  - `name_field` with the base name pre-selected without the extension;
  - an expanded/collapsed disclosure (`controls/disclosure.rs`);
  - `allowed_types`, `allows_other_types` and "Hide extension";
  - Save is disabled while the name is empty or invalid;
  - when `exists` is true, show `Alert::replace_existing` and return
    `Ok(path)` only after Replace.
- Open:
  - `allows_multiple`, `can_choose_files` and `can_choose_directories`;
  - double-click opens a file or enters a folder;
  - Open is disabled while the selection is not choosable.
- Both:
  - present as a sheet (T5) when given a document window, or as an
    app-modal window (`window_role.rs`) otherwise;
  - return `ModalResult<Vec<PathBuf> | LocationId>`;
  - New Folder runs `create_folder`, then selects the new folder and starts
    an inline rename (`TableView::begin_editing`);
  - keyboard: Cmd-Shift-G (go to folder) is listed as deferred,
    Cmd-Up/Down for parent and enter, Tab across sidebar → browser → name
    field.
- A consumer API that is short:
  `SavePanel::new(provider).allowed_types(..).name("Untitled").present(window, cx)`.

**Avoid**

- Copying the table or browser geometry into the panel. If something does
  not fit, fix it in T1 or T2 instead.
- Writing files. The panel returns a destination, and the consumer saves.
- Shipping icon view in the first version. It is listed below as optional.

**Regression traps**

- Changing the view mode must not lose the selection or the location.
- Typing in the name field must not trigger the browser's type-select. The
  table's `key_down` already returns early while editing, but the name
  field is a separate focus target, so add a test.
- The replace-existing alert over a sheet: cancelling the alert returns to
  the panel, not to the document.
- A directory with 30,000 entries keeps frame time bounded. Add a test like
  `a_hundred_thousand_rows_…` in `tests/table_view.rs`.

---

## T8. Consumer wiring and documentation

**Files:**

- `crates/silvestris/src/document.rs` (Save / Save As / Open commands);
- `crates/silvestris/src/lifecycle.rs` (the close → save flow);
- `docs/documents.md`, `docs/consumers.md`, `docs/coverage.md`,
  `CHANGELOG.md` and `README.md`.

**Deliverables**

- Document Save As presents `SavePanel` as a sheet. The "Don't Save /
  Cancel / Save" close flow chains into it.
- Open command presents `OpenPanel` app-modal.
- `docs/file-panels.md` holds a consumer recipe with `StdFsProvider`.
- Close #29, #34 and #41, and the #22/#23 parts used here, with links to
  the commits.

**Avoid**

- Changing `document.rs` identity or dirty-state semantics (#37 goldens
  cover them).

**Regression traps**

- `window-frame-edited` goldens and `docs/lifecycle.md` scenarios: the close
  flow must still deliver `Cancelled` when the window closes mid-sheet.

---

## Optional after T7

- **#36 drag and drop**, then **#35 icon grid**: icon view mode, and
  dragging files from the panel.
- **Cmd-Shift-G go-to-folder** sheet, **Spotlight search** through a
  provider `search(query)`, and the **media browser**. None of these block a
  usable panel.
