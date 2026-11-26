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
- Pop-up pressed states: no 10.6 capture shows a held pop-up. The pressed body
  darkens the measured normal gloss and the pressed cap reuses the measured
  selected-segment blue. Disabled pop-ups are measured (`popup.disabled`).
- Pop-up small and mini cap widths scale from the measured 21 px regular cap
  by control height; no 10.6 small/mini pop-up is in the corpus.
- Radio small and mini bezel sizes (12/10 px) and their labels are designed;
  the regular 14 px bezel, dot fraction and row pitch are measured
  (`radio.selected`, Appearance pane).
- Slider small and mini track/thumb sizes are designed; the regular 5 px
  track and 15 x 18 px pointed thumb are measured from the Dock capture.
  Tick labels use the small system font; their exact baseline is designed.
  The round thumb is a circle (12 x 12 small thumb in the Finder status bar);
  its regular 15 px diameter reuses the pointed thumb's measured width.
- Progress small/mini heights, spinner sizes, and the barber-pole animation
  speed are designed. The regular 16 px bar, its one-pixel corners, the
  determinate fill, and the barber pole's 32 px period, 45-degree slant,
  and both gloss profiles are measured from
  `indeterminate-barber-pole-installer.png` and the Software Update capture.
  The 4 px stripe edge is an estimate: the capture is upscaled, so its blur
  overstates the source's anti-aliasing. Dark Aqua's barber profiles are
  designed.
