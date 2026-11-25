# Open and Save panel visual comparison

The correction brief is `NSSAVEPANEL-2.md` (commit `8d038b1`); the panels it
corrects are from `b90b569`. The sheet rewrite is `5d5c38f`; the interaction
pass on top adds the per-panel extension policy, the browser context menus and
the single-panel Save transition. This sheet records the rectangles and capture
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
527 × 398 outer height minus the 22 px title band.

- Samples: `save-sheet-minimal`, `save-sheet-expanded`, `save-sheet-grow`.
- The grow sample is the at-rest state after a 400 × 300 document grew to fit
  the 527-wide sheet. Growth and exact restore are asserted in
  `crates/silvestris/tests/sheet.rs`.

## Local side-by-side sheet

`target/nssavepanels-comparison/index.html` (generated, not committed) shows
each candidate beside the full reference captures and lists the rectangles
above. The candidate tree is `target/nssavepanels-goldens/` after a render.
