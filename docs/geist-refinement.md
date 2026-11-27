# Geist refinement

This file records the baseline and the change surface for the Geist refinement
in `docs/geist.txt`. Geist is a presentation theme for the existing
Silvestris application model. It does not replace the 2009 HIG structure,
interaction rules, or control semantics.

## Baseline

The verified starting commit is `d575f34cb7472f86015c7421998464ea6247ec81`
on branch `devel/sil-cocoa-fixture`. The first change made here was a separate
test-only commit (`90023f0`) for the installer Continue breathing lifecycle,
captured before any Geist work.

- GPUI: `6b720973027999bae402df23d469026f4cced988` (unchanged).
- Rust: `1.98.1`, GNU host on Windows (unchanged).
- Theme identities: `ThemeFamily::{Aqua, Geist}` times
  `Appearance::{Light, Dark}`; `ThemePreference::resolve` honours system or
  explicit appearance. The default preference stays Aqua Light.
- Theme resolution: `silvestris-core/src/lib.rs::theme` maps the four ids to
  `aqua::{aqua_light, aqua_dark}` and `geist::{geist_light, geist_dark}`.
  `theme_with_font_overrides` applies global overrides first, then per-theme
  overrides. `silvestris/src/theme.rs` holds one `ThemeState`; switching only
  rebuilds the `Theme` and refreshes windows, not application state.

The four appearances before refinement:

| Theme | Typography family | Geometry identity | Surface identity |
| --- | --- | --- | --- |
| Aqua Light | Lato / Source Code Pro | 10.6 measured | measured Aqua |
| Aqua Dark | Lato / Source Code Pro | same metrics as Aqua Light | designed dark Aqua |
| Geist Light | Inter / Geist Mono | flatter, taller | designed light Geist |
| Geist Dark | Inter / Geist Mono | same as Geist Light | Lotus's inherited dark Geist |

Aqua typography and metrics, read from `crates/silvestris-core/src/aqua.rs`
before any change:

| Role | Aqua value |
| --- | --- |
| UI / mono family | Lato / Source Code Pro |
| Regular / small / mini | 13 / 11 / 9 |
| Mono / title / heading | 12 / 13 / 11 |
| Menu / view / toolbar label | 13 / 12 / 10 |
| Window radius / titlebar / toolbar | 4 / 22 / 32 |
| Labelled toolbar / bottom bar | 49 / 32 |
| Push heights / radius | 21 / 18 / 15, capsule (11) |
| Push padding / label dy | 16/14/11, -1/0/0 |
| Popup cap widths | 21 / 18 / 15 |
| Field heights / text inset | 22 / 19, 4 |
| Checkbox sizes / radius | 14 / 12 / 10, 2 |
| Radio sizes / dot fraction | 14 / 12 / 10, 0.24 |
| Source / table row heights | 20 / 18 |
| Menu item / radius | 18 / 5 |
| Scroller width | 15 / 11 / 9 |
| Panel radius | 5 |

Geist typography and geometry at baseline are recorded in
`crates/silvestris-core/src/geist.rs`: Inter, sizes 13/11/9, heading 11,
label 10, window radius 8, push radius 6, segment and text-field radii 6,
checkbox radius 4, scroller width 10/8/6.

## Change surface

- `silvestris-core`: theme identities, resolved tokens, the Geist palette,
  typography values, and theme-resolved corner policy. The palette is private
  to this crate; `Theme::colors` and the other token structs stay public.
- GPUI renderers in `crates/silvestris/src`: read resolved tokens. A Geist
  control uses the same implementation as its Aqua counterpart. Flat paint
  paths are added only where the layered Aqua painter is wrong for Geist.
- Font registration in `crates/silvestris-fonts` and
  `tools/fonts/manifest.py`: vendored faces, names, hashes and licenses.

`crates/silvestris-core/src/aqua.rs` is read-only for this project unless a
shared type needs a purely mechanical field addition. Any new field in a
shared token type must be initialized for Aqua with values that preserve its
existing output exactly. Every new theme-specific rendering decision uses the
resolved `ThemeId` or tokens, never an application name or an OS assumption.
Palette values belong in core, not in individual GPUI controls.

Out of scope: changing the meaning of `ThemeFamily`, `ThemeId` or
`ThemePreference`, a separate Geist widget crate, changing the default theme
or consumer configuration, mixing theme selection with system appearance
detection, upgrading GPUI, or changing unrelated public APIs.

The regression gates are: `ThemePreference::resolve` still honours system and
explicit appearances; global and per-theme font overrides keep their
precedence; theme switching refreshes windows without recreating state.

## Visual baseline

The clean-source functional baseline is `cargo test --workspace --locked`:
every test passed (182 unit tests in `silvestris`, 45 in `silvestris-core`,
2 in `silvestris-fonts`, plus integration and doc tests). The log is kept at
`target/geist-baseline-tests.log` (a local artifact).

