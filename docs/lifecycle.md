# Application lifecycle and coordinated quit

Apps wired quit and window-close by hand: review dirty documents, wait for
sheets, flush state, stop workers, then exit. `lifecycle` owns that order and
leaves the work to the application.

Evidence: HIG p. 246 (dialogs for saving, closing and quitting), p. 177 (the
Application menu). Single-instance IPC and dock integration are #63; this
module receives a `reopen` callback and does not implement them.

## AppDelegate

```rust
impl AppDelegate for MyApp {
    fn did_launch(&mut self, cx: &mut App) { /* register commands, open window */ }
    fn should_open_untitled(&mut self, _: &App) -> bool { true }
    fn reopen(&mut self, cx: &mut App) { /* new window when none are open */ }
    fn should_terminate(&mut self, cx: &mut App) -> Terminate {
        match self.flush() {
            Flush::Done => Terminate::Now,
            Flush::Pending => Terminate::Later(cx.spawn(async move |this, cx| {
                this.update(cx, |app, cx| app.finish_shutdown(cx)).ok();
            })),
            Flush::Blocked => Terminate::Cancel,
        }
    }
}
```

Every hook has a default. `did_become_active`/`did_resign_active` are called by
`notify_activation`; the same signal is what panels (#27) and caret/animation
timers use to stop while the window is inactive. `should_open_untitled` and
`reopen` are for the app menu and OS reopen; the window policy stays the app's.

`Terminate::Later(Task<()>)` is the async shutdown hook: sandcat awaits
`StorageWorker::shutdown()` and saves its draft here; lotus flushes coalesced
divider saves and stops workers. The coordinator returns `Deferred`; the app
awaits `take_deferred()`, calls `deferred_finished`, and only then reviews
documents.

## QuitCoordinator

`request_quit(cx)`:

1. calls `should_terminate` — `Cancel` stops, `Now` continues, `Later` waits;
2. queues every dirty `document::DocumentController` document;
3. with no dirty documents returns `Terminate`; otherwise returns
   `Review { remaining }` and `needs_review_alert()` is true when more than one
   document is dirty.

The app presents the review prompt with `review_name(cx)` and reports
`ReviewChoice`:

- `Cancel` → `Cancelled`, the app stays open and documents keep their dirty
  state;
- `Don't Save` → the next document, or `Terminate`;
- `Save` → runs `Document::save`; `Saved` advances, `NeedsPanel` returns
  `Saving` for the #29 panel, `Failed` cancels the quit.

`resolve_save` completes a panel. `progress()` reports total and remaining for
a progress indicator.

No wall-clock sleep: the application owns the timeout and calls
`timeout_elapsed()` from its own timer, which forces `Terminate` so a stuck
worker cannot trap the process. The app decides the timeout duration.

## Traps

- Workers are not documents. A cancel for one window's document must not skip
  the app's own shutdown work; `Later` runs before the document review, so it
  cannot be skipped by a review cancel.
- A quit that exits before an async storage future resolves loses data. The
  `Later` contract is "await me"; do not call `Terminate` until
  `deferred_finished` returns it.
- Closing a window during a save uses `DocumentController`'s saving phase, not
  this coordinator; a second close does not prompt.

## Tests

Two dirty documents: review order, review name, Don't Save, then Cancel keeps
both dirty. Saving with a panel waits and then terminates. Clean documents
terminate; a failed save cancels. `Later` returns Deferred, then terminates;
a timeout forces exit. A delegate Cancel aborts. Activation and reopen hooks
fire once each.

No gallery or golden: lifecycle has no visual surface. The dirty close button
is covered by #37's `window-frame-edited` goldens.
