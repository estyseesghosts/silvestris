# Open and Save panel visual comparison

The correction brief is `NSSAVEPANEL-2.md` (commit `8d038b1`); the panels it
corrects are from `b90b569`. The sheet rewrite is `5d5c38f`. The interaction
pass added the per-panel extension policy, the browser context menus and the
single-panel Save transition. The hardening brief supplied at `04febbb`
(`docs/SAVEPANEL-2.md`) made the provider authoritative for identity,
validation, existence, trash and Save disposition; the same pass added the
supplied NSSavePanel layout rows. This sheet records the rectangles and capture
hashes used to compare the corrected renders. No corpus image or crop is
committed.

## Open dialog

Reference: `hig-09/vm-screenshots/Screen shot 2026-11-23 at 4.15.47 AM.png`,
SHA-256 `e90f72715f75d9395561aa7dbce48a315ba4da1741515ef1a6fe7cad40adfbb7`.

- Window: x 217, y 37, width 527, height 398 (M: `panel.outer`).
- Title band: 22 px; toolbar 42 px; sidebar 160 px; button row 46 px
  (M: `panel.outer`, `panel.geometry`).
- Controls: history 43 px, view selector 69 px, path pop-up 219 px, search
  108 px; controls 22 rows; regular buttons 21 rows (M: `panel.geometry`).
- Samples: `open-dialog-columns`, `open-dialog-list`.

## Save sheet

No 10.6 Save-sheet capture exists in hig-09. The sheet chrome and attachment
were compared against the two captured sheet/attached-panel states:

- `do-you-want-to-save-textedit.png`, SHA-256
  `984d8c4a0d0a941ce4072ff5374b797827d5d1ce644d7669e108ac33810b3795`:
  the document is not dimmed, the sheet has square top corners, side and
  bottom borders, and no separate title bar.
- `Screen shot 2026-11-23 at 4.10.01 AM.png`, SHA-256
  `24b5b68ed38be20f834d733da7d20d374b31a7d568d401caa3f4b503949621cf`:
  an attached panel hangs flush from below the toolbar.

Save dimensions that remain estimates are listed in `docs/exceptions.md`:
minimal content height 160 px, sheet shadow, 200/160 ms emerge/retract, and
the grow-to-fit margin. The expanded content height (376 px) is the measured
527 × 398 outer height minus the 22 px title band. An optional message line
adds 30 px and an inset accessory box adds 88 px; both are estimates from the
supplied references.

- Samples: `save-sheet-minimal`, `save-sheet-expanded`, `save-sheet-grow`.
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
