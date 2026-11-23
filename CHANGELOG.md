# Changelog

Silvestris has no released crate version. Consumers pin commits, so entries are
keyed by commit range. A range lists public API changes a consumer must handle
when moving its pin. Behaviour-only changes are listed because consumers can
depend on focus, key routing and layout.

## After 216de9b

PR #20, window roles. Additive:

- `window_role::WindowRole` (`Document`, `Utility`, `Panel`, `Preferences`,
  `Dialog`, `Assistant`, `About`) with `spec`, `titlebar_height`, `autosave_key`
  and `ALL`; `RoleSpec`.
- `window_role::role_options`, `centered_on`, `cascade_from`,
  `restore_bounds`, `save_frame`, `open_role`, `open_owned_role`;
  `OwnedWindow`, `FrameStore` and `OwnedWindows`.
- `WindowFrame::role`, which makes the drawn title bar agree with the role.

Behaviour: a role decides resizability, live title-bar buttons, title
visibility, parent centring, the frame-autosave key and, for utility windows, a
16 px title band. Fixed roles draw only the close button. `OwnedWindows` closes
its children when the application's close policy calls `close_all`; Silvestris
does not install an `on_window_should_close` handler of its own.

PR #57, responder chain. Additive:
 Additive:

- `responder::Responder` with `handles`, `state` and `perform`, plus
  `ResponderState` (`enabled`, `checked`, `title`).
- `responder::ResponderChain` with `push`, `replace`, `resolve`, `validate`,
  `dispatch`, `present`, `begin_transient` and `end_transient`.
- `responder::ResponderEntry` with `new`, `custom`, `active_when`,
  `when_focused` and `capture_keys`; `ResponderScope`.
- `responder::FocusReturn` for transient surfaces and sheets.
- `CommandButton::chain`, `CommandScope::chain`, `PopupMenu::set_chain`.

Behaviour: a resolved responder's `state` decides enabled and label for menu,
toolbar and shortcut presentation; a claiming but disabled layer answers
`Disabled` without falling through; a dropped responder entity passes outward.

## After 3ac1105

Pending #74 integration commit:

- Added `preferences::TabView::scroll_handle`, which accepts a caller-owned
  `gpui::ScrollHandle` instead of always using a keyed internal handle.
  Additive.
- Changed `Shell` inspector focus to transfer in `window.defer` after the
  frame, on open and close. A focus change made by another view in the same
  frame wins. Consumers that relied on focus being set synchronously inside
  `Shell::render` should read `Window::focused` after the frame.
- Changed `Shell`'s inspector key handler to ignore printable keys while a
  `SilvestrisTextField` is in the focus context. Consumer key callbacks no
  longer see those keys when a text field has focus.
- Added regression tests in `crates/silvestris/src/shell.rs`. No removals.

## 06a88af..3ac1105

PR #71, “Allow consumers to override theme font families”. Additive:

- `silvestris_core::FontOverrides` with `new`, `ui`, `mono`, `is_empty`.
- `Theme::apply_font_overrides` and
  `silvestris_core::theme_with_font_overrides`.
- `silvestris::set_global_font_overrides`, `set_font_overrides`,
  `clear_font_overrides` and `font_overrides`.

A per-theme override wins over the global one. Sizes, colours and metrics are
unchanged.

## bf52d64..06a88af

PR #78, roadmap and issue template. Documentation only; no code change.

## d1db8f0..bf52d64

PRs #79 and #80.

- PR #79 reformatted the workspace with `rustfmt.toml` (`max_width = 100`, LF
  line endings). No API change.
- PR #80 added `silvestris::test_support` with `scenario`, `fixtures` and
  `script`. `test_support` is now available under `cfg(test)` as well as the
  `test-support` feature; the existing `test_support::initialize` helper moved
  into the module. Additive.

## Before d1db8f0

No stable public API. The source is a work-in-progress shell for Lotus issues
#118–#138. Treat every type as movable and pin an exact commit.
