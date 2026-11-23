# Modal sessions

A modal session is the policy half of a sheet or an app-modal dialog. It does
not draw: #22 owns the sheet layout and #23 the alert layout. It decides which
commands the covered scope may run, where Tab moves, what queues and what
stacks, and how the result is delivered.

Evidence: HIG p. 235 (types of dialogs and their scope), p. 236 (sheets are
document-modal), p. 241 (dismissing dialogs; Escape cancels). The scope table is
the HIG one: a document-modal sheet makes the covered document modal and leaves
sibling documents usable.

## Scope

| Scope | Covers | Behaviour |
| --- | --- | --- |
| `ModalScope::Document` | The window the session was presented in | Commands from that window's responder chain resolve `Disabled` unless allowed; sibling windows are untouched |
| `ModalScope::Application` | Every window | Every chain resolves `Disabled` unless allowed; stacks on top of an open sheet |

`ResponderChain::dispatch` and `present` call `modal::command_allowed`, so a
shortcut, a toolbar `CommandButton`, a menu row and a direct call agree. Without
an installed session nothing is blocked.

The allow-list defaults to the editing commands a sheet's text field needs:
`edit.copy`, `edit.cut`, `edit.paste`, `edit.select-all`, `edit.undo`,
`edit.redo`. `allow` adds one; `allow_only` replaces the set. Commands handled
directly by the modal content's own elements (buttons, fields) are not affected
— the block is for invocations routed through the chain.

## Lifecycle

```rust
let focus = ConfirmationFocus::new(cx);
let session = cx.new(|_| {
    ModalSession::new(ModalScope::Document)
        .with_focus_stops(focus.handles().to_vec())
        .on_end(|result| { /* Accepted, Rejected or Cancelled */ })
});
modal::present(session.clone(), window, cx);
```

- `present` records the window as the owner. A document session presented while
  another document session is active on the same window is queued; it becomes
  active when the current one finishes.
- An application session is never queued; it becomes active above the sheet.
- `active(window, cx)` returns the session that currently owns the window;
  `is_modal` says whether any does.
- `finish(session, result, window, cx)` marks the session ended, removes it
  from the registry, restores the focus recorded at activation, advances the
  queue and then calls the one-shot `on_end` callback with `Accepted`,
  `Rejected` or `Cancelled`. A session that already ended is ignored.
- `escape_session(session, window, cx)` rejects an active session and returns
  `false` for a queued or ended one, so the caller can pass Escape outward.
- `cycle_focus(session, forward, window, cx)` is the Tab trap: it moves within
  the declared stops and wraps at both ends. With no stops, focus leaves the
  content (it blurs) rather than escaping into the covered window.
- `cancel_owned(owner, cx)` cancels every active and queued session owned by a
  window. Call it from `on_window_should_close` before the window is removed:
  GPUI keeps one close callback per window and the application owns the close
  decision (#20), so Silvestris cannot install it here.

Result delivery is a callback with a typed enum rather than a future because
the callback also has to run on window teardown, when polling a future would
be too late. It runs after the session is out of the registry; it must not
borrow the session.

## Escape order

The order is fixed by #60 and this module: the innermost transient surface,
then the modal sheet, then the application. `TransientLayer::escape` returns
false when no transient is open; the sheet's key handler then calls
`escape_session`; if that returns false (no active sheet) the key goes to the
app. A transient opened from inside a sheet (a combo list) is still the first
thing Escape closes.

## Gallery

The gallery's transient row has "Open modal sheet". It presents a
`ConfirmationSheet` inside a document session with the sheet's Cancel and Apply
handles as the focus stops, so Save is blocked in the gallery window while the
sheet is up, Tab cycles between the buttons, Escape rejects, and the session
clears when it ends. That reuses the existing sheet; it is a policy demo, not
the #22 redesign.

## Tests

`modal.rs` covers: a document session blocks Save in its window but not in a
sibling, while Copy still runs; an application session blocks both windows and
stacks over a sheet; a second document session queues and advances on finish;
Tab and Shift-Tab wrap inside the stops; owner close cancels active and queued
sessions and delivers `Cancelled`; Escape rejects only the active session;
presentation reads disabled while blocked.
