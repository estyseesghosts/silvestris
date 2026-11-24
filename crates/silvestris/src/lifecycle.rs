//! Application lifecycle and coordinated quit (#59).
//!
//! The application supplies an [`AppDelegate`] for launch, reopen, activation
//! and the final termination decision. [`QuitCoordinator`] turns that into the
//! 10.6 order: ask the delegate, review each dirty document (#37), let a
//! deferred termination finish, then quit.
//!
//! The delegate's `Later` future is the hook for work that must finish before
//! exit: sandcat awaits `StorageWorker::shutdown()` and saves its draft; lotus
//! flushes coalesced divider saves and stops workers. The coordinator never
//! waits on wall-clock time itself; the application drives a timeout and calls
//! [`QuitCoordinator::timeout_elapsed`] so a stuck worker cannot trap the app.
//!
//! Single-instance IPC and dock integration belong to #63 host services; this
//! module only receives the `reopen` callback.

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::rc::Rc;

use gpui::{App, Global, SharedString, Task, WindowId};

use crate::document::{DocumentHandle, SaveResult};

/// The application delegate. Every hook has a default so an app implements
/// only what it needs.
pub trait AppDelegate: 'static {
    fn did_launch(&mut self, _cx: &mut App) {}
    /// Whether reopening with no windows should open an untitled document.
    fn should_open_untitled(&mut self, _cx: &App) -> bool {
        true
    }
    /// Dock click, second instance or "reopen" from the OS.
    fn reopen(&mut self, _cx: &mut App) {}
    fn did_become_active(&mut self, _cx: &mut App) {}
    fn did_resign_active(&mut self, _cx: &mut App) {}
    /// The final termination decision. `Later` means async shutdown work is in
    /// flight; `Cancel` aborts the quit.
    fn should_terminate(&mut self, _cx: &mut App) -> Terminate {
        Terminate::Now
    }
}

/// A delegate shared between [`install`] and a [`QuitCoordinator`]: pass
/// `Box::new(shared.clone())` to `QuitCoordinator::new`.
pub type SharedDelegate<D> = Rc<RefCell<D>>;

impl<D: AppDelegate> AppDelegate for Rc<RefCell<D>> {
    fn did_launch(&mut self, cx: &mut App) {
        self.borrow_mut().did_launch(cx);
    }
    fn should_open_untitled(&mut self, cx: &App) -> bool {
        self.borrow_mut().should_open_untitled(cx)
    }
    fn reopen(&mut self, cx: &mut App) {
        self.borrow_mut().reopen(cx);
    }
    fn did_become_active(&mut self, cx: &mut App) {
        self.borrow_mut().did_become_active(cx);
    }
    fn did_resign_active(&mut self, cx: &mut App) {
        self.borrow_mut().did_resign_active(cx);
    }
    fn should_terminate(&mut self, cx: &mut App) -> Terminate {
        self.borrow_mut().should_terminate(cx)
    }
}

/// The installed delegate and the app-level activation derived from window
/// activation. Desktop GPUI has no application activation event, so the app is
/// active while any of its Silvestris windows is.
struct Installed {
    delegate: Rc<RefCell<dyn AppDelegate>>,
    active_windows: BTreeSet<WindowId>,
    app_active: bool,
    pending: bool,
}

impl Global for Installed {}

/// How long a window-activation change waits before it counts as an app-level
/// change. Estimate: long enough to cover the deactivate/activate message pair
/// of an in-app window switch, short of anything a person sees; see
/// `docs/exceptions.md`.
pub const ACTIVATION_GRACE: std::time::Duration = std::time::Duration::from_millis(50);

/// Install the application delegate. Calls `did_launch` now; from then on
/// every window opened through `chrome::open_window` (so every role window)
/// reports activation, and the delegate sees `did_become_active` /
/// `did_resign_active` once per app-level change. Wire OS reopen with
/// `application.on_reopen(lifecycle::reopen)` before `run`.
pub fn install<D: AppDelegate>(delegate: SharedDelegate<D>, cx: &mut App) {
    let delegate: Rc<RefCell<dyn AppDelegate>> = delegate;
    cx.set_global(Installed {
        delegate: delegate.clone(),
        active_windows: BTreeSet::new(),
        app_active: false,
        pending: false,
    });
    cx.on_window_closed(|cx, id| window_activation_changed(id, false, cx))
        .detach();
    delegate.borrow_mut().did_launch(cx);
}

/// Forward an OS reopen (dock click, second launch) to the installed delegate.
pub fn reopen(cx: &mut App) {
    let Some(delegate) = cx
        .try_global::<Installed>()
        .map(|installed| installed.delegate.clone())
    else {
        return;
    };
    delegate.borrow_mut().reopen(cx);
}