The committed goldens were rendered into `target/goldens` at 100, 125, 150,
175 and 200% for all four appearances. The acceptance gate before changes:

```text
1440 goldens, 90 failed
aqua-light: 30 failures, aqua-dark: 30,
geist-light: 15, geist-dark: 15
```

All 90 failures are pre-existing host drift in six controls: `popup-button`,
`popup-disabled`, `popup-textured`, `segmented-icon`, `source-list-secondary`
and `table-view`. They are recorded as pre-existing defects, not regressions
from this refinement. They are not accepted or regenerated. Aqua Light and
Aqua Dark captures at all five scales remain the reference for later
comparison. The current Geist captures are preserved under `docs/geist-before`
as *before* references; they are not the desired final appearance.

The capture set covers buttons, fields, checkboxes, radio buttons, segmented
controls, popups, menus, scrollbars, source lists, tables, toolbars,
preferences, split views and window frames. Native Windows 11 window captures
(active/inactive, maximized/floating, all four appearances) and the exact
DWM corner composition are not reproducible in the headless capture path;
they need `tools/capture/capture-window.ps1` on an interactive Windows 11
host and are recorded as a verification limitation, not as a passing gate.

Do not regenerate accepted Aqua images because a candidate differs, run the
gate with `--accept` automatically, replace historical Aqua references with
Geist screenshots, or compare screenshots at mismatched scales. Font
registration changes can silently substitute faces; rounded borders can
change by one pixel at fractional scaling; offscreen rendering does not prove
native window transparency.

## Checkpoint: typography and font registration

Geist Sans is vendored from the same pinned release as Geist Mono,
`vercel/geist-font` v1.9.2 (archive SHA-256
`15f066d58dcb99fb10014a49913009fdaf41e464cfb8851a44b44e593c95d3d8`).
Static TTF faces for Regular 400, Italic 400, Medium 500, SemiBold 600 and
Bold 700 live in `crates/silvestris-fonts/fonts/geist-sans`. The internal
names were inspected with `tools/fonts/sfnt_names.py`: `Geist-Regular.ttf`
and `Geist-Italic.ttf` report family "Geist"; the Medium and SemiBold files
have legacy family names "Geist Medium" and "Geist SemiBold" but declare the
typographic family "Geist" (name ID 16), which is the registered family.
Inter remains bundled as an auxiliary family but is no longer Geist's UI
font. Aqua's Lato and Source Code Pro registrations are untouched.

Geist typography roles are now regular 13, small 12, mini 11, title 13,
heading 12 semibold, menu 13, list/table 13, toolbar label 11 and mono 12.
Section headings use 600 only for Geist; Aqua keeps its measured bold
heading. No global line-height override was added.

`cargo run -p silvestris-demo --features capture --bin measure_typography`
records width and ascent/descent for Latin, digits, punctuation, ellipses,
paths and accented strings at 100, 125, 150, 175 and 200% into
[`geist-typography/measurements.json`](geist-typography/measurements.json).
At 100%, 13px Geist Sans measures 41.7 px for "Cancel" with a 16.9 px
line box, leaving 5.5 px above and below inside a 28px push button. The
values are logical and scale linearly; the JSON records both logical and
device pixels. Headless GPUI tests use a no-op text system, so native font
registration is verified by the real capture path, not by those tests.

## Checkpoint: palette, surfaces and control states

`crates/silvestris-core/src/geist.rs` now contains one private `GeistPalette`.
`dark()` and `light()` hold the documented values; `resolve()` maps both through
the same component recipe, so Light is not a separate collection of recipes.
Nothing in a GPUI renderer reads palette values directly.

| Palette role | Dark | Light | Token destinations |
| --- | --- | --- | --- |
| document | `#000000` | `#FFFFFF` | `content`, `canvas`, field wells |
| window | `#0A0A0A` | `#FAFAFA` | `window`, chrome, toolbar, sidebar |
| raised | `#111111` | `#FFFFFF` | preference groups, inset surfaces |
| control / hover / pressed | `#171717` / `#1F1F1F` / `#292929` | `#FFFFFF` / `#FAFAFA` / `#F2F2F2` | bevel states and `Controls::hover` |
| border / hover / strong | `#262626` / `#393939` / `#505050` | `#E6E6E6` / `#C9C9C9` / `#8F8F8F` | bevel borders, menu border, focus separation |
| text / secondary / disabled | `#EDEDED` / `#A1A1A1` / `#878787` | `#171717` / `#666666` / `#8F8F8F` | text, headings, placeholders |
| focus | `#0095FF` | `#0070F3` | `accent`, `link`, `focus_ring`, progress |
| selection | `#0B3A5C` | `#E6F1FE` | table selection, text selection |
| action / action text | `#EDEDED` / `#000000` | `#171717` / `#FFFFFF` | default button and selected checkbox |
| thumb / hover / inactive | `#555555` / `#666666` / `#393939` | `#A0A0A0` / `#7B7B7B` / `#C9C9C9` | scrollbar thumb materials |

