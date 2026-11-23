# Consumer integration contract

Silvestris is consumed as source. Consumers pin an exact revision, share its
GPUI revision and toolchain, and report gaps upstream instead of patching a
private copy.

## Supported dependency forms

Use a git revision with a committed lockfile:

```toml
silvestris = { git = "https://github.com/estyseesghosts/silvestris", rev = "<full commit>" }
```

Commit `Cargo.lock` and `rust-toolchain.toml` with the change. A consumer
build must resolve one GPUI: the revision in `config/pins.toml`, currently
`6b720973027999bae402df23d469026f4cced988`, crate version 0.2.2. Mixing two
GPUI revisions produces incompatible types even when both compile.

A path dependency is allowed only during local development against a clean,
committed Silvestris tree. Do not commit a path to a dirty checkout, a vendor
copy or a fork. Sandcat is the recorded exception, below.

Vendoring a private copy is not a supported form. Every local patch becomes
invisible to the other consumers and is lost on the next refresh. Upstream the
generic mechanism, then move the pin.

## Current consumer pins

Pins verified on 2026-11-23. Full commit ids, not abbreviations, belong in
`Cargo.toml` and `config/pins.toml`.

| Consumer | Pin | Form | Local patches |
| --- | --- | --- | --- |
| lotus | `d1db8f0c6cbe80e80fb1dfebbb5c78990d2b9265` | vendored `vendor/silvestris` (tree `a1b78ec`) | Inspector scroll, inspector focus deferral, printable keys, Settings bounds |
| chaus | `a62b3b2553cc4af4c109b092d234522346006d30` | git revision | none |
| johjoh | `d1db8f0c6cbe80e80fb1dfebbb5c78990d2b9265` | git revision | none |
| sandcat | uncommitted checkout | path dependency | deferred |

Lotus's three generic patches are now upstream in `4df116b` (PR #81). Re-pin
lotus to that revision or later, regenerate `vendor/silvestris` from it and
delete the local patch list. The Settings bounds restore stays in lotus: it
decides which bounds to persist for which window role.

Sandcat builds against a path dependency into an uncommitted checkout. Its
adoption is deferred until the M1 foundations land. It is not a gating
consumer, no provisional API exists for it, and this exception is recorded so
the pin table does not imply support.

## Gap register