/// Record one window's activation. `chrome::open_window` calls this; an app
/// that opens windows another way can call it from its own observer.
///
/// The app-level answer is settled after [`ACTIVATION_GRACE`]: switching
/// between two of the app's windows deactivates one before activating the
/// other, in separate platform messages, and that is not the app resigning.
pub fn window_activation_changed(window: WindowId, active: bool, cx: &mut App) {
    let Some(installed) = cx.try_global::<Installed>() else {
        return;
    };
    let mut windows = installed.active_windows.clone();
    if active {
        windows.insert(window);
    } else {
        windows.remove(&window);
    }
    let schedule = !installed.pending;
    {
        let installed = cx.global_mut::<Installed>();
        installed.active_windows = windows;
        installed.pending = true;
    }
    if schedule {
        cx.spawn(async move |cx| {
            cx.background_executor().timer(ACTIVATION_GRACE).await;
            cx.update(settle_activation);
        })
        .detach();
    }
}

fn settle_activation(cx: &mut App) {
    let Some(installed) = cx.try_global::<Installed>() else {
        return;
    };
    let now = !installed.active_windows.is_empty();
    let changed = now != installed.app_active;
    let delegate = installed.delegate.clone();
    {
        let installed = cx.global_mut::<Installed>();
        installed.pending = false;
        installed.app_active = now;
    }
    if changed {
        notify_activation(&mut *delegate.borrow_mut(), now, cx);
    }
}

/// Whether any of the app's windows is active, per [`install`]'s tracking.
#[must_use]
pub fn is_app_active(cx: &App) -> bool {
    cx.try_global::<Installed>()
        .is_some_and(|installed| installed.app_active)
}

/// What the delegate says about termination.
pub enum Terminate {
    Now,
    /// Work that must finish before the process exits.
    Later(Task<()>),
    Cancel,
}

/// What the user chose while reviewing a dirty document during quit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewChoice {
    Save,
    DontSave,
    Cancel,
}

/// The next step for the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuitStep {
    /// Call `cx.quit()`.
    Terminate,
    /// Present the review prompt for `remaining` dirty documents. Cancel aborts
    /// the quit; otherwise call `resolve_review`.
    Review { remaining: usize },
    /// A save is running or needs a Save panel (#29). Call `resolve_save`.
    Saving,
    /// The delegate's `Later` future is running. Await `take_deferred`, then
    /// call `deferred_finished`.
    Deferred,
    /// Keep the application open.
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    Deferred,
    Review,
    Saving,
}

/// Progress for a quit sheet or a menu item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuitProgress {
    pub dirty_total: usize,
    pub remaining: usize,
}

/// Coordinates one quit attempt.
pub struct QuitCoordinator {
    delegate: Box<dyn AppDelegate>,
    documents: Vec<Box<dyn DocumentHandle>>,
    review: VecDeque<usize>,
    dirty_total: usize,
    phase: Phase,
    deferred: Option<Task<()>>,
}

impl QuitCoordinator {
    #[must_use]
    pub fn new(delegate: Box<dyn AppDelegate>) -> Self {
        Self {
            delegate,
            documents: Vec::new(),
            review: VecDeque::new(),
            dirty_total: 0,
            phase: Phase::Idle,
            deferred: None,
        }
    }

    pub fn add_document(&mut self, document: Box<dyn DocumentHandle>) {
        self.documents.push(document);
    }

    #[must_use]
    pub fn delegate(&mut self) -> &mut dyn AppDelegate {
        &mut *self.delegate
    }

    #[must_use]
    pub fn progress(&self) -> QuitProgress {
        QuitProgress {
            dirty_total: self.dirty_total,
            remaining: self.review.len(),
        }
    }

    /// More than one dirty document: show the "Review changes…" alert before
    /// the per-document prompt.
    #[must_use]
    pub fn needs_review_alert(&self) -> bool {
        self.dirty_total > 1
    }

    /// Start a quit. Calls `should_terminate`, then queues dirty documents.
    pub fn request_quit(&mut self, cx: &mut App) -> QuitStep {
        if self.phase != Phase::Idle {
            return match self.phase {
                Phase::Deferred => QuitStep::Deferred,
                Phase::Saving => QuitStep::Saving,
                Phase::Review => QuitStep::Review {
                    remaining: self.review.len(),
                },
                Phase::Idle => unreachable!(),
            };
        }
        match self.delegate.should_terminate(cx) {
            Terminate::Cancel => QuitStep::Cancelled,
            Terminate::Now => self.begin_review(cx),
            Terminate::Later(task) => {
                self.deferred = Some(task);
                self.phase = Phase::Deferred;
                QuitStep::Deferred
            }
        }
    }

