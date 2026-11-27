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