Every consumer-side bespoke UI element found in the 2026-11-23 survey, with the
issue that replaces it. Adoption happens in the consumer repository after the
Silvestris issue lands. Sandcat rows are inventory only while its adoption is
deferred.

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
| lotus | Vendored Silvestris patches ([lotus#141](https://github.com/estyseesghosts/lotus/issues/141) inspector, [lotus#74](https://github.com/estyseesghosts/lotus/issues/74) focus deferral, [lotus#58](https://github.com/estyseesghosts/lotus/issues/58) Settings bounds) | [#74](https://github.com/estyseesghosts/silvestris/issues/74) (upstreamed; lotus re-pins) |
| lotus | PICA texture/preview rendering, debugger domain panels | app-owned (domain views composed from Silvestris tables/splits) |
| lotus | Settings window bounds choice per role | app-owned (which bounds persist is application policy) |
| sandcat | `apps/desktop/src/scrollbar.rs` Aqua scroller over `ListState` | [#40](https://github.com/estyseesghosts/silvestris/issues/40) |
| sandcat | Composer on the discarded `TextArea` prototype | [#58](https://github.com/estyseesghosts/silvestris/issues/58) (new multiline API); sandcat re-adopts after the foundations land |
| sandcat | `body_segments` / code-block rendering in transcript | [#38](https://github.com/estyseesghosts/silvestris/issues/38) |
| sandcat | `status_label`/`status_color` session badges, `project_tone` | [#4](https://github.com/estyseesghosts/silvestris/issues/4), [#48](https://github.com/estyseesghosts/silvestris/issues/48) |
| sandcat | First-run / server pairing setup state | [#30](https://github.com/estyseesghosts/silvestris/issues/30) |
| sandcat | Permission prompts and forms | [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#23](https://github.com/estyseesghosts/silvestris/issues/23), [#54](https://github.com/estyseesghosts/silvestris/issues/54) |
| sandcat | Theme/model/agent pickers in toolbar | [#2](https://github.com/estyseesghosts/silvestris/issues/2), [#76](https://github.com/estyseesghosts/silvestris/issues/76) |
| sandcat | `!SilvestrisTextField` key-context exclusions | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| sandcat | Shutdown draft flush | [#59](https://github.com/estyseesghosts/silvestris/issues/59) |
| sandcat | Composer submission, Ctrl+1–4 policy, transcript following | app-owned (session behaviour on top of [#38](https://github.com/estyseesghosts/silvestris/issues/38), [#57](https://github.com/estyseesghosts/silvestris/issues/57)) |
| chaus | Find bar (`editor_find.rs`) | [#77](https://github.com/estyseesghosts/silvestris/issues/77) |
| chaus | Close / conflict / recovery prompts (`editor_close.rs`, `editor_conflict.rs`, `editor_recovery.rs`) | [#37](https://github.com/estyseesghosts/silvestris/issues/37), [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#23](https://github.com/estyseesghosts/silvestris/issues/23) (policies stay app-owned) |
| chaus | Menus assembled by hand (`tests/menus.rs`) | [#24](https://github.com/estyseesghosts/silvestris/issues/24) |
| chaus | Startup-only shortcut bindings | [#66](https://github.com/estyseesghosts/silvestris/issues/66) |
| chaus | Open/Save | [#29](https://github.com/estyseesghosts/silvestris/issues/29) |
| chaus | Locally registered Geist Sans house font (`chaus-app/assets/fonts`) | [#71](https://github.com/estyseesghosts/silvestris/issues/71) (font overrides), then [#70](https://github.com/estyseesghosts/silvestris/issues/70) |
| chaus | Ropey editor, Vim engine, highlighting | app-owned by design (may reuse [#58](https://github.com/estyseesghosts/silvestris/issues/58) IME/selection traits) |
| johjoh | `heading`/`caption`/`group` helpers | [#13](https://github.com/estyseesghosts/silvestris/issues/13) (+ typography roles) |
| johjoh | Hand-built ROM list rows | [#32](https://github.com/estyseesghosts/silvestris/issues/32) |
| johjoh | `prompt_for_paths` native dialogs | [#29](https://github.com/estyseesghosts/silvestris/issues/29), [#63](https://github.com/estyseesghosts/silvestris/issues/63) |
| johjoh | `render_bottom` progress/status strip | [#75](https://github.com/estyseesghosts/silvestris/issues/75), [#8](https://github.com/estyseesghosts/silvestris/issues/8) |
| johjoh | Error advice text | [#23](https://github.com/estyseesghosts/silvestris/issues/23) |
| johjoh | OS file drops (`ExternalPaths`) | [#36](https://github.com/estyseesghosts/silvestris/issues/36) |

A new bespoke element goes in this table with an issue number or an app-owned
reason. "App-owned" means the element encodes policy the framework cannot
generalise; it does not excuse drawing a generic control by hand. The table is
the single copy: `ROADMAP.md` links here.

## Upstreamed lotus deltas

Each entry names the generic mechanism, its regression test and what stays in
lotus.

`TabView::scroll_handle` (`crates/silvestris/src/preferences.rs`) accepts a
caller-owned scroll position so an inspector pane can reveal content after a
layout change instead of depending on a keyed internal handle. Lotus keeps the
policy of which pane to reveal and when. Covered indirectly by the preference
pane tests; a dedicated scroll test is part of #18's render path.

Shell inspector focus now transfers in `window.defer`, after the frame is
built, on open and on close. A focus change made by another view in the same
frame (for example a toolbar reporting to AccessKit) wins, because the deferred
closure checks the focused handle before moving it. Tests:
`shell::tests::inspector_focus_moves_after_the_frame_and_returns_on_close` and
`shell::tests::deferred_inspector_focus_does_not_override_a_toolbar`.

Shell's inspector key handler ignores printable keys while a
`SilvestrisTextField` is in the focus context, so GPUI's input handler receives
the character. Navigation, Escape and consumer shortcuts still run. Test:
`shell::tests::printable_keys_reach_a_focused_text_field_through_the_shell`.
Lotus's guest-input isolation rules and emulator focus policy remain in lotus.

## Reporting a gap

File an issue in `estyseesghosts/silvestris` with the standard anatomy: Why,
References, Deliverables, Avoid, Regression traps, Done when. Add the milestone,
layer label, priority and consumer label. Include a failing scenario using the
interaction harness where the behaviour is testable.

Do not add a consumer-specific type, policy or name to a Silvestris crate. Do
not keep a second implementation of a control that Silvestris already ships.
If the gap is app-owned domain behaviour, write it down as such in the gap
register.

## Moving a pin

1. Choose the Silvestris commit and read the matching `CHANGELOG.md` range.
2. Update `Cargo.toml` and `Cargo.lock` together; keep `config/pins.toml`
   aligned.
3. Run `tools/consumers/check.ps1` with the candidate checkout present.
4. Record any behavioural change to focus, keys, windows or persistence in the
   consumer's own release notes.

Changing the GPUI revision or the Rust toolchain is a breaking event. It needs
a note here, a passing consumer check and the same revision in every consumer.

## Build check

`tools/consumers/check.ps1` is opt-in and CI-able:

```powershell
pwsh tools/consumers/check.ps1 -Candidate . -Consumer lotus,chaus,johjoh
```

It skips a consumer whose checkout is absent, always skips sandcat, and fails
on a build error. Lotus is checked from a temporary worktree whose
`vendor/silvestris` is replaced by the candidate tree, so the build proves
lotus compiles against the candidate; it does not prove the old vendor copy was
byte-identical to a commit (compare trees for that). Git-pinned consumers are
checked from a temporary worktree with a Cargo `[patch]` entry pointing at the
candidate, so the consumer's own working tree and lockfile are not modified.