    /// The delegate's deferred work finished; continue with the review.
    pub fn deferred_finished(&mut self, cx: &mut App) -> QuitStep {
        if self.phase != Phase::Deferred {
            return self.current_step();
        }
        self.deferred = None;
        self.phase = Phase::Idle;
        self.begin_review(cx)
    }

    /// Hand the future to the application to await.
    pub fn take_deferred(&mut self) -> Option<Task<()>> {
        self.deferred.take()
    }

    fn begin_review(&mut self, cx: &App) -> QuitStep {
        self.review.clear();
        for (index, document) in self.documents.iter().enumerate() {
            if document.is_dirty(cx) {
                self.review.push_back(index);
            }
        }
        self.dirty_total = self.review.len();
        if self.review.is_empty() {
            self.phase = Phase::Idle;
            return QuitStep::Terminate;
        }
        self.phase = Phase::Review;
        QuitStep::Review {
            remaining: self.review.len(),
        }
    }

    /// The document the next review prompt is about.
    #[must_use]
    pub fn review_name(&self, cx: &App) -> Option<SharedString> {
        self.review
            .front()
            .map(|index| self.documents[*index].display_name(cx))
    }

    /// Resolve the current review prompt.
    pub fn resolve_review(&mut self, choice: ReviewChoice, cx: &mut App) -> QuitStep {
        if self.phase != Phase::Review {
            return self.current_step();
        }
        match choice {
            ReviewChoice::Cancel => {
                self.review.clear();
                self.phase = Phase::Idle;
                QuitStep::Cancelled
            }
            ReviewChoice::DontSave => {
                self.review.pop_front();
                self.advance(cx)
            }
            ReviewChoice::Save => {
                self.phase = Phase::Saving;
                let Some(index) = self.review.front().copied() else {
                    self.phase = Phase::Idle;
                    return QuitStep::Terminate;
                };
                match self.documents[index].save(cx) {
                    SaveResult::Saved => {
                        self.review.pop_front();
                        self.phase = Phase::Review;
                        self.advance(cx)
                    }
                    SaveResult::NeedsPanel => QuitStep::Saving,
                    SaveResult::Failed(_) => {
                        self.phase = Phase::Idle;
                        self.review.clear();
                        QuitStep::Cancelled
                    }
                }
            }
        }
    }

    /// Complete a Save panel (#29) for the current document.
    pub fn resolve_save(&mut self, result: SaveResult, cx: &mut App) -> QuitStep {
        if self.phase != Phase::Saving {
            return self.current_step();
        }
        match result {
            SaveResult::Saved => {
                self.review.pop_front();
                self.phase = Phase::Review;
                self.advance(cx)
            }
            SaveResult::NeedsPanel => QuitStep::Saving,
            SaveResult::Failed(_) => {
                self.phase = Phase::Idle;
                self.review.clear();
                QuitStep::Cancelled
            }
        }
    }

    fn advance(&mut self, _cx: &App) -> QuitStep {
        if self.review.is_empty() {
            self.phase = Phase::Idle;
            QuitStep::Terminate
        } else {
            self.phase = Phase::Review;
            QuitStep::Review {
                remaining: self.review.len(),
            }
        }
    }

    /// The application's timeout expired. The delegate has already had its
    /// chance; force termination instead of trapping the process.
    pub fn timeout_elapsed(&mut self) -> QuitStep {
        self.phase = Phase::Idle;
        self.review.clear();
        self.deferred = None;
        QuitStep::Terminate
    }

    fn current_step(&self) -> QuitStep {
        match self.phase {
            Phase::Idle => QuitStep::Cancelled,
            Phase::Deferred => QuitStep::Deferred,
            Phase::Review => QuitStep::Review {
                remaining: self.review.len(),
            },
            Phase::Saving => QuitStep::Saving,
        }
    }
}

