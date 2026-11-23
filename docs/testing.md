# Interaction scenarios

`silvestris::test_support::scenario::Scenario` drives a GPUI test app through
focus, typing, key chords, clicks and window lifecycle. It uses the same focus
handles, key bindings, actions and click handlers as the application.

Enable the `test-support` feature in a consumer. Unit tests inside this crate
compile the harness automatically.

## Use

Open a window with a root view that stores its focus handle, then drive it. The
example assumes a fixture that returns the window handle and the view's
`FocusHandle`:

```rust
let mut scenario = Scenario::new();
let (window, focus) = open_my_window(&mut scenario);

scenario.focus(&focus);
scenario.assert_focused(&focus);
scenario.click("save-button");
scenario.press("ctrl-s");
scenario.advance(Duration::from_millis(16));
assert!(scenario.close());
```

`open` makes the new window active. `activate` switches between windows.
`windows()` reports them in open order. `close` closes the active window and
activates the most recent remaining one.

## Rules

- Use stable `debug_selector` names. Never derive them from row or element
  indices; scenarios rot when the layout changes.
- Never sleep on wall-clock time. Use `scenario.advance` so GPUI's test clock
  runs timers deterministically.
- Assert observable effects: focused handle, window list, rendered bounds,
  emitted event or command result. Do not assert that a mock was called.
- A view that should close must install a close handler, for example
  `window.on_window_should_close(cx, |_, _| true)`. `close` returns false when
  the view vetoes it.

## What it proves

GPUI dispatch inside the mock platform: focus routing, key bindings, action
handlers, click handlers, timer advancement, redraw and window close.

## What it does not prove

Native input delivery, OS window-manager behaviour, DWM or IME integration, or
font rasterisation. Those belong to the platform matrix (#50) and the visual
gate (#18). Headless dispatch is not a substitute for either.

## Current coverage

The harness has two tests: focus/click/press/advance/close, and opening,
activating and closing two windows in order. Fixtures for the text field, table,
sheet and utility panel, menu helpers, command assertions and record/replay
remain on #68.
