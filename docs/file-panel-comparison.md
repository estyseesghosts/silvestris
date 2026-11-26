# Open and Save panel visual comparison

The correction brief is `NSSAVEPANEL-2.md` (commit `8d038b1`); the panels it
corrects are from `b90b569`. The sheet rewrite is `5d5c38f`. The interaction
pass added the per-panel extension policy, the browser context menus and the
single-panel Save transition. The hardening brief supplied at `04febbb`
(`docs/SAVEPANEL-2.md`) made the provider authoritative for identity,
validation, existence, trash and Save disposition; the same pass added the
supplied NSSavePanel layout rows. `docs/SAVEPANEL-3.txt` then supplied genuine
10.6 Save-sheet captures, so the Save rows below are now measured rather than
estimated. This sheet records the rectangles and capture hashes used to compare
the corrected renders. No corpus image or crop is committed.

## Open dialog

Reference: `hig-09/vm-screenshots/Screen shot 2026-11-23 at 4.15.47 AM.png`,
SHA-256 `e90f72715f75d9395561aa7dbce48a315ba4da1741515ef1a6fe7cad40adfbb7`.

- Window: x 217, y 37, width 527, height 398 (M: `panel.outer`).
- Title band: 22 px; toolbar 42 px; sidebar 160 px; button row 46 px
  (M: `panel.outer`, `panel.geometry`).
- Controls: history 43 px, view selector 69 px, path pop-up 219 px, search
  108 px; controls 22 rows; regular buttons 21 rows (M: `panel.geometry`).
- Samples: `open-dialog-columns`, `open-dialog-list`, `open-dialog-icons`.

The 527 × 398 window, the 42 px toolbar, the 160 px sidebar, the 46 px Open
button row, the 43 px history pair, the 69 px view selector, the 219 px path
pop-up and the 108 px search field are measured and must not move when Save or
icon-view work changes.

## Save sheet

References: genuine 1x 1440 × 900 window captures in
`hig-09/vm-screenshots/sl-archive/dialogs/`.

| capture | SHA-256 |
| --- | --- |
| `dialogs--save-sheet--collapsed.png` | recorded by `tools/metrology` when accepted |
| `dialogs--save-sheet--expanded.png` | recorded by `tools/metrology` when accepted |
| `dialogs--save-sheet--icon-view.png` | recorded by `tools/metrology` when accepted |

Measured rectangles (1x, screen coordinates from the capture):

| element | collapsed | expanded |
| --- | --- | --- |
| sheet | x 385-815, y 88-274 (431 × 185) | x 330-870, y 88-488 (541 × 399) |
| `Save As:` ink | 419-467 | 419-467 |
| `Where:` ink | 427-467 | 427-467 |
| name field element | 474-725, y 104-125 | 474-725, y 104-125 |
| name focus ring | 472-727 | 472-727 |
| disclosure bezel | 733-754, y 105-125 (21 × 20) | same |
| disclosure glyph | rows 114-118, width 7 → 1 (down) | rows 114-118, width 1 → 7 (up) |
| Where pop-up | 474-725, y 141-162 | (row folds away) |
| toolbars/browser | (hidden) | separator y 141-142, browser 185-370 |
| Format group box | 400-801, y 174-227 | 343-857, y 381-434 |
| `Format:` ink | 422-490 | 422-490 |
| Format pop-up | 504-781 | 504-781 |
| `Hide extension` ink | (hidden) | 351-461, y 448-462 |
| `New Folder` bezel | (hidden) | 481-581, y 448-468 |
| `Cancel` bezel | 636-704, y 241-262 | 691-762, y 448-468 |
| `Save` bezel | 720-789, y 241-262 | 775-846, y 448-468 |
| grow grip | (none) | y 475-488 |

Derived geometry now in `file_panel/chrome.rs`:

- Shared label column: both labels end on x 467, 20 px inset plus a 63 px
  column with their colons right-aligned.
- Label/control gap: 6 px (colon 467, element 474).
- Field: 252 px wide at 22 px regular height in both states.
- Disclosure: 6 px after the field ring, 21 × 20 px bezel, solid 7 × 5 px
  glyph, down collapsed and up expanded.
- Row gap: 15 px from the field row to the Where row.
- Format box: 54 px tall including its border, 11 px below the Where row,
  14 px inside the sheet's row column; its label ink is 422-490 and its pop-up
  504-781, so the label and pop-up group is inset 21 px inside the collapsed box
  and centred inside the expanded one.