/// Call the delegate's activation hooks. The app wires this from
/// `observe_window_activation`; the same signal stops panel visibility (#27)
/// and caret/animation timers.
pub fn notify_activation(delegate: &mut dyn AppDelegate, active: bool, cx: &mut App) {
    if active {
        delegate.did_become_active(cx);
    } else {
        delegate.did_resign_active(cx);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{AppContext, Context, Entity, TestAppContext};

    use super::*;
    use crate::document::Document;

    struct Demo {
        name: SharedString,
        dirty: bool,
        save: SaveResult,
    }

    impl Document for Demo {
        fn display_name(&self) -> SharedString {
            self.name.clone()
        }
        fn is_dirty(&self) -> bool {
            self.dirty
        }
        fn save(&mut self, _: &mut Context<Self>) -> SaveResult {
            match self.save.clone() {
                SaveResult::Saved => {
                    self.dirty = false;
                    SaveResult::Saved
                }
                other => other,
            }
        }
        fn revert(&mut self, _: &mut Context<Self>) {
            self.dirty = false;
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Now,
        Cancel,
        Later,
    }

    struct Delegate {
        mode: Rc<Cell<Mode>>,
        active: Rc<Cell<usize>>,
        resigned: Rc<Cell<usize>>,
        reopened: Rc<Cell<usize>>,
        launched: Rc<Cell<usize>>,
    }

    impl AppDelegate for Delegate {
        fn did_launch(&mut self, _: &mut App) {
            self.launched.set(self.launched.get() + 1);
        }
        fn reopen(&mut self, _: &mut App) {
            self.reopened.set(self.reopened.get() + 1);
        }
        fn did_become_active(&mut self, _: &mut App) {
            self.active.set(self.active.get() + 1);
        }
        fn did_resign_active(&mut self, _: &mut App) {
            self.resigned.set(self.resigned.get() + 1);
        }
        fn should_terminate(&mut self, cx: &mut App) -> Terminate {
            match self.mode.get() {
                Mode::Now => Terminate::Now,
                Mode::Cancel => Terminate::Cancel,
                Mode::Later => {
                    let task = cx.spawn(async |_| {});
                    Terminate::Later(task)
                }
            }
        }
    }

    fn document(
        app: &mut TestAppContext,
        name: &str,
        dirty: bool,
        save: SaveResult,
    ) -> Entity<Demo> {
        app.update(|cx| {
            cx.new(|_| Demo {
                name: name.into(),
                dirty,
                save,
            })
        })
    }

    fn delegate(mode: Mode) -> Delegate {
        Delegate {
            mode: Rc::new(Cell::new(mode)),
            active: Rc::new(Cell::new(0)),
            resigned: Rc::new(Cell::new(0)),
            reopened: Rc::new(Cell::new(0)),
            launched: Rc::new(Cell::new(0)),
        }
    }

    fn coordinator(mode: Mode) -> QuitCoordinator {
        QuitCoordinator::new(Box::new(delegate(mode)))
    }

    #[test]
    fn two_dirty_documents_are_reviewed_one_by_one_and_cancel_keeps_the_app() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let one = document(&mut app, "One", true, SaveResult::Saved);
        let two = document(&mut app, "Two", true, SaveResult::Saved);
        let mut quit = coordinator(Mode::Now);
        quit.add_document(Box::new(one.clone()));
        quit.add_document(Box::new(two.clone()));

        assert_eq!(
            app.update(|cx| quit.request_quit(cx)),
            QuitStep::Review { remaining: 2 }
        );
        assert!(quit.needs_review_alert());
        assert_eq!(
            app.update(|cx| quit.review_name(cx)).as_deref(),
            Some("One")
        );
        assert_eq!(
            app.update(|cx| quit.resolve_review(ReviewChoice::DontSave, cx)),
            QuitStep::Review { remaining: 1 }
        );
        assert_eq!(
            app.update(|cx| quit.resolve_review(ReviewChoice::Cancel, cx)),
            QuitStep::Cancelled
        );
        assert!(app.update(|cx| one.read(cx).is_dirty()));
        assert!(app.update(|cx| two.read(cx).is_dirty()));
    }

    #[test]
    fn saving_each_document_terminates_and_a_panel_waits_for_the_result() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let one = document(&mut app, "One", true, SaveResult::NeedsPanel);
        let two = document(&mut app, "Two", true, SaveResult::Saved);
        let mut quit = coordinator(Mode::Now);
        quit.add_document(Box::new(one.clone()));
        quit.add_document(Box::new(two));
        assert_eq!(
            app.update(|cx| quit.request_quit(cx)),
            QuitStep::Review { remaining: 2 }
        );
        assert_eq!(
            app.update(|cx| quit.resolve_review(ReviewChoice::Save, cx)),
            QuitStep::Saving
        );
        assert_eq!(
            app.update(|cx| quit.resolve_save(SaveResult::Saved, cx)),
            QuitStep::Review { remaining: 1 }
        );
        assert_eq!(
            app.update(|cx| quit.resolve_review(ReviewChoice::Save, cx)),
            QuitStep::Terminate
        );
    }

    #[test]
    fn clean_documents_terminate_and_a_failed_save_cancels_the_quit() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let clean = document(&mut app, "Clean", false, SaveResult::Saved);
        let mut quit = coordinator(Mode::Now);
        quit.add_document(Box::new(clean));
        assert_eq!(app.update(|cx| quit.request_quit(cx)), QuitStep::Terminate);

        let dirty = document(&mut app, "Dirty", true, SaveResult::Failed("no".into()));
        let mut quit = coordinator(Mode::Now);
        quit.add_document(Box::new(dirty));
        assert_eq!(
            app.update(|cx| quit.request_quit(cx)),
            QuitStep::Review { remaining: 1 }
        );
        assert_eq!(
            app.update(|cx| quit.resolve_review(ReviewChoice::Save, cx)),
            QuitStep::Cancelled
        );
    }

    #[test]
    fn a_delegate_later_waits_and_a_timeout_forces_exit() {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let mut quit = coordinator(Mode::Later);
        assert_eq!(app.update(|cx| quit.request_quit(cx)), QuitStep::Deferred);
        assert_eq!(
            app.update(|cx| quit.deferred_finished(cx)),
            QuitStep::Terminate
        );
        assert_eq!(quit.timeout_elapsed(), QuitStep::Terminate);
    }

    #[test]
    fn a_delegate_can_cancel_the_quit() {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let mut quit = coordinator(Mode::Cancel);
        assert_eq!(app.update(|cx| quit.request_quit(cx)), QuitStep::Cancelled);
    }

    struct Blank;
    impl gpui::Render for Blank {
        fn render(
            &mut self,
            _: &mut gpui::Window,
            _: &mut Context<Self>,
        ) -> impl gpui::IntoElement {
            gpui::div()
        }
    }

    /// `install` drives the delegate: launch once, app activation derived from
    /// its windows with no flicker when focus moves between two of them, a
    /// resign when the last one deactivates, and reopen.
    #[test]
    fn install_drives_launch_activation_and_reopen() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let d = delegate(Mode::Now);
        let (launched, active, resigned, reopened) = (
            d.launched.clone(),
            d.active.clone(),
            d.resigned.clone(),
            d.reopened.clone(),
        );
        let shared = Rc::new(RefCell::new(d));
        app.update(|cx| install(shared.clone(), cx));
        assert_eq!(launched.get(), 1);

        let open = |app: &mut TestAppContext| {
            app.update(|cx| {
                crate::chrome::open_window(gpui::WindowOptions::default(), cx, |_, cx| {
                    cx.new(|_| Blank)
                })
                .unwrap()
            })
        };
        let first = open(&mut app);
        let second = open(&mut app);
        app.run_until_parked();
        let settle = |app: &mut TestAppContext| {
            app.run_until_parked();
            app.executor()
                .advance_clock(ACTIVATION_GRACE + std::time::Duration::from_millis(1));
            app.run_until_parked();
        };
        let activate = |app: &mut TestAppContext, w: gpui::WindowHandle<Blank>| {
            w.update(app, |_, window, _| window.activate_window())
                .unwrap();
            settle(app);
        };
        activate(&mut app, first);
        assert_eq!(active.get(), 1);
        assert!(app.update(|cx| is_app_active(cx)));
        activate(&mut app, second);
        assert_eq!(
            (active.get(), resigned.get()),
            (1, 0),
            "moving between the app's windows is not a resign"
        );
        app.update(|cx| {
            second
                .update(cx, |_, window, cx| {
                    window_activation_changed(window.window_handle().window_id(), false, cx)
                })
                .unwrap()
        });
        settle(&mut app);
        assert_eq!(resigned.get(), 1);
        assert!(!app.update(|cx| is_app_active(cx)));

        app.update(reopen);
        assert_eq!(reopened.get(), 1);
        // The quit coordinator shares the same delegate.
        let mut quit = QuitCoordinator::new(Box::new(shared.clone()));
        assert!(matches!(
            app.update(|cx| quit.request_quit(cx)),
            QuitStep::Terminate
        ));
    }

    #[test]
    fn activation_hooks_fire_once_per_signal() {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let mut d = delegate(Mode::Now);
        let active = d.active.clone();
        let resigned = d.resigned.clone();
        let reopened = d.reopened.clone();
        app.update(|cx| notify_activation(&mut d, true, cx));
        app.update(|cx| notify_activation(&mut d, false, cx));
        app.update(|cx| d.reopen(cx));
        assert_eq!(active.get(), 1);
        assert_eq!(resigned.get(), 1);
        assert_eq!(reopened.get(), 1);
    }
}