Shadows and scrims are the only palette values with alpha. Selected table
rows use the selection role and stay distinct from a pressed neutral control.
Popup caps, segmented selections and ordinary controls use neutral surfaces;
blue is limited to focus, links, selection, progress and meaningful selected
states. The popup cap no longer has a blue fill.

Aqua retains its measured capsule geometry through the new GPUI-free
`CornerRule`: `Capsule` for segments and fields, with hover and scrollbar
fields set to the existing Aqua materials. Geist uses `Fixed(4)` for segments
and ordinary fields, `Fixed(6)` for search fields, a 4px push and textured
radius, a 6px menu and panel radius, a 3px checkbox radius, and a 4px
progress radius. The installer Continue control and radio buttons stay
circular. These shared-type additions are mechanical and are covered by
explicit assertions in `crates/silvestris-core/tests/aqua_isolation.rs`; the
Aqua fixtures were regenerated only to include the new fields.

## Implementation across the shared controls

- Buttons and textured controls keep 28/24/20 px heights and 12/10/8 px
  padding. Their radius comes from `metrics.push_radius` and
  `metrics.textured_radius`; Aqua's measured values are unchanged. Popup caps
  keep 18/16/14 px and now use the neutral body material with a subtle
  divider, not a blue accent. The installer Continue control remains circular
  and uses the primary-action material for Geist.
- Segments and fields resolve `CornerRule` through the renderer, including
  the focus ring: Geist segments and normal fields use 4 px, search fields
  6 px, and Aqua keeps half-height capsules.
- The bevel and group-box painters take a flat path only when a material has
  one fill, one border and no emboss; Aqua's layered path is unchanged. Group
  boxes use `metrics.group_radius` without changing insets.
- Checkboxes use a 3 px radius, radios stay circular, slider tracks and thumbs
  use neutral materials, and progress uses a 4 px radius. Indeterminate
  progress keeps the existing animation timing but draws a flat moving accent
  block instead of Aqua's 45 degree barber-pole shading.
- Preferences keep 668 px width, 81 px cells, 100 px bands, 32 px icons, 20 px
  margins, an 8 px row gap and a right-aligned label column. Geist headings
  are 12 px semibold secondary text, category labels use a 15 px line box,
  and notes use a 16 px line box. The 2 px control-to-note gap is unchanged.
- Source lists, tables, toolbars, bottom bars, tab views, menus and split
  views read the same palette; row heights and native menu semantics are
  unchanged.

## Scrollbars and artwork

`Scrollbar` now resolves a presentation from `ThemeId`. Aqua keeps its whole
painter and arrow preferences. Geist draws no track and no arrows: a 6 px
rounded thumb is centred in the 10 px allocation with a 2 px end inset and an
18 px minimum length. The track geometry is a separate function, so the
original `ScrollSource` pointer model, grab offset, page clicks, jump-to-spot
and drag notifications are shared. `preferences::pane_viewport_with_scroll`
and `PreferencesOverview::scroll_handle` let hosts attach that bar to native
overflow views without changing the existing call sites; the native bar's
reserved width becomes zero when the Geist bar is mounted.

Geist interface artwork uses ten generic pictograms from Lucide 0.468.0
(archive SHA-256 `2fe6e35a57404dab89adaddd6df49ec60d13bba3d6caee5d8805f885079a1c57`).
`tools/icons/geist_pictograms.py` converts the pinned SVGs to static polylines
once; nothing parses SVG during a frame. The roles are settings, database,
message-square, monitor, volume-2, gamepad-2, bug, bot, wrench and
sliders-horizontal. Ink comes from theme text tokens, including disabled and
inactive states. File artwork and consumer artwork keep their own vocabularies.

## Native frame behaviour

The GPUI root uses `metrics.window_radius` (Aqua 4, Geist 6) and clips the
titlebar, body and bottom bar with one `overflow_hidden` root. Maximized
windows use radius 0. Linux client decorations keep their transparent margin
and drawn shadow. Windows requests `DWMWCP_ROUNDSMALL` and records only that
the request was accepted in `FrameCapabilities`; it does not claim the
compositor honoured an exact pixel radius. Native corner captures over light
and dark desktops at 100/125/150/200% still need an interactive Windows 11
host and are a documented verification limitation, not a passing gate.

## Verification in this environment

- `cargo test --workspace --locked` passed at the palette checkpoint.
- `cargo test -p silvestris-core`, the Aqua isolation fixtures, the Geist
  contrast checks, font metadata checks, scrollbar interaction tests and theme
  switching tests pass.
- Candidate Geist goldens are rendered with `pwsh tools/metrology/render.ps1`
  and reported by `tools/metrology/gate.py`. Aqua differences are reported as
  forbidden; Geist differences are reported as awaiting review. Final golden
  acceptance is deliberately left to the user after visual inspection.
