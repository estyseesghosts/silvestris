# Visual goldens

The visual gate renders the M2 control set and three window frames at 100, 125,
150, 175 and 200% in all four appearances, compares every pixel against the
committed goldens, and reports the distance to a 10.6 reference crop for
information. The control set covers push buttons (all styles and sizes),
checkboxes (on/off/mixed/small/mini/disabled), radios, rounded and textured
pop-ups, segmented controls (text/icon/textured/small), sliders (round,
pointed, vertical, disabled), progress (bar, barber pole, spinner, ring),
disclosures (collapsed/expanded/button), group box, well, separator and both
source-list styles. 175% exists for the device-pixel snapping rule (#12); see
[docs/snapping.md](snapping.md). The frames are a document window and a fixed
About window plus the edited document, so the role-driven title-bar buttons are
gated too (#20). The toolbar samples cover rectangular and capsule items,
disabled state, Icon Only and Text Only; the bottom-bar samples cover the
regular and small families. Two preferences pages show the fitted pane and the
pane that exceeds the fixed window. Two split-view pages show the vertical thin
divider and the horizontal thick divider with its dimple. One action page shows
the closed Action menu, command pop-down and small scope bar.

Committed:

- `goldens/renders/<appearance>/<100|125|150|175|200>/<control>.png` and `layout.json`.
- `goldens/manifest.json`: sha256 per golden.
- `goldens/reasons.json`: why each golden is accepted.
- `reference/crops.json`: corpus file, sha256, rectangle, size, state and, where
  known, capture scale for each control reference. `rect` is in the capture's
  physical pixels. The gallery capture's resolution metadata (168 dpi) does not
  identify a display scale, so its crops record `scale: null` and the note says
  so; the Appearance pane capture is 144 dpi and 2x. The same file indexes all
  70 uncategorised captures with a `subject_guess` and `verified: false` — a
  triage label from one contact-sheet pass, not evidence. No corpus image or
  crop is committed.

## Render

`crates/silvestris-demo/src/bin/capture.rs` opens a fixed-size window, lays out
the samples at known logical rectangles, reads the frame back with
`Window::render_to_image`, and crops each sample. The `capture` feature enables
GPUI's `test-support` APIs (`Window::set_scale_factor` and readback).

```powershell
pwsh tools/metrology/render.ps1
pwsh tools/metrology/render.ps1 -Appearance aqua-light -Scale 1.0,2.0
```

Output defaults to `target/goldens`. On a Windows GNU host that builds GPUI
outside the normal environment, pass `-PrependPath` with the directory that
provides `windres`; `SILVESTRIS_WINDRES_SHIM` works too.

Determinism comes from:

- bundled fonts only, registered by `silvestris::init`;
- grayscale text antialiasing (`App::set_text_rendering_mode`);
- a fixed logical layout and an explicit scale factor per render;
- no focused control, so no caret, focus ring or key state;
- no progress indicator or default-button throb, so no animation phase;
- a fixed window size and no window chrome in the capture path.

Two runs on the same host produced byte-identical PNGs during the initial
acceptance. GPU driver differences are a host risk; #50 owns cross-platform
verification.

## Gate

```powershell
python tools/metrology/gate.py --candidate target/goldens
```

The comparison is strict: a pixel fails when any channel differs by more than
`--tolerance` (default 0). A size mismatch fails. The report at
`target/golden-report/index.html` shows golden, candidate, an amplified
difference image and, for Aqua Light at 100%, the 10.6 crop id and mean channel
distance.

`--accept` copies the candidate over the golden, adds a candidate that has no
golden yet (a new control or a new scale), and requires a reason:

```powershell
python tools/metrology/gate.py --candidate target/goldens \
  --accept --reason "raise the checkbox tick by one pixel"
```

125% is included because it is where half-pixel edges break on controls with
hairlines (text-field and segmented borders, slider track).

`cargo test -p silvestris --test goldens` reads `manifest.json` and
`reasons.json`. It fails when a committed golden changed without
`--accept`, when a golden has no reason, or when the manifest lists a file that
is not present. The render step is not part of `cargo test`: it needs a display
and a GPU.

## Adding a control

1. Add a `Sample` to `capture.rs` with a fixed logical size and a representative
   state. Avoid animation, focus and caret.
2. Run `render.ps1` and inspect the PNG.
3. Accept it with a reason, then update `docs/coverage.md` with the golden id.
   When a change is scoped to one control and this host is known to drift on
   others, accept only that control:

   ```powershell
   python tools/metrology/accept_control.py disclosure-button target/goldens \
     "why this control changed"
   ```

   That regenerates just those manifest and reason entries, so pre-existing
   host drift on unrelated goldens is not silently frozen.
4. Add a crop to `reference/crops.json` if a 10.6 reference exists, with the
   rectangle in capture pixels and a sha256 of the source file. Verify the crop
   at high zoom before committing it. Otherwise record the gap in
   `docs/exceptions.md`; do not use a 10.7 or later capture.

## Panel review captures

The file-panel samples (`open-dialog-columns`, `open-dialog-list`,
`open-dialog-icons`, `save-sheet-minimal`, `save-sheet-minimal-crop`,
`save-sheet-expanded`, `save-sheet-expanded-icons`, `save-sheet-grow`) live on
their own capture pages after the control grid and the menus. They are rendered
and reviewed against the 10.6 references recorded in
[file-panel-comparison.md](file-panel-comparison.md), but they are not in the
committed golden set: the panels are still being corrected against newly
supplied 10.6 captures, so gating them now would freeze the wrong geometry.
`save-sheet-minimal-crop` is the minimal sheet alone with no document behind it,
so dead vertical space is visible in review. Panel geometry is instead asserted
in `tests/save_panel.rs` and `tests/open_panel.rs`.

## Scope

Goldens gate Silvestris against itself. The 10.6 crop distance is informational:
Aqua Dark and Geist have no 10.6 counterpart, and a designed appearance gates
only against its own previous output. Native window decoration, DWM shadow and
ClearType are not in the capture path; native visual review stays in #50.
