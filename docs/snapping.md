# Device-pixel snapping

Aqua draws one-pixel hairlines: bezels, separators, list grid lines, window
edges. At 125%, 150% and 175% a logical coordinate rarely lands on a device
pixel, so an unsnapped line straddles two device rows and looks blurred or
double. One rule prevents every control from re-solving it.

## The rule

All positions are in logical pixels; the window scale factor `s` maps them to
device pixels (`logical * s`).

- **Quads snap their edges.** `paint::snap_edge(v)` returns `round(v * s) / s`.
  A quad's origin and its far edge both snap, so a border starts on a device row
  and its width is a whole number of device pixels. Use `paint::snap_bounds` for
  a whole `Bounds`; the size is recomputed from the two snapped edges rather
  than snapped independently.
- **A centred stroke snaps its centre.** `paint::snap_stroke_center(v)` returns
  `(round(v * s - 0.5) + 0.5) / s`. A one-device stroke centred on a logical
  edge at 10.0 would span two device pixels at 1.25x; its centre belongs half a
  device pixel away from that edge.
- **Hairline thickness is a whole number of device pixels.**
  `convert::hairline(window)` returns `round(s).max(1) / s`: one device pixel at
  1x and 1.25x, two at 1.5x, 1.75x and 2x. A fractional scale cannot draw exactly
  one device pixel, and the nearest whole number is crisp where a fractional
  one alternates.
- Rounding is `f32::round`, i.e. half away from zero. The direction does not
  alternate per control.

## Where it is applied

- `paint::bevel`, `paint::thumb`, `paint::field`, `paint::focus_ring`,
  `paint::selection` and `paint::chrome` snap their bounds before painting.
- `paint::hline` and `paint::vline` snap both ends and use a hairline
  thickness, so a separator is always an integral number of device pixels.
- `paint::resize_grip` snaps the window corner.
- Menu separators, the preferences group-box top edge and the sidebar divider
  call `paint::hline`/`vline` instead of drawing a one-pixel div.
- The text field's search glyph border, stroke and marked-text underline use
  `convert::hairline`.

## Guard

`crates/silvestris/tests/hairlines.rs` scans `crates/silvestris/src` for
`.h(px(1.…))`, `.w(px(1.…))`, `thickness: px(1.…)` and `stroke(px(1.…))` outside
`paint.rs` and `convert.rs`, and fails when the line does not use the helper.
The two helper files are exempt because they define it.

`paint.rs` tests the rule directly: a hairline is a whole number of device
pixels at 1.0, 1.25, 1.5, 1.75 and 2.0; edges land on device edges; a stroke
centre lands on a device centre; a snapped rectangle has four device edges.

## Goldens

`goldens/renders` has 175% as well as 100/125/150/200, so the scale between the
two common fractional cases is covered. Snapping was landed before the M2
fidelity work, as the issue requires: it shifted existing goldens and they were
re-accepted once with the reason recorded in `goldens/reasons.json`.

## Limits

- A hairline at 1.5x is two device pixels, not a half-transparent one. This is
  a deliberate choice; a 10.6 VM at 150% has no half-pixel to copy.
- Snapping a rounded control can move its painted edge by less than one device
  pixel, so text and artwork inside it can sit a fraction off the bezel. That is
  below the device grid and not visible.
- The native window edge and DWM frame are outside this rule; they are #50's
  platform verification.
