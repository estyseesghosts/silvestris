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

Lotus's three generic patches are now upstream. Re-pin lotus to the commit that
contains `docs/consumers.md`, regenerate `vendor/silvestris` from that commit
and delete the local patch list. The Settings bounds restore stays in lotus: it
decides which bounds to persist for which window role.

Sandcat builds against a path dependency into an uncommitted checkout. Its
adoption is deferred until the M1 foundations land. It is not a gating
consumer, no provisional API exists for it, and this exception is recorded so
the pin table does not imply support.

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
`vendor/silvestris` is replaced by the candidate tree, which proves the vendor
copy is byte-equivalent to an upstream commit. Git-pinned consumers are checked
from a temporary worktree with a Cargo `[patch]` entry pointing at the
candidate, so the consumer's own working tree and lockfile are not modified.