- The default-button throb breathes either side of the measured default
  bevel: lighter and less saturated, then deeper and more saturated, about 8%
  each way over a 1.1 s cycle. The two-sided swing was observed on a 10.6 VM
  (Mail's Welcome assistant, Continue, with focus in a text field); period and
  amplitude are estimates, since no frame timing was captured. The throb stops
  for inactive windows, reduced motion and a held press. Focus in another
  control does not stop it.
- A pointer press fades into the pressed (blue) bevel over 90 ms, holds while
  the button is down and fades back over 120 ms on release, observed on a 10.6
  VM with Mail's Cancel. Both durations are estimates. Keyboard presses and
  reduced motion show the pressed bevel immediately.
- Help-button gloss is measured from the Appearance pane help tag only at the
  ring/glyph level; pressed and disabled help states are designed.
- Disclosure pressed and animated rotation are designed; the collapsed
  silhouette, size and colour are measured (`disclosure.collapsed`). The
  13 px hit box is the HIG value, not a drawn-pixel measurement.
- Recessed controls, textured pop-ups, round text buttons and group-box
  padding follow the measured layer order and the HIG; where no 10.6 crop
  exists the material is designed from the nearest measured token.
- Group-box and tab-view edges are measured (M: groupbox.top, .left,
  .bottom; the same rows appear in the Date & Time and Network captures).
  Corner pixels inside the 4 px radius are GPUI's anti-aliasing, not the
  capture's. Dark Aqua's edge rows are designed: each sits at the same
  fraction between edge and fill as its Aqua Light row.
- Under the tabs, 10.6 shows b9b9b9 then e4e4e4 before the box fill; the
  segmented emboss token (black at alpha 48) gives bbbbbb and no second row.
  The token is shared with every segmented control, so it is not retuned for
  tabs.
- Small and mini tabs use the segmented control's 18 and 15 px heights. The
  HIG gives 17 and 15 px without saying whether the outline is included, and
  no 10.6 small or mini tab view is in the corpus. Label margins (12 / 10 /
  8 px) are the HIG's (p. 341).
- Dark Aqua is an invented appearance (10.6 shipped no dark mode). It keeps
  Aqua Light geometry, layer order and hue identities; every surface, mixed
  state and focus ring is designed and named `designed:` in `aqua.rs`.
  Geist Light and Dark are designed on the Geist role scale. They are not
  recoloured Aqua and are not claimed as historical.


## Menus

`PopupMenu` and the pop-up button menu follow the two contextual captures
(`right-click-context-menu-nested.png`, `expanded-contextual-menu.png`;
M: menu.context.*). Measured: 18 px rows, 12 px separators with the line on
the 6th row, 4 px padding, the translucent body, the shadow fit, row and
submenu placement, the arrow and the label inset.

- Menu-bar menus use 19 px rows (M: menu.body). Silvestris does not draw
  menu-bar menus, so `menu_item_height` is the contextual 18. No 10.6
  capture shows an open pop-up button menu; it uses the contextual height.
- The edge is one alpha for all four sides (black at 0.105, the
  least-squares fit). 10.6's top edge is lighter and its bottom edge darker:
  the reproduction is 6 levels too dark at the top and 10 too light at the
  bottom.
- Square corners on a submenu follow GPUI's flip rule, recomputed in
  `submenu_opens_left`. If GPUI changes that rule, the square corner can land
  on the wrong side.
- The checkmark and dash column (6 px from the body edge) is an estimate: no
  capture shows a checked menu item. The marks are font glyphs.
- The 8 px gap between the longest label and the submenu arrow and the 20 px
  gap before the key column are estimates. No capture has a label long
  enough to reach either.
- The key column is measured from a menu-bar capture (M: menu.bar.key_column)
  and the arrow position in both kinds matches (19 px from the body edge),
  so the key column is assumed to match too. On Windows and Linux the
  modifiers are spelled out ("Ctrl+") with no gap before the key.
- Row text moves up 1 px (`Metrics::menu_label_dy`), for the same reason as
  push button labels: centred Lato 13 puts the baseline one row below the
  10.6 ink.
- No 10.6 capture shows a highlighted contextual row over a disabled one, a
  scrolling menu or a menu in an inactive window.
- Dark Aqua's menu body keeps Aqua Light's translucency over its own fill;
  its edge and shadow are designed.

## Scroll bars

- Small (11 px) and mini (9 px) bars are estimates; no capture shows one.
  HIG p. 227 names the sizes but gives no geometry. The measured row
  profiles are resampled to the narrower bar by nearest row.
- Arrows at both ends is an estimate: one 16 px box at each end and a 1 px
  border between the leading box and the track. Every capture shows the
  10.6 default, both arrows together.
- The thumb bands are two-stop gradients. The measured bands curve
  (M: scroller.thumb.row: 88 a5 a1 98 in the upper band), so the middle rows
  differ by up to 0x1d. The row of track just past the trailing edge
  (`#aeaeae`) is not darkened.
- The 16 px line, 18 px minimum thumb and 350 / 50 ms repeat timings are
  estimates (HIG p. 226 leaves the line unit to the application).
- Option-click in the track, which inverts the Appearance track-click
  preference in 10.6, is not implemented; no corpus source documents it.

## Table view

- The plain header bevel is the Activity Monitor measurement
  (ff..e8 / e9..ff); the Finder list header is lighter in its lower band
  (f4..ff, M: table.header.plain). One token serves both.
- The type-select reset (1 s), the 3 px divider grab zone and the 4 px
  reorder threshold are estimates.
- Dark Aqua's sorted header and Geist's sort styling are designed.
- Grid lines are designed (`table_grid`); no capture shows a gridded table.

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

- Issue #5/#128: 13 px bounds, initially right-pointing, down when expanded:
  hig-09/03-aqua-interface/controls.md, "Disclosure triangles" (HIG pp. 329-331).
- The collapsed 4.5 x 6 px silhouette and secondary-text colour are measured
  from `disclosure.collapsed`; the expanded shape is the same silhouette
  rotated 90 degrees. Pressed ink and rotation timing are designed.
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
- Radio, slider and progress now use their own measured Aqua materials and
  geometry tokens (see above); the remaining size and animation estimates are
  listed under Estimated values. Busy progress stops animating while inactive
  or when GPUI reports reduced motion. The gallery exercises these controls
  under all four themes.

## Windows

- A utility window's title band is 16 logical px. HIG p. 198 shows the smaller
  title bar but the corpus has no utility-window capture at a known scale, so
  the height is an estimate. Document, panel, preferences and dialog title bars
  use the measured `theme.metrics.titlebar_height`.
- Whether a panel floats above other applications, hides on deactivation or
  joins the parent's window group is a platform capability, not a role flag.
  `RoleSpec::hides_on_deactivate` records the intent; #50 owns per-platform
  verification.
- App activation on desktop GPUI is derived from window activation, because
  the pin has no application activation event. `lifecycle::ACTIVATION_GRACE`
  (50 ms) is an estimate: an in-app window switch arrives as a deactivate and
  an activate in separate platform messages, and the grace keeps that from
  reading as the app resigning. No 10.6 capture can measure it.

## File panels

- Panel layout is measured from the 4.15.47 AM Open capture (M:
  `panel.outer`, `panel.geometry`): outer 527 x 398, title band 22, toolbar 42,
  sidebar 160 to the divider, button row 46, history 43, view selector 69
  (three 22-23 px segments), path pop-up 219, search 108, controls 22 rows and
  regular buttons 21 rows. `file_panel/chrome.rs` states whether each constant
  is the outer or content height.
- The minimal Save content height is derived, not estimated: it is the measured
  Save-As row, Where row, Format box and button row from the genuine 10.6
  captures in `hig-09/vm-screenshots/sl-archive/dialogs/`
  (`dialogs--save-sheet--collapsed.png`, `--expanded.png`, `--icon-view.png`).
  `file_panel/chrome.rs` states each measured row and derives
  `SAVE_COLLAPSED_HEIGHT` and `SAVE_EXPANDED_HEIGHT` from them, and the
  transition interpolates between exactly those two constants.
- The Save sheet's outer width is the exception: 10.6 sizes it to its content
  (431 px collapsed, 541 px expanded), while Silvestris presents Save in the
  same 527 px modal card as Open. The Save rows are laid out in one fixed inner
  column centred in that card, so the Save As column never moves between
  states. Do not claim the 431/541 px widths.
- The Save sheet's 0.22 black shadow, 12 px blur, 5 px downward offset, 200 ms
  emerge and 160 ms retract remain estimates; no capture isolates the shadow.
  The minimal/expanded transition is one panel whose 220 ms ease unfolds the
  chooser while the Where row folds away.
- The panel icon grid is measured from the icon-view Save capture: 48 px
  artwork, 109 px vertical pitch, ~184 px horizontal pitch in the panel's
  browser and a 96 x 32 px selected-name pill. `CELL_WIDTH` is 172 px because
  Silvestris keeps a whole number of cells across the browser; that is 12 px
  narrower than the measured pitch and still gives two columns. The Finder
  reference puts the same artwork on a 137 px pitch in a full window. Finder's
  icon-size slider is not reproduced.
- The file-panel icons are original full-colour 16 px artwork designed for this
  project, drawn at 48 px in icon view; they are not traced or recoloured Apple
  artwork. The inactive/disabled variants desaturate rather than switching to a
  second drawing. The icon set is a design choice, not a corpus measurement.
- Icon-view selection is the filename block only: a rounded pill behind the
  label, not a table row over the whole cell. The full `CELL_WIDTH x
  CELL_HEIGHT` area stays clickable through an overlay, and the artwork keeps
  its colours when selected (M: `finder--view-icon--documents.png`).
- The NSSavePanel layout references (`docs/file-panel-comparison.md`) supply
  row order but not a known scale. The 30 px message line and 88 px accessory
  box are estimates from those crops. The Open preview column and selection
  band follow the Panther layout; their labels are File, Kind, Size and Last
  modified, not Panther's Owner/Group.
- Symlinks and reparse points are classified but not decorated: a link to a
  directory is navigable, a link to a file is a document, and a broken link is
  an `Alias`. No separate link badge is drawn; the icon set has no link variant.
  Windows junctions are followed as directories. Rename and trash operate on
  the link itself, never on its target.
- Grow-to-fit: GPUI exposes content resize but no window move. The sheet grows
  the owner's content area from its existing origin and clamps the request to
  the display's remaining space; it restores the exact previous content size
  on dismissal. HIG p. 236's "moves the window away from the edge" is therefore
  approximated rather than reproduced.
- One sheet per window is implemented by refusal (HIG p. 236). The
  replace-existing alert is application-modal (p. 251), not a second sheet.
- The Open minimum (560 x 360) is an estimate: the toolbar's fixed controls
  total 524 px including its margins, so below that they would clip. The Open
  window's 527 x 398 outer frame and remembered frame are measured.
- `browser.grip` is measured at `#5a5a5a`; the control reuses
  `colors.text_secondary` (Aqua `#4c4c4c`) rather than adding a browser-only
  token. `browser.divider` is the scroller track (`#dcdcdc` in the capture);
  the control reuses the nearby measured scroller track border so it cannot
  drift from the shared scroll bar.
- `ColumnBrowser`'s 100 px minimum column width and its 3x-default drag limit
  are estimates. The row height (19 px), branch triangle (7 x 6 px) and resize
  grip (two 1 px lines 3 px apart, 7 px tall, 5 px above the bottom) are
  measured.
- The file-panel icons are original full-colour 16 px artwork designed for this
  project, drawn at 48 px in icon view; they are not traced or recoloured Apple
  artwork. The inactive/disabled variants desaturate rather than switching to a
  second drawing. The icon set is a design choice, not a corpus measurement.
- The Save panel's inline rename commits through `FileProvider::rename`; the
  folder name suggestion "untitled folder" follows 10.6, but the numeric
  suffix pattern is our own.
- The collapsed Save sheet's Where pop-up reuses `PathPopUp`; the expanded
  state's Format list is derived from `allowed_types` unless the consumer adds
  choices. The Format-to-extension policy is intentionally minimal: choosing
  a format updates the allowed-type filter, not the typed name.

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
