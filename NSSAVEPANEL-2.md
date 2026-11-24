# Correcting the Open and Save panels

`b90b569` ("Finish the Open and Save panels") shipped a panel that passes its
tests and fails on sight. This brief lists what is wrong, why, and the exact
order to fix it. It supersedes the presentation parts of
`docs/NSOPEN-DIALOG-COMPLETION.md` (T5, T7, T8). The model and provider work
(`file_panel/model.rs`, `file_panel/provider.rs`) is sound and stays.

Every task below inherits the rules at the top of
`docs/NSOPEN-DIALOG-COMPLETION.md` (citations, goldens in 4 appearances × 5
scales, scenario tests in their own files, the three gates, the #106 golden
flakes, commit as terezi with no trailer). Read `AGENTS.md` "Half Steps" before
starting. A task is not done because its checklist is ticked; it is done when
the result is indistinguishable in structure and behaviour from the cited 10.6
evidence.

## What the user saw

The gallery's "Open panel" button (`crates/silvestris-demo/src/bin/gallery.rs:1466-1475`)
calls `OpenPanel::present(None, …)`, which opens a separate fixed-size
`WindowRole::Dialog` window (`file_panel/panel.rs:1296-1384`). The gallery's
"Save panel" button does the same for Save (`gallery.rs:1454-1463`). There is
no code path in the gallery or the demo that shows a panel as a sheet, so the
only presentation anyone has looked at is the one that is wrong for Save.

Defects visible in that capture, each traced to source:

| # | Defect | Cause |
| --- | --- | --- |
| D1 | Save opens a new window instead of a sheet on the document | Gallery passes `owner: None`; `SavePanel::present` (`file_panel/save.rs:102-112`) treats a missing owner as "make a window". HIG p. 247: Save is a document sheet. |
| D2 | Button row is clipped off the bottom | `PANEL_HEIGHT = 398` (`file_panel/chrome.rs:24`) is the *outer* height of the 4.15.47 AM capture including the 22 px title bar, but `present` (`panel.rs:1346-1350`) passes it as the window bounds and `FilePanel::render` (`panel.rs:1069`) forces the content to the same 398 px. |
| D3 | Window cannot be resized | `PanelWindow` uses `.fixed_size()` (`panel.rs:1305`). HIG p. 245: the Open dialog has a resize control in the lower-right corner. |
| D4 | Horizontal scroller with a wide thumb under a list that does not need it | `build_table` adds Name 220 + Date 145 + Size 64 + Kind 80 = 509 px of columns and sets `set_fill_width(false)` (`panel.rs:870-939`). The browser area is 527 − 157 = 370 px. |
| D5 | Icons are flat black blobs | `Icon::paint` draws every icon "in one colour" (`icons.rs:37-38`). 10.6 file panels use full-colour 16 px icons (4.15.47 AM capture, sidebar and list). |
| D6 | Toolbar controls are the wrong shape and size | Path pop-up is a huge round-ended control showing `/`; segmented controls render as dark selected blocks; there are two view segments where 10.6 has three (`file_panel/chrome.rs:86-114`; capture shows icon/list/column). |
| D7 | Sidebar has no disclosure triangles and no scroller | `build_sidebar` (`panel.rs:803-858`) builds flat sections; the capture shows ▼ on DEVICES / PLACES / SEARCH FOR and a vertical scroller. |
| D8 | Push buttons are small size | `ControlSize::Small` on New Folder, Cancel and Open/Save (`panel.rs:1232, 1241, 1255`). The capture's Cancel/Open are regular push buttons. |
| D9 | Opens in list view | 10.6 Open opens in column view (capture) and remembers the last choice (HIG p. 245, "remember the user's last selection"). |
| D10 | `/` and `--` everywhere | The path pop-up shows the raw location id instead of a display name; the gallery fixture (`gallery.rs:1977+`) has no dates, so Date Modified shows `--`. Fixture data that makes the control look broken is a defect in the sample. |

Defects in the sheet path that nobody saw because nothing exercises it:

| # | Defect | Cause |
| --- | --- | --- |
| D11 | The parent is dimmed | `Sheet::render` paints `theme.colors.scrim` over the whole content area (`sheet.rs:208`). 10.6 does not dim the parent: `hig-09/specs/sheet.md` §10, and `vm-screenshots/do-you-want-to-save-textedit.png` shows the document unchanged under its sheet. |
| D12 | The sheet is clamped to the window | `.max_w_full()` (`sheet.rs:220`). HIG p. 236: a sheet may be wider than its window, and the window moves away from the screen edge to make room. A 527 × 398 panel in a 400 × 300 document window is squashed. |
| D13 | Sheets stack | The registry keeps "bottom sheet first, alert on top" (`sheet.rs:65`), and `show_replace` (`panel.rs:444-463`) presents an alert sheet over the Save sheet. HIG p. 236: "Only one sheet may be open for a window at any one time … the first sheet closes before the second one opens." |
| D14 | No emergence | No animation (`sheet.rs:14-16`). HIG p. 236: "Sheets are displayed as an animation that appears to emerge from the window's title bar." Without it the panel reads as an overlay card, not a sheet. |
| D15 | Minimal Save is wrong | The toolbar is always rendered (`panel.rs:1089`) and there is no **Where** pop-up. HIG p. 247: the minimal Save dialog is Save As field, Where pop-up, disclosure, Cancel, Save. The toolbar, sidebar and browser belong to the expanded state only (p. 248). |
| D16 | Goldens show a bare panel | `capture.rs:306-330` renders `FilePanel::build` with no window and no sheet, so the goldens cannot catch D1, D2, D11–D14. |

## What 10.6 actually does

Read these before touching code. Do not work from memory.

- **Save is a document sheet** (HIG p. 247). It emerges from the bottom edge of
  the title bar, or of the toolbar when the window has one. It has no title
  bar of its own (p. 236). The parent window keeps its active appearance and
  is not dimmed (`do-you-want-to-save-textedit.png`: traffic lights stay
  coloured, document text unchanged). The sheet's top edge is flush with the
  attachment edge, its sides and bottom carry a drop shadow, and the parent's
  content is visible beside it. `Screen shot 2026-11-23 at 4.10.01 AM.png`
  shows an attached panel hanging from below an Address Book toolbar with the
  same flush top edge.
- **Open is application-modal** (HIG p. 245; `hig-09/03-aqua-interface/windows.md`
  "Open dialog"). It is its own window titled "Open" or "AppName: Open", sized
  527 × 398 outer in the 4.15.47 AM capture, resizable from the lower-right
  corner. The user's statement that panels fly out of the window is correct
  for Save, Save As, Export As and sheet-presented Choose (p. 251). It is not
  correct for File > Open, and converting Open into a sheet would be the
  opposite error. Keep both paths and make each look right.
- **Minimal vs expanded Save** (pp. 247-248): minimal has Save As field, Where
  pop-up, disclosure, Cancel, Save, plus custom elements between Where and the
  buttons. Expanded adds back/forward, the sidebar, list/column browser, a
  Format pop-up slot, New Folder and Hide extension. The panel remembers the
  last state across invocations and launches.
- **Tab order in Save** (p. 248): Save As field → source list → visible
  columns → back to the field.
- **Replace**: p. 236 forbids two sheets on one window. `windows.md:300`
  records the collision alert as application-modal (p. 251). Read p. 251 in
  `Apple-OSXHIGuidelines-2009.pdf` and implement what it says. Stacking an
  alert sheet over the Save sheet is not an option either way.

No 10.6 Save sheet capture exists in hig-09. The collapsed height (160 px,
`docs/exceptions.md` "File panels") is a guess. See S0.

---

## S0. Get the evidence first

**Deliverables**

- Ask the user for 10.6 captures of TextEdit's Save sheet, minimal and
  expanded, on a document window, plus one mid-animation if they can grab it.
  These go into hig-09 as a separate commit by the user, not from this task.
- Until they exist, every Save-sheet dimension is an estimate and is logged in
  `docs/exceptions.md` with the reasoning.
