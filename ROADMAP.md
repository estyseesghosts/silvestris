# Silvestris roadmap

Silvestris is the GUI layer for lotus, sandcat, chaus and johjoh, and for any future app in the same style. Apps compose and extend Silvestris; they do not paint their own controls. Every bespoke widget in a consumer is either a gap in Silvestris (tracked below) or a deliberate app-owned domain view (listed as such).

Work is chosen by three inputs: what consumers are drawing themselves today (the gap register), what the Mac OS X 10.6 reference corpus in hig-09 shows, and what the HIG (2009) specifies. Three reference apps (a Finder-style file manager, the Installer/Setup Assistant, and System Preferences) are the acceptance tests: if one needs a private widget, Silvestris is missing a component.

Adoption status: lotus, chaus and johjoh are active consumers. Sandcat currently builds against an uncommitted Silvestris checkout; it is not a gating consumer, and its adoption is deferred until Silvestris's own foundations land. Sandcat rows in the gap register are demand evidence, not a schedule.

Tracking issue: [#72](https://github.com/estyseesghosts/silvestris/issues/72). Coverage: `docs/coverage.md` ([#52](https://github.com/estyseesghosts/silvestris/issues/52)). Consumer rules and pins: `docs/consumers.md` ([#74](https://github.com/estyseesghosts/silvestris/issues/74)). Public API changes per commit range: `CHANGELOG.md`.

## Layers

1. **Quality**: regression gates, harnesses and documentation that every other layer relies on.
2. **Foundation**: responders, transient surfaces, modality, window roles, text core, undo, documents, lifecycle, preferences store.
3. **Control**: individual controls measured against 10.6.
4. **View**: scrollers, tables, outlines, browsers, text views, splits, drag and drop.
5. **Window**: sheets, alerts, menus, toolbars, bottom bars, panels, file panels, assistants, preferences windows.
6. **Reference app**: whole applications built only from public APIs.

## M0 · Ground truth

Clean tree, consumer pin contract, visual and interaction regression gates, coverage matrix.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#18](https://github.com/estyseesghosts/silvestris/issues/18) | Visual regression gate: accepted 10.6 reference crops, Silvestris goldens and a diff tool | P0 | quality | – |
| [#68](https://github.com/estyseesghosts/silvestris/issues/68) | Interaction scenario harness: scripted focus, keyboard, menu and window lifecycle tests | P0 | quality | – |
| [#73](https://github.com/estyseesghosts/silvestris/issues/73) | Triage the uncommitted working tree: format to the style guides, keep real hunks, discard the TextArea prototype | P0 | quality | – |
| [#74](https://github.com/estyseesghosts/silvestris/issues/74) | Upstream lotus's vendored Silvestris patches and define the consumer pinning contract | P0 | quality | [#73](https://github.com/estyseesghosts/silvestris/issues/73) |
| [#52](https://github.com/estyseesghosts/silvestris/issues/52) | Coverage matrix, consumer gap register and public API guide | P1 | quality | – |

## M1 · Foundations

Responders, transient UI, modality, window roles, shared text core, undo, documents and lifecycle.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#20](https://github.com/estyseesghosts/silvestris/issues/20) | Window roles: document, utility, panel, preferences, dialog, assistant and About windows | P0 | foundation | [#68](https://github.com/estyseesghosts/silvestris/issues/68) |
| [#57](https://github.com/estyseesghosts/silvestris/issues/57) | Responder chain: focused command routing from control to window to app | P0 | foundation | [#68](https://github.com/estyseesghosts/silvestris/issues/68) |
| [#60](https://github.com/estyseesghosts/silvestris/issues/60) | Transient surface manager: menus, popovers, tooltips and completions with one dismissal model | P0 | foundation | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| [#21](https://github.com/estyseesghosts/silvestris/issues/21) | Modal sessions: app-modal dialogs, document-modal sheets, focus trap and command blocking | P0 | foundation | [#57](https://github.com/estyseesghosts/silvestris/issues/57), [#60](https://github.com/estyseesghosts/silvestris/issues/60) |
| [#58](https://github.com/estyseesghosts/silvestris/issues/58) | Shared text-editing core beneath TextField, TextArea, token fields and editable cells | P0 | foundation | [#57](https://github.com/estyseesghosts/silvestris/issues/57), [#73](https://github.com/estyseesghosts/silvestris/issues/73) |
| [#37](https://github.com/estyseesghosts/silvestris/issues/37) | Document controller: identity, dirty state, save/revert and close coordination | P0 | foundation | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#21](https://github.com/estyseesghosts/silvestris/issues/21), [#39](https://github.com/estyseesghosts/silvestris/issues/39) |
| [#62](https://github.com/estyseesghosts/silvestris/issues/62) | Preferences store: schema versions, migration and observable settings | P1 | foundation | – |
| [#12](https://github.com/estyseesghosts/silvestris/issues/12) | Device-pixel snapping for one-pixel Aqua edges at fractional scales | P1 | foundation | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#61](https://github.com/estyseesghosts/silvestris/issues/61) | Typed bindings and collection controllers over GPUI entities | P1 | foundation | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| [#66](https://github.com/estyseesghosts/silvestris/issues/66) | Runtime shortcut rebinding with conflict detection | P1 | foundation | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| [#39](https://github.com/estyseesghosts/silvestris/issues/39) | Undo manager: grouped undo/redo with per-document and per-editor stacks | P1 | foundation | [#57](https://github.com/estyseesghosts/silvestris/issues/57), [#58](https://github.com/estyseesghosts/silvestris/issues/58) |
| [#59](https://github.com/estyseesghosts/silvestris/issues/59) | Application lifecycle: launch, reopen, activation and coordinated quit | P1 | foundation | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#37](https://github.com/estyseesghosts/silvestris/issues/37) |

## M2 · Control fidelity

Every shipped control measured against Mac OS X 10.6 in all four appearances.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#3](https://github.com/estyseesghosts/silvestris/issues/3) | Source list: 10.6 sidebar materials, section headers and selection highlight | P0 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#4](https://github.com/estyseesghosts/silvestris/issues/4) | Source list rows: badges, eject/action accessories, status dots and hover | P0 | control | [#3](https://github.com/estyseesghosts/silvestris/issues/3) |
| [#5](https://github.com/estyseesghosts/silvestris/issues/5) | Disclosure triangles: measured 10.6 silhouettes and rotation | P0 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#1](https://github.com/estyseesghosts/silvestris/issues/1) | Pop-up button: measured 10.6 enabled bezel, arrow cap and states | P0 | control | [#12](https://github.com/estyseesghosts/silvestris/issues/12), [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#6](https://github.com/estyseesghosts/silvestris/issues/6) | Radio buttons: dedicated 10.6 bezel and selected dot | P0 | control | [#12](https://github.com/estyseesghosts/silvestris/issues/12), [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#7](https://github.com/estyseesghosts/silvestris/issues/7) | Slider: 10.6 inset track, metallic thumb, tick marks and focus | P0 | control | [#12](https://github.com/estyseesghosts/silvestris/issues/12), [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#2](https://github.com/estyseesghosts/silvestris/issues/2) | Textured toolbar pop-up variant with its own indicator artwork | P1 | control | [#1](https://github.com/estyseesghosts/silvestris/issues/1) |
| [#8](https://github.com/estyseesghosts/silvestris/issues/8) | Progress indicators: 10.6 determinate bar, barber pole and spinners | P1 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#9](https://github.com/estyseesghosts/silvestris/issues/9) | Default push button: pulsing blue throb and default/cancel key semantics | P1 | control | [#10](https://github.com/estyseesghosts/silvestris/issues/10) |
| [#11](https://github.com/estyseesghosts/silvestris/issues/11) | Segmented control: icon segments, small/mini sizes, momentary and textured variants | P1 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#13](https://github.com/estyseesghosts/silvestris/issues/13) | Tab views, group boxes, separators and inset wells | P1 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#10](https://github.com/estyseesghosts/silvestris/issues/10) | Push buttons: measured pressed, disabled, inactive and small/mini states | P1 | control | [#12](https://github.com/estyseesghosts/silvestris/issues/12), [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#17](https://github.com/estyseesghosts/silvestris/issues/17) | Active/inactive window and focus/selection appearance audit | P1 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18), [#20](https://github.com/estyseesghosts/silvestris/issues/20) |
| [#70](https://github.com/estyseesghosts/silvestris/issues/70) | Four-appearance conformance and live theme switching | P1 | quality | [#18](https://github.com/estyseesghosts/silvestris/issues/18), [#68](https://github.com/estyseesghosts/silvestris/issues/68) |
| [#19](https://github.com/estyseesghosts/silvestris/issues/19) | Checkbox: tick and mixed glyph calibration, small/mini layout | P2 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18) |
| [#16](https://github.com/estyseesghosts/silvestris/issues/16) | Menus: 10.6 borders, shadow, rounded corners, separators and submenu geometry | P2 | control | [#18](https://github.com/estyseesghosts/silvestris/issues/18), [#60](https://github.com/estyseesghosts/silvestris/issues/60) |

## M3 · Collections and text views

Scrollers, tables, outlines, column browser, multiline text, split views, drag and drop.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#40](https://github.com/estyseesghosts/silvestris/issues/40) | Scrollers: horizontal bar, corner, and any scroll source (ScrollHandle, ListState, uniform lists) | P0 | view | [#12](https://github.com/estyseesghosts/silvestris/issues/12) |
| [#36](https://github.com/estyseesghosts/silvestris/issues/36) | Drag and drop: sessions, drag images, drop targets, autoscroll and OS file drops | P0 | view | [#57](https://github.com/estyseesghosts/silvestris/issues/57), [#60](https://github.com/estyseesghosts/silvestris/issues/60) |
| [#32](https://github.com/estyseesghosts/silvestris/issues/32) | Table view: headers, sort, column resize, selection, alternating rows, inline edit, virtualisation | P0 | view | [#40](https://github.com/estyseesghosts/silvestris/issues/40), [#58](https://github.com/estyseesghosts/silvestris/issues/58), [#61](https://github.com/estyseesghosts/silvestris/issues/61) |
| [#38](https://github.com/estyseesghosts/silvestris/issues/38) | Multiline text view and read-only formatted text (logs, licences, transcripts) | P0 | view | [#39](https://github.com/estyseesghosts/silvestris/issues/39), [#40](https://github.com/estyseesghosts/silvestris/issues/40), [#58](https://github.com/estyseesghosts/silvestris/issues/58) |
| [#53](https://github.com/estyseesghosts/silvestris/issues/53) | Split views: multi-pane, thin and thick Aqua dividers, collapse and persistence | P1 | view | [#12](https://github.com/estyseesghosts/silvestris/issues/12) |
| [#33](https://github.com/estyseesghosts/silvestris/issues/33) | Outline view: hierarchical table with disclosure, independent of source lists | P1 | view | [#5](https://github.com/estyseesghosts/silvestris/issues/5), [#32](https://github.com/estyseesghosts/silvestris/issues/32) |
| [#34](https://github.com/estyseesghosts/silvestris/issues/34) | Column browser: Finder/Open-panel multi-column navigation with preview | P1 | view | [#32](https://github.com/estyseesghosts/silvestris/issues/32), [#40](https://github.com/estyseesghosts/silvestris/issues/40) |
| [#77](https://github.com/estyseesghosts/silvestris/issues/77) | Inline find bar and search field: match count, next/previous, incremental highlight | P1 | control | [#38](https://github.com/estyseesghosts/silvestris/issues/38), [#58](https://github.com/estyseesghosts/silvestris/issues/58) |

## M4 · Windows, sheets and panels

Sheets, alerts, menus, toolbar, panels, Open/Save, assistants, preferences windows, forms.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#23](https://github.com/estyseesghosts/silvestris/issues/23) | Alerts: standard layouts, icon badge, button order and suppression checkbox | P0 | window | [#22](https://github.com/estyseesghosts/silvestris/issues/22) |
| [#24](https://github.com/estyseesghosts/silvestris/issues/24) | Menu bar composition: standard application, File, Edit, View, Window and Help menus | P0 | window | [#57](https://github.com/estyseesghosts/silvestris/issues/57), [#66](https://github.com/estyseesghosts/silvestris/issues/66) |
| [#30](https://github.com/estyseesghosts/silvestris/issues/30) | Assistant workflow engine: step pages, validation, Go Back/Continue, conclusion | P0 | window | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#54](https://github.com/estyseesghosts/silvestris/issues/54) |
| [#22](https://github.com/estyseesghosts/silvestris/issues/22) | Document sheets attached to the title bar, built on modal sessions | P0 | window | [#9](https://github.com/estyseesghosts/silvestris/issues/9), [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#21](https://github.com/estyseesghosts/silvestris/issues/21) |
| [#29](https://github.com/estyseesghosts/silvestris/issues/29) | Open, Save and Choose panels with a pluggable file provider | P0 | window | [#3](https://github.com/estyseesghosts/silvestris/issues/3), [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#32](https://github.com/estyseesghosts/silvestris/issues/32), [#34](https://github.com/estyseesghosts/silvestris/issues/34) |
| [#14](https://github.com/estyseesghosts/silvestris/issues/14) | Preferences window: animated resize to each pane's content, no scroll traps | P1 | window | [#20](https://github.com/estyseesghosts/silvestris/issues/20) |
| [#25](https://github.com/estyseesghosts/silvestris/issues/25) | Menu tracking: hover corridor to submenus, type-select and edge scrolling | P1 | window | [#60](https://github.com/estyseesghosts/silvestris/issues/60) |
| [#54](https://github.com/estyseesghosts/silvestris/issues/54) | Forms: label alignment, inline validation marks, dependent fields | P1 | window | [#61](https://github.com/estyseesghosts/silvestris/issues/61) |
| [#26](https://github.com/estyseesghosts/silvestris/issues/26) | Toolbar: item model, overflow, display modes and customisation palette | P1 | window | [#2](https://github.com/estyseesghosts/silvestris/issues/2), [#36](https://github.com/estyseesghosts/silvestris/issues/36) |
| [#27](https://github.com/estyseesghosts/silvestris/issues/27) | Utility panels and inspectors: floating, hide on deactivate, Get Info style | P1 | window | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| [#75](https://github.com/estyseesghosts/silvestris/issues/75) | Bottom bar: gradient +/−/action buttons, status text and resize-safe layout | P1 | window | [#10](https://github.com/estyseesghosts/silvestris/issues/10), [#12](https://github.com/estyseesghosts/silvestris/issues/12) |
| [#76](https://github.com/estyseesghosts/silvestris/issues/76) | Spotlight-style search menu and command palette over the command registry | P1 | control | [#57](https://github.com/estyseesghosts/silvestris/issues/57), [#60](https://github.com/estyseesghosts/silvestris/issues/60) |
| [#55](https://github.com/estyseesghosts/silvestris/issues/55) | Fonts and Colors panels with integration hooks | P2 | window | [#27](https://github.com/estyseesghosts/silvestris/issues/27) |
| [#15](https://github.com/estyseesghosts/silvestris/issues/15) | Preferences extras: help button, lock/authorisation button, Spotlight search highlighting | P2 | window | [#14](https://github.com/estyseesghosts/silvestris/issues/14), [#76](https://github.com/estyseesghosts/silvestris/issues/76) |
| [#28](https://github.com/estyseesghosts/silvestris/issues/28) | Drawers attached to window edges | P2 | window | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#22](https://github.com/estyseesghosts/silvestris/issues/22) |
| [#47](https://github.com/estyseesghosts/silvestris/issues/47) | Standard dialog shells: About, progress (copy) window, Print and Page Setup | P2 | window | [#8](https://github.com/estyseesghosts/silvestris/issues/8), [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#27](https://github.com/estyseesghosts/silvestris/issues/27) |

## M5 · Specialty controls and artwork

Icon grid, path, date, token, wells, indicators, scope bars and the artwork vocabulary.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#45](https://github.com/estyseesghosts/silvestris/issues/45) | Indicators and live metrics: level, rating, relevance, capacity, history graph | P1 | control | – |
| [#48](https://github.com/estyseesghosts/silvestris/issues/48) | Artwork vocabulary and icon API: system images, toolbar templates, cursors, consumer icon sets | P1 | control | – |
| [#41](https://github.com/estyseesghosts/silvestris/issues/41) | Path control and path pop-up: clickable breadcrumbs | P1 | control | [#48](https://github.com/estyseesghosts/silvestris/issues/48) |
| [#46](https://github.com/estyseesghosts/silvestris/issues/46) | Scope bars, action (gear) menus and command pop-down buttons | P1 | control | [#26](https://github.com/estyseesghosts/silvestris/issues/26) |
| [#42](https://github.com/estyseesghosts/silvestris/issues/42) | Date and time pickers: textual stepper fields, graphical calendar and clock | P1 | control | [#58](https://github.com/estyseesghosts/silvestris/issues/58), [#60](https://github.com/estyseesghosts/silvestris/issues/60) |
| [#35](https://github.com/estyseesghosts/silvestris/issues/35) | Icon grid view: Finder icon view with arrange, labels, marquee selection | P1 | view | [#36](https://github.com/estyseesghosts/silvestris/issues/36), [#48](https://github.com/estyseesghosts/silvestris/issues/48), [#61](https://github.com/estyseesghosts/silvestris/issues/61) |
| [#43](https://github.com/estyseesghosts/silvestris/issues/43) | Token field: tokens with menus, keyboard editing and removal | P2 | control | [#58](https://github.com/estyseesghosts/silvestris/issues/58) |
| [#44](https://github.com/estyseesghosts/silvestris/issues/44) | Colour wells and image wells | P2 | control | [#36](https://github.com/estyseesghosts/silvestris/issues/36), [#55](https://github.com/estyseesghosts/silvestris/issues/55) |

## M6 · Reference apps and adoption

Finder, Installer and System Preferences built from public APIs; accessibility sign-off.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#51](https://github.com/estyseesghosts/silvestris/issues/51) | Accessibility and keyboard contract audit across all controls | P1 | quality | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| [#31](https://github.com/estyseesghosts/silvestris/issues/31) | Reference app: Installer and Setup Assistant flow | P1 | reference app | [#8](https://github.com/estyseesghosts/silvestris/issues/8), [#23](https://github.com/estyseesghosts/silvestris/issues/23), [#30](https://github.com/estyseesghosts/silvestris/issues/30), [#38](https://github.com/estyseesghosts/silvestris/issues/38) |
| [#56](https://github.com/estyseesghosts/silvestris/issues/56) | Reference app: System Preferences with real panes | P1 | reference app | [#7](https://github.com/estyseesghosts/silvestris/issues/7), [#13](https://github.com/estyseesghosts/silvestris/issues/13), [#14](https://github.com/estyseesghosts/silvestris/issues/14), [#15](https://github.com/estyseesghosts/silvestris/issues/15), [#42](https://github.com/estyseesghosts/silvestris/issues/42), [#44](https://github.com/estyseesghosts/silvestris/issues/44), [#45](https://github.com/estyseesghosts/silvestris/issues/45), [#54](https://github.com/estyseesghosts/silvestris/issues/54), [#62](https://github.com/estyseesghosts/silvestris/issues/62) |
| [#49](https://github.com/estyseesghosts/silvestris/issues/49) | Reference app: Finder-style file manager from public APIs only | P1 | reference app | [#26](https://github.com/estyseesghosts/silvestris/issues/26), [#27](https://github.com/estyseesghosts/silvestris/issues/27), [#29](https://github.com/estyseesghosts/silvestris/issues/29), [#32](https://github.com/estyseesghosts/silvestris/issues/32), [#33](https://github.com/estyseesghosts/silvestris/issues/33), [#34](https://github.com/estyseesghosts/silvestris/issues/34), [#35](https://github.com/estyseesghosts/silvestris/issues/35), [#36](https://github.com/estyseesghosts/silvestris/issues/36), [#41](https://github.com/estyseesghosts/silvestris/issues/41), [#46](https://github.com/estyseesghosts/silvestris/issues/46), [#75](https://github.com/estyseesghosts/silvestris/issues/75) |

## M7 · Platform breadth and release

macOS/Linux parity, host services, locale, text services, packaging.

| # | Issue | Pri | Layer | Depends on |
|---|---|---|---|---|
| [#50](https://github.com/estyseesghosts/silvestris/issues/50) | Platform matrix: Windows, macOS and Linux behaviour and fidelity | P1 | quality | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#68](https://github.com/estyseesghosts/silvestris/issues/68) |
| [#69](https://github.com/estyseesghosts/silvestris/issues/69) | Consumer packaging: redistributable fonts and artwork, licences, GPUI compatibility | P1 | quality | [#48](https://github.com/estyseesghosts/silvestris/issues/48), [#74](https://github.com/estyseesghosts/silvestris/issues/74) |
| [#64](https://github.com/estyseesghosts/silvestris/issues/64) | Text services: spelling, substitutions and rich clipboard interchange | P2 | foundation | [#58](https://github.com/estyseesghosts/silvestris/issues/58) |
| [#65](https://github.com/estyseesghosts/silvestris/issues/65) | Locale formatting, bidirectional layout and keyboard-layout-aware shortcuts | P2 | foundation | [#58](https://github.com/estyseesghosts/silvestris/issues/58) |
| [#63](https://github.com/estyseesghosts/silvestris/issues/63) | Host service adapters: native file dialogs, notifications, single instance, open-with | P2 | foundation | [#29](https://github.com/estyseesghosts/silvestris/issues/29), [#59](https://github.com/estyseesghosts/silvestris/issues/59) |

## Definition of done

An issue is done only when all of these hold (skip a line only when it cannot apply, and say why in the closing comment):

1. `cargo fmt --all -- --check` with the repository `rustfmt.toml` (style guides: 100 columns, lowercase hex
   literals, macro matchers), `cargo clippy --workspace --all-targets --locked -- -D warnings` and
   `cargo test --workspace --locked` pass.
2. Behaviour has tests: unit tests for pure logic, scenario tests ([#68](https://github.com/estyseesghosts/silvestris/issues/68)) for focus, keyboard, menus, modality and windows.
3. Visual work has Silvestris goldens ([#18](https://github.com/estyseesghosts/silvestris/issues/18)) at 100/150/200% (plus 125% for hairline-heavy controls) in all four
   appearances, and cites the 10.6 crop ids it was measured from. Anything estimated is listed in `docs/exceptions.md`.
4. The gallery has a sample of every new public type and state.
5. `docs/coverage.md` row updated; public API documented (rustdoc + the relevant `docs/*.md`).
6. No consumer-specific names, types or policies inside Silvestris; at least one consumer or reference app uses the
   public API (or a consumer issue is filed to adopt it).
7. Each commit is self-contained, formatted separately from behaviour changes, and the tree is clean after it.

## Issue anatomy and labels

Every issue uses the same sections, in this order:

- **Header line**: milestone, layer, priority, consumers, dependencies, what it unblocks.
- **Why**: the gap, the current code, and which consumers or reference apps need it (with file names).
- **References**: HIG pages (hig-09 notes/specs) and corpus captures. Corpus paths are relative to a local hig-09
  checkout; the captures are not redistributed.
- **Deliverables**: checkable, concrete outputs (types, behaviours, docs, migrations).
- **Avoid**: scope and approaches that are explicitly out, usually because another issue owns them or because they
  recreate bespoke behaviour.
- **Regression traps**: existing behaviour (often a consumer's) that the change can break, each with what to test.
- **Done when**: acceptance checks, plus the definition of done below.

Labels: `layer: foundation | control | view | window | reference app | quality`, `P0` (blocks consumers or other
issues), `P1` (needed by a reference app or consumer soon), `P2` (completeness and polish), `area: …`, and
`consumer: lotus | sandcat | chaus | johjoh` for the apps that are waiting on it.

## Consumer gap register

Survey of 2026-11-23. Each consumer-side bespoke element maps to the issue that replaces it; adoption happens in the consumer's repository once the Silvestris issue lands. Sandcat rows are inventory only while its adoption is deferred.

| Consumer | Bespoke today | Replaced by |
|---|---|---|
| lotus | `settings_window/shared.rs` `fit_size`/`fit_pending` pane resizing | [#14](https://github.com/estyseesghosts/silvestris/issues/14) |
| lotus | `settings_window/controllers.rs`, `field_text`/`parse_field` | [#61](https://github.com/estyseesghosts/silvestris/issues/61), [#54](https://github.com/estyseesghosts/silvestris/issues/54) |
| lotus | `settings_window/bindings.rs` shortcut editor | [#66](https://github.com/estyseesghosts/silvestris/issues/66) |
| lotus | `gauges.rs` stat cards, pills, sparklines | [#45](https://github.com/estyseesghosts/silvestris/issues/45) |
| lotus | `icons.rs` 16 px stroke icon set, `group_icon` | [#48](https://github.com/estyseesghosts/silvestris/issues/48) |
| lotus | `lotus_window/command_palette.rs` | [#76](https://github.com/estyseesghosts/silvestris/issues/76) |
| lotus | `tiling.rs` edge drop zones, drag previews | [#36](https://github.com/estyseesghosts/silvestris/issues/36) (primitives); tiling policy stays app-owned |
| lotus | Split stage dividers | [#53](https://github.com/estyseesghosts/silvestris/issues/53) |
| lotus | Memory, register, breakpoint, kernel, snapshot tables; event timeline | [#32](https://github.com/estyseesghosts/silvestris/issues/32), [#33](https://github.com/estyseesghosts/silvestris/issues/33), [#40](https://github.com/estyseesghosts/silvestris/issues/40) |
| lotus | `state_dot`, row accessories in the session list | [#4](https://github.com/estyseesghosts/silvestris/issues/4) |
| lotus | Status bar values (`status_value`), toolbar ([lotus#133](https://github.com/estyseesghosts/lotus/issues/133)) | [#75](https://github.com/estyseesghosts/silvestris/issues/75), [#26](https://github.com/estyseesghosts/silvestris/issues/26) |
| lotus | Secondary windows (investigation, regression, record/test) ([lotus#134](https://github.com/estyseesghosts/lotus/issues/134)) | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#27](https://github.com/estyseesghosts/silvestris/issues/27) |
| lotus | Flat legacy primary action buttons | [#10](https://github.com/estyseesghosts/silvestris/issues/10) |
| lotus | Vendored Silvestris patches ([lotus#141](https://github.com/estyseesghosts/lotus/issues/141) inspector, [lotus#74](https://github.com/estyseesghosts/lotus/issues/74) focus deferral, [lotus#58](https://github.com/estyseesghosts/lotus/issues/58) Settings bounds) | [#74](https://github.com/estyseesghosts/silvestris/issues/74) |
| lotus | PICA texture/preview rendering, debugger domain panels | app-owned (domain views composed from Silvestris tables/splits) |
| sandcat | `apps/desktop/src/scrollbar.rs` Aqua scroller over `ListState` | [#40](https://github.com/estyseesghosts/silvestris/issues/40) |
| sandcat | Composer on the discarded `TextArea` prototype | [#58](https://github.com/estyseesghosts/silvestris/issues/58) (new multiline API); sandcat re-adopts after the foundations land |
| sandcat | `body_segments` / code-block rendering in transcript | [#38](https://github.com/estyseesghosts/silvestris/issues/38) |
| sandcat | `status_label`/`status_color` session badges, `project_tone` | [#4](https://github.com/estyseesghosts/silvestris/issues/4), [#48](https://github.com/estyseesghosts/silvestris/issues/48) |
| sandcat | First-run / server pairing setup state | [#30](https://github.com/estyseesghosts/silvestris/issues/30) |
| sandcat | Permission prompts and forms | [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#23](https://github.com/estyseesghosts/silvestris/issues/23), [#54](https://github.com/estyseesghosts/silvestris/issues/54) |
| sandcat | Theme/model/agent pickers in toolbar | [#2](https://github.com/estyseesghosts/silvestris/issues/2), [#76](https://github.com/estyseesghosts/silvestris/issues/76) |
| sandcat | `!SilvestrisTextField` key-context exclusions | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| sandcat | Shutdown draft flush | [#59](https://github.com/estyseesghosts/silvestris/issues/59) |
| chaus | Find bar (`editor_find.rs`) | [#77](https://github.com/estyseesghosts/silvestris/issues/77) |
| chaus | Close / conflict / recovery prompts (`editor_close.rs`, `editor_conflict.rs`, `editor_recovery.rs`) | [#37](https://github.com/estyseesghosts/silvestris/issues/37), [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#23](https://github.com/estyseesghosts/silvestris/issues/23) (policies stay app-owned) |
| chaus | Menus assembled by hand (`tests/menus.rs`) | [#24](https://github.com/estyseesghosts/silvestris/issues/24) |
| chaus | Startup-only shortcut bindings | [#66](https://github.com/estyseesghosts/silvestris/issues/66) |
| chaus | Open/Save | [#29](https://github.com/estyseesghosts/silvestris/issues/29) |
| chaus | Locally registered Geist Sans house font (`chaus-app/assets/fonts`) | PR [#71](https://github.com/estyseesghosts/silvestris/issues/71) (font overrides), then [#70](https://github.com/estyseesghosts/silvestris/issues/70) |
| chaus | Ropey editor, Vim engine, highlighting | app-owned by design (may reuse [#58](https://github.com/estyseesghosts/silvestris/issues/58) IME/selection traits) |
| johjoh | `heading`/`caption`/`group` helpers | [#13](https://github.com/estyseesghosts/silvestris/issues/13) (+ typography roles) |
| johjoh | Hand-built ROM list rows | [#32](https://github.com/estyseesghosts/silvestris/issues/32) |
| johjoh | `prompt_for_paths` native dialogs | [#29](https://github.com/estyseesghosts/silvestris/issues/29), [#63](https://github.com/estyseesghosts/silvestris/issues/63) |
| johjoh | `render_bottom` progress/status strip | [#75](https://github.com/estyseesghosts/silvestris/issues/75), [#8](https://github.com/estyseesghosts/silvestris/issues/8) |
| johjoh | Error advice text | [#23](https://github.com/estyseesghosts/silvestris/issues/23) |
| johjoh | OS file drops (`ExternalPaths`) | [#36](https://github.com/estyseesghosts/silvestris/issues/36) |

## Tracker changes on 2026-11-23 (initial rewrite)

- All issues rewritten to the anatomy above; bracketed title prefixes replaced by labels and milestones.
- New: [#72](https://github.com/estyseesghosts/silvestris/issues/72) (this roadmap), [#73](https://github.com/estyseesghosts/silvestris/issues/73) (triage the uncommitted tree), [#74](https://github.com/estyseesghosts/silvestris/issues/74) (upstream lotus's vendored patches, pinning contract), [#75](https://github.com/estyseesghosts/silvestris/issues/75) (bottom bar), [#76](https://github.com/estyseesghosts/silvestris/issues/76) (search menu / command palette), [#77](https://github.com/estyseesghosts/silvestris/issues/77) (find bar and search field).
- Re-scoped: [#13](https://github.com/estyseesghosts/silvestris/issues/13) now includes tab views and group boxes; [#15](https://github.com/estyseesghosts/silvestris/issues/15) adds the lock button; [#40](https://github.com/estyseesghosts/silvestris/issues/40) covers every scroll source; [#45](https://github.com/estyseesghosts/silvestris/issues/45) adds history graphs; [#47](https://github.com/estyseesghosts/silvestris/issues/47) drops Find/Replace (now [#77](https://github.com/estyseesghosts/silvestris/issues/77)) and adds the progress window; [#48](https://github.com/estyseesghosts/silvestris/issues/48) adds the icon API; [#52](https://github.com/estyseesghosts/silvestris/issues/52) becomes coverage matrix + gap register + guide (its reference-app part is [#56](https://github.com/estyseesghosts/silvestris/issues/56)).
- Open PR [#71](https://github.com/estyseesghosts/silvestris/issues/71) (consumer font-family overrides) is the extension-point pattern this roadmap wants: review it against the definition of done and [#70](https://github.com/estyseesghosts/silvestris/issues/70), then land it before consumers bundle fonts themselves.
- Closed: [#67](https://github.com/estyseesghosts/silvestris/issues/67) (composition patterns), folded into [#52](https://github.com/estyseesghosts/silvestris/issues/52) (guide) and [#61](https://github.com/estyseesghosts/silvestris/issues/61) (controllers).
- The original issue bodies are preserved in the first comment of each issue.

## Roadmap adjustment, 2026-11-23

- Sandcat is not a gating consumer. Its path dependency on a dirty checkout and its adoption are deferred until Silvestris's own foundations land; no provisional API is kept for it, and M0 does not wait on it.
- Rust formatting follows `style-guides/rust-style-guide.md`: a `rustfmt.toml` with `max_width = 100`, `hex_literal_case = "Lower"` and `format_macro_matchers = true`. The format-only commit is [#73](https://github.com/estyseesghosts/silvestris/issues/73)'s first step.
- The uncommitted `TextArea` prototype is discarded, not landed as-is. [#58](https://github.com/estyseesghosts/silvestris/issues/58) owns the multiline text API; [#73](https://github.com/estyseesghosts/silvestris/issues/73) attaches its duplication inventory as #58's checklist.
- [#74](https://github.com/estyseesghosts/silvestris/issues/74) is re-scoped to lotus's vendored patches plus the pin contract for lotus, chaus and johjoh. Sandcat re-pins later.
