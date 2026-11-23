# Exceptions and gaps

Logged deviations from the HIG, the handbook or the measured reference, and
platform gaps that limit what Silvestris can do at the pinned GPUI. Handbook
15 asks for "logged exceptions when historical visual exactness conflicts"
with accessibility; this file is that log. Values marked `designed:` in the
token sources (`crates/silvestris-core/src/aqua.rs`, `geist.rs`) are not
repeated here; they are listed in place next to the token.

## Contrast

| Theme | Pair | Ratio | Why kept |
|---|---|---|---|
| Aqua Light | table selection (3b77da) / white text | 4.34:1, below 4.5 | Measured 10.6 value (M: table.selection.active). Dark Aqua's 2f63b8 passes. Exception recorded in `silvestris-core` `contrast` test. |

## Typography

- Aqua uses Lato and Source Code Pro in place of Lucida Grande and Monaco
  (epic #118: bundled offline fonts, no Apple fonts shipped). Glyph widths
  differ, so measured *padding* is kept, not measured widths: Force Quit is
  99 px in 10.6 and 94 px in Silvestris with the same 16 px padding
  (M: button.push.widths).
- Regular push labels are moved up 1 px (`Metrics::push_label_dy`) because
  centred Lato 13 sits one row lower than the reference ink
  (M: button.push.label_rows).

## Window controls

- Sphere art is an analytic model fitted to the 10.6 Finder capture
  (tools/metrology/fit_traffic.py), mean absolute error 7.3/255 per channel
  over the three lights and 4.8 on the held-out disabled light. Residuals are
  largest in the saturated band just under the specular cap.
- Hover glyph colours and strokes are designed (not captured).
- Resize grip (`paint::resize_grip`, M: grip.*): on Windows the corner
  answers `WM_NCHITTEST` with `HTBOTTOMRIGHT` (native.rs `grip_proc`), so the
  system shows its diagonal resize cursor there. The 2009 HIG (PDF p. 161)
  keeps the arrow cursor over the grip; the native cursor is kept because it
  comes with the native size loop, snapping and DPI handling.

- Windows: the custom frame adds `WS_CAPTION` back to GPUI's Normal
  HWND. GPUI still owns the titlebar content and its non-client hit tests.
  DWM controls the native outline, small corner radius and exterior shadow.
  An Aqua Light screen capture at 100% DPI shows rounded Main and Settings
  corners and a visible outline where the windows overlap. No exterior
  shadow was visible on this desktop, including in a standard WinForms
  reference capture. The corner attribute's return value alone does not
  establish the shape. See handbook 06 and Microsoft's
  [Windows 11 rounding guidance](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners).

## Estimated values

`estimated:` tokens come from post-10.6 captures the user supplied because no
10.6 capture shows the state. They are fits, not measurements.

- Aqua Light pressed push button (`aqua_pressed` in aqua.rs): a per-channel
  linear map `pressed = a * default + b` fitted from the default to the
  pressed bevel in the post-10.6 capture "Screenshot 2026-11-19 020811.png",
  mean error 3.0 / 2.5 / 1.4 (R / G / B). Dark Aqua's pressed bevel is
  designed.

## Text fields

- Caret blink: Windows uses `GetCaretBlinkTime` (native.rs `caret_blink`);
  other platforms use a designed 500 ms half-period until a platform source is
  wired.
- Text, baseline and caret rows match the Go to Folder field
  (M: field.geometry): ink 5 px from the left border, baseline 18 rows under
  the first focus-ring row in both, caret on the same 17 rows. Lato's cap height is
  one row taller than Lucida Grande's at 13 px.

## Accessibility gaps at the GPUI pin

- No `aria_disabled`: disabled controls are removed from the tab order and
  lose their click handlers, but assistive technology is not told they are
  disabled. Revisit when the pin moves.

## Checkboxes

- Regular Aqua boxes reuse measured materials (`checkbox.off/on/off.disabled`).
  Small/mini dimensions, label gap and vector check/dash outlines are estimates;
  Dark Aqua/Geist are designed. See [checkbox contract](checkbox.md).
- Label clicks and Mixed→On are recreation choices from CMP-03's open decisions;
  Enter is an extension to historical Space activation. Native visual, DPI and
  accessibility delivery checks remain pending.

## Disclosure triangle

- Issue #128: 13 px bounds, initially right-pointing, down when expanded:
  hig-09/03-aqua-interface/controls.md, "Disclosure triangles" (HIG pp. 329-331).
- The inset triangle silhouette and secondary-text colour are estimates, not
  measured 10.6 artwork. The same geometry is used for all four themes.
- Modern keyboard extension: Tab focus, Enter/Space toggle, Left collapse,
  Right expand; expansion callbacks stop propagation so enclosing rows do not
  select. Disabled controls share the GPUI accessibility limitation above.
- Visual/DPI and real keyboard validation remain pending.


## Additional controls

- Gradient buttons use the 21 px square Print & Fax add/remove controls
  in the 10.6 reference as the Aqua Light normal-state model (HIG pp.
  273–276). Pressed, selected and disabled states reuse the textured
  materials. Dark Aqua and Geist reuse their respective textured materials
  with the same square geometry.
- Search reuses text entry and IME handling with the search material,
  a capsule bezel and clear action (HIG p. 325).
- Radio, slider and progress geometry is estimated from their HIG
  descriptions. Busy progress stops animating while inactive or when GPUI
  reports reduced motion. The gallery exercises these controls under all
  four themes. Native/DPI baselines for the new variants are pending.

## Windows

- A utility window's title band is 16 logical px. HIG p. 198 shows the smaller
  title bar but the corpus has no utility-window capture at a known scale, so
  the height is an estimate. Document, panel, preferences and dialog title bars
  use the measured `theme.metrics.titlebar_height`.
- Whether a panel floats above other applications, hides on deactivation or
  joins the parent's window group is a platform capability, not a role flag.
  `RoleSpec::hides_on_deactivate` records the intent; #50 owns per-platform
  verification.

## Visual goldens

- `goldens/renders` is produced by the native Windows renderer, not by a
  software rasterizer. Runs on the same host and driver are byte-identical
  (grayscale text, bundled fonts, fixed scale, no focus or animation), but a
  different GPU or driver can change antialiasing. Re-render on the reference
  host before treating a cross-host difference as a regression; #50 owns
  platform verification.
- The 10.6 crop rectangles in `reference/crops.json` name the corpus file and
  sha256. The gallery capture is a 1x window capture; the Appearance pane
  capture is 2x. No corpus image is committed.
- A control without a 10.6 crop reports no perceptual distance. That is a
  reference gap, not a passing fidelity claim.