- Re-measure the 4.15.47 AM Open capture with `tools/metrology/measure.py`:
  outer window, title bar, toolbar, sidebar, button row, push-button size,
  segmented-control size, path pop-up size, search field. Record each as a
  measurement id in `reference/measurements.json` with the capture hash. Fix
  `file_panel/chrome.rs:23-29` so each constant states whether it is outer or
  content height.

**Avoid**: editing hig-09; guessing sheet geometry from 10.7+ or from memory.

**Stopping point**: if the user cannot provide Save captures, continue S1–S5
with logged estimates and say so in the final report. Do not claim measured
fidelity for Save.

## S1. Rewrite sheet presentation (`sheet.rs`)

**Files**: `crates/silvestris/src/sheet.rs`, `crates/silvestris/src/chrome.rs`
(overlay mount at `chrome.rs:515-525`, band at `chrome.rs:382-466`),
`crates/silvestris/src/modal.rs`, `crates/silvestris/tests/sheet.rs`.

**Deliverables**

- Remove the scrim (`sheet.rs:208`). The blocking layer stays (it must still
  `occlude()` and swallow mouse input over the parent's content) but paints
  nothing. Do not delete `colors.scrim` if something else uses it.
- Attachment: the sheet's top edge sits on the bottom edge of the window band
  (title bar plus toolbar, `chrome.rs:382`). The overlay is already mounted in
  the content area below the band; verify with a test that the sheet's top
  equals the content area's top, in a window with and without a toolbar.
- Sheet chrome: no top border, square top corners, side and bottom border plus
  a shadow concentrated below and to the sides. Take colours and shadow extent
  from `do-you-want-to-save-textedit.png` and the 4.10.01 AM capture; log
  anything unmeasurable. Do not reuse `shadow_lg()` without a measurement.
- Size: content-driven width and height. When the sheet is larger than the
  parent's content area, grow the parent window to fit for the life of the
  sheet and restore its exact previous bounds on dismissal (the in-window
  equivalent of p. 236's "moves the window away from the edge"). Keep the
  window on screen. Remove `.max_w_full()` (`sheet.rs:220`). Log the growth
  behaviour in `docs/exceptions.md` as our adaptation.
- One sheet per window. `present` on a window that already has a live sheet
  either refuses (returns `false`) or queues until the first is dismissed; pick
  one, cite p. 236, and document it. Remove the stack comment and logic at
  `sheet.rs:63-67`.
- Emerge and retract animation. The sheet slides down from behind the
  attachment edge, clipped to the content area, and retracts upward on
  dismissal. Duration and curve are estimates (no 10.6 timing evidence,
  `specs/sheet.md` §5 and §10). Log them. Drive it from GPUI's animation API,
  repaint only the sheet, and make the final state reachable without wall-clock
  time in tests. Modal setup (`modal::present`) happens at the start of the
  animation so input is blocked immediately; `on_finish` fires after the
  retract completes or immediately if the owner window closes.
- The parent keeps its active-window appearance while its sheet is open and
  takes the normal inactive appearance when another window is key (p. 237).
- The title bar stays draggable and the window stays movable while the sheet
  is open; only content interaction is blocked (p. 236).

**Avoid**: a second window for a sheet; any scrim or dimming; per-control
Escape handling (the sheet's existing Escape handler at `sheet.rs:211-216` goes
through `modal::escape_session`, keep that); changing shared shadow tokens.

**Regression traps**: closing the owner window mid-animation or mid-dispatch
(`sheet::close_owner`, `modal::cancel_owned`); Escape during the emerge
animation; focus restoration to the invoking control (A11Y J4); the
`ConfirmationSheet` and `Alert` callers in `controls/confirmation.rs` and
`controls/alert.rs`, which now render inside the new chrome; inactive-window
appearance; all four appearances and 100–200% scale; idle repaint stops when
the animation ends.

**Tests** (`tests/sheet.rs`): no scrim element in the overlay; top edge equals
content top with and without a toolbar; parent grows and restores bounds
exactly; second `present` on the same window obeys the one-sheet rule;
animation reaches its end state under the test executor without sleeping;
owner close during animation delivers `Cancelled` once.

## S2. Presentation API that makes the right thing the default

**Files**: `file_panel/save.rs`, `file_panel/open.rs`, `file_panel/panel.rs`
(`present`, lines 1312-1384), `docs/file-panels.md`, `CHANGELOG.md`.

**Deliverables**

- `SavePanel::begin_sheet(owner, window, cx)` presents on a document window.
  `SavePanel::run_modal(window, cx)` is the application-modal fallback for an
  app with no document window, named so nobody reaches for it by accident.
  Remove the `Option<AnyWindowHandle>` overload; it is how D1 happened.
- `OpenPanel::run_modal(window, cx)` is the primary Open path (p. 245).
  `OpenPanel::begin_sheet` exists for Choose-style use (p. 251).
- The dialog window title for Open defaults to "Open" and accepts an
  app-supplied title such as "TextEdit: Open" (p. 245).
- The replace-existing flow follows S1's one-sheet rule and p. 251 (see
  "What 10.6 actually does"). `show_replace` (`panel.rs:444-463`) currently
  skips the alert entirely when there is no owner and saves over the file.
  That is a data-loss bug; fix it in this task.
- `document.rs` Save As / Open helpers call the new names.

**Avoid**: keeping the old `present(Option<…>)` as a deprecated alias;
consumer-specific defaults.

**Regression traps**: `Document::save_to` and the close-with-unsaved-changes
flow in `document.rs`; Save during an open Save sheet must be blocked by
`ModalScope::Document`; existing `tests/save_panel.rs` and `tests/open_panel.rs`
must be ported, not deleted.

## S3. Open dialog window (`run_modal`)

**Files**: `file_panel/panel.rs` (`PanelWindow` 1296-1309, `present` 1343-1381,
`panel_height` 1391-1397), `file_panel/chrome.rs`, `window_role.rs`.

**Deliverables**

- Outer size 527 × 398 from S0's measurement; the content area is outer minus
  the measured title bar. Nothing is clipped at any scale (D2).
- Resizable with the lower-right grip (p. 245). Minimum size measured or
  logged. The browser area takes the extra space; toolbar and button row keep
  their heights. Remove `.fixed_size()` and the hard `.h(px(...))` at
  `panel.rs:1069`; the panel fills its host.
- Frame persisted under the dialog role so the next Open reopens at the user's
  size (p. 245, "remember").

**Avoid**: a separate sizing path for the sheet and the window; the panel
reads its size from its host in both.

**Regression traps**: `desktop.json` `schema_version=1` and unknown-field
round-trips if the frame is persisted there; Settings bounds restore;
application-modal blocking of other windows.

## S4. Panel contents that match 10.6

**Files**: `file_panel/panel.rs` (render 956-1140, `name_row` 1143-1193,
`button_row` 1195-1290, `build_sidebar` 803-858, `build_table` 860-942),
`file_panel/chrome.rs` (toolbar 51-136), `controls/path_control.rs`,
`controls/segmented.rs`, `controls/source_list.rs`, `icons.rs`.

Fix each of these in the generic control when the defect is in the control,
not in the panel. If a control cannot produce the 10.6 look, that is a control
bug with its own commit and goldens.

**Deliverables**

- Toolbar (D6): textured back/forward pair and view selector at the measured
  sizes; the path pop-up is a regular pop-up showing the current location's
  icon and display name (`Macintosh HD`, `Documents`), never a raw id; search
  field at the measured width. The view selector has icon, list and column
  segments as in the capture. Icon view needs #35; if #35 is not done, land it
  first (see S6). A disabled or missing segment is not acceptable.
- Sidebar (D7): DEVICES / PLACES / SEARCH FOR with disclosure triangles that
  collapse their groups, a vertical scroller when the content overflows, the
  source-list background and selection colours from the capture, eject only on
  ejectable volumes.
- Icons (D5): a designed full-colour 16 px set for folder, document, home,
  desktop, documents, applications, volume, network volume, smart search and
  the toolbar glyphs, with selected and inactive variants. Original artwork
  only: no tracing or recolouring Apple icons (AGENTS.md). Record them in
  `docs/exceptions.md` as designed. Monochrome silhouettes stay available for
  toolbar glyphs only.
- List view (D4): Name and Date Modified visible by default, Size and Kind
  available but hidden, last column fills the viewport (`set_fill_width(true)`),
  no horizontal scroller unless the user widens columns past the viewport.
- Default view (D9): column view on first open; the last-used view and
  sidebar width persist across invocations.
- Buttons (D8): regular-size push buttons with measured widths; Open/Save is
  the default button; New Folder sits at the left (Save expanded only).
- Save minimal (D15): Save As label and field, Where pop-up (volumes, sidebar
  places, Recent Places, p. 247), disclosure, Cancel, Save. No toolbar. Custom
  accessory between Where and the buttons.
- Save expanded (p. 248): Save As row, then toolbar, sidebar and browser, then
  Format slot and Hide extension, then New Folder / Cancel / Save. Accessory
  between the browser and the buttons. Expanded state and Hide extension both
  persist (pp. 247-248) through the preferences store, not a panel global.
- Name field selection (p. 247): base name selected, visible extension not
  selected, on every open.
- Tab order (p. 248): field → sidebar → visible columns → field. Test it.
- Gallery fixture (D10): realistic names, dates, sizes and kinds for every
  entry, including folders.

**Avoid**: panel-local paint for anything a control owns; new shared tokens to
fix one control; geometry from Finder or 10.7+; changing the model or
provider APIs to fix a view problem.

**Regression traps**: programmatic selection and location changes must not
fire user callbacks; selection and scroll preserved by id across reload and
sort; type-select in list and column view; inline rename after New Folder
(`pending_rename`, `panel.rs:963-989`); disabled-file dimming for disallowed
types; inactive-window selection colour in sidebar and browser.

## S5. Show it where it will be judged

**Files**: `crates/silvestris-demo/src/bin/gallery.rs` (1449-1477, 1977+),
`crates/silvestris-demo/src/bin/capture.rs` (306-330, 351-410, 555-580),
`tools/metrology/render.ps1`.

**Deliverables**

- Gallery: a small "Untitled" document window with File > Save As (sheet,
  minimal and expanded) and File > Open (dialog). Remove the "as
  application-modal windows" cell; it demonstrates the wrong default.
- Goldens render the Save sheet attached to a document window in minimal and
  expanded states, the Open dialog as a framed window, and one frame of the
  sheet at rest over a document smaller than the panel (the grow-to-fit case).
  Bare `FilePanel::build` goldens are removed.
- Side-by-side comparison sheet for the user: our render next to the 4.15.47
  AM capture rectangle and (when S0 delivers) the Save captures, at 100%.
  Commit hashes and rectangles only, never crops (AGENTS.md).

**Stopping point**: the user signs off on these renders before S6 or before
closing #29 again. Do not accept goldens without that sign-off.

## S6. Icon view (#35), if not already done

The capture's view selector has three segments and S4 requires all three.
#35 depends on #36 and #48 (NSSAVEPANEL.md). If #35 is too large for this
pass, stop and tell the user; do not ship a two-segment toolbar again or a
placeholder segment.

## S7. Docs and tracker

- `docs/file-panels.md`: replace the recipe with `begin_sheet` / `run_modal`,
  state which panel uses which presentation and why (pp. 245, 247, 251).
- `docs/exceptions.md` "File panels": replace the 160 px note with S0's
  results; add sheet animation timing, sheet shadow, grow-to-fit and the
  designed icon set.
- `docs/coverage.md`, `CHANGELOG.md`, `docs/consumers.md` (API rename).
- Reopen #29 and #22 or file a follow-up issue that references this brief and
  `b90b569`.

## Done when

- File > Save As in the gallery document window slides a sheet out from under
  the title bar, the document is not dimmed, the window grows if it must and
  returns to its exact frame on Cancel.
- File > Open shows a resizable framed "Open" window whose layout matches the
  4.15.47 AM capture rectangle by rectangle, with nothing clipped at
  100/125/150/200%.
- Every defect D1–D16 has a test or golden that fails on `b90b569` and passes
  now. List them in the final report by number.
- Gates pass, goldens accepted with user sign-off, docs updated, commits
  authored by terezi with no trailer.
