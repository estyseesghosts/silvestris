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

Every hook has a default. `should_open_untitled` and `reopen` are for the app
menu and OS reopen; the window policy stays the app's.

## Installing the delegate

```rust
let app = Rc::new(RefCell::new(MyApp::default()));
application.on_reopen(lifecycle::reopen);
application.run(move |cx| {
    silvestris::init(ThemePreference::default(), cx)?;
    lifecycle::install(app.clone(), cx); // calls did_launch
    let quit = QuitCoordinator::new(Box::new(app.clone()));
    // ...
});
```

`install` stores the delegate and calls `did_launch`. Desktop GPUI has no
application activation event, so the app counts as active while any window
opened through `chrome::open_window` (every role window) is active. Each of
those windows reports its activation; after `ACTIVATION_GRACE` (50 ms, an
estimate in `docs/exceptions.md`) the delegate gets `did_become_active` or
`did_resign_active` once per app-level change. Moving focus between two of the
app's windows is not a resign. `is_app_active(cx)` reads the same state. A
window opened another way can report through `window_activation_changed`.

`reopen(cx)` forwards an OS reopen (dock click, second launch) to the installed
delegate; pass it to GPUI's `Application::on_reopen` before `run`.

`Rc<RefCell<D>>` implements `AppDelegate`, so the same delegate drives
`install` and `QuitCoordinator`. Caret blink does not need the app-level
signal: text clients stop on their own window's deactivation (see `text.md`).

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
fire once each. `install` calls `did_launch` once; with two windows opened
through `open_window`, activating the first makes the app active, switching to
the second is not a resign, and the last deactivation resigns once; `reopen`
reaches the delegate; the same `Rc<RefCell<_>>` delegate drives a
`QuitCoordinator`.

No gallery or golden: lifecycle has no visual surface. The dirty close button
is covered by #37's `window-frame-edited` goldens.