- Bottom row: 13 px below the Format box; Hide extension, New Folder, flexible
  space, Cancel, Save with a 13 px Cancel ↔ Save gap and a 20 px sheet inset.

Silvestris presents the Save panel in the same 527 px modal card as Open rather
than AppKit's content-tight 431/541 px sheet. The rows above are laid out in one
fixed inner column centred in that card, so the column never moves between
states. This is the one deliberate difference from the capture, recorded in
`docs/exceptions.md`.

## Icon grid

References: `hig-09/vm-screenshots/sl-archive/finder/finder--view-icon--documents.png`
and the icon-view Save sheet (`dialogs--save-sheet--icon-view.png`).

Measured, at 1x:

| measurement | value | how measured |
| --- | --- | --- |
| icon painted size | 48 px tall artwork (Finder), 49 × 63 px in the panel (document artwork including its fold) | tallest artwork ink in a single icon column |
| horizontal cell pitch | 137 px (Finder icons at 439, 573, 700, 829); 184 px in the panel's wider browser | gap between the left edges of adjacent icon columns |
| vertical cell pitch | 109 px (Finder label rows at 212-224 and 324-336) | gap between the top of one label row and the next |
| maximum filename block width | 96 px selected pill (two lines, 32 px tall); unselected ink up to 98 px | bounding box of the selected label artwork |

The measured artwork is 48 px, so the current 32 px logical icon is smaller than
the reference. The pitch is wider than the current 88 × 76 px logical cell. The
constants are corrected to the measured pitch in `icon_grid.rs`; the icon size
follows the same reference rather than an eyeballed 48 px. Finder's icon-size
slider is not reproduced: an NSSavePanel is not a Finder window.

- Samples: `save-sheet-minimal` (tightly cropped), `save-sheet-expanded`,
  `save-sheet-expanded-icons`, `save-sheet-grow`.
- The grow sample is the at-rest state after a 400 × 300 document grew to fit
  the 527-wide sheet. Growth and exact restore are asserted in
  `crates/silvestris/tests/sheet.rs`.

## NSSavePanel layout references

The user supplied three real-application captures in
`~/Downloads/nssavepanel`; they are not committed. They are used for layout
rows, not theme colours.

- `nssavepanel-expanded-leopard.png` (512 x 419), SHA-256
  `9102238f6089bd61eeba128edd2517c9a326d2ef5d167bb2ada828c5852419d8`.
  Top to bottom: centred message, Save As field with a square disclosure button
  at its trailing edge, toolbar, sidebar (DEVICES, SHARED), Name/Date Modified
  list, inset accessory box, bottom row with New Folder at the leading edge and
  Cancel/Save at the trailing edge.
- `nssavepanel-expanded-lion.png` (782 x 419, cropped), SHA-256
  `55820e4977190e2de8678e21215885be374c0ac3b50f53a34ccf847736c16d78`.
  Same Save As + disclosure row, toolbar with view/arrange controls, sidebar
  with FAVORITES/SHARED/DEVICES, a single browser column, and the same bottom
  row. No message or accessory is visible.
- `nsopenpanel_panther.jpg` (468 x 409), SHA-256
  `5804a322801caa867e5ec268cae593259b805e9e4b449f00e2fcddcfc9f8b1c2`.
  Title bar, toolbar, sidebar, column browser with a selected-file preview
  column, a metadata band (File, Last modified, Owner, Group) and Cancel/Open.

The panel implementation follows those rows. Message and accessory heights in
`chrome.rs` are estimates from these captures at an unknown scale: 30 px for
the message line and 88 px for the inset accessory box plus margins. The Open
selection band shows File, Kind, Size and Last modified instead of the
Panther-specific Owner/Group rows.

## Local side-by-side sheet

`target/nssavepanels-comparison/index.html` (generated, not committed) shows
the supplied NSSavePanel/NSOpenPanel captures beside the Save and Open
candidates, then lists every candidate at four appearances and five scales. The
candidate tree is `target/nssavepanels-goldens/` after a render. The 100 panel
files are candidates, not accepted goldens; the gate's existing 70 failures are
tracked separately.

The post-hardening render sets `save-sheet-expanded` in list view with a
message and a sample accessory, and selects `Photo.png` in
`open-dialog-columns` so the preview column and selection band are visible. The
gate reports 1020 existing goldens with the same 70 popup/source-list/table
failures; no panel image is in the committed golden set yet.
