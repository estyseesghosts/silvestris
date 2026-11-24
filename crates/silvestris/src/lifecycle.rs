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

use std::collections::VecDeque;

use gpui::{App, SharedString, Task};

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
    }

    impl AppDelegate for Delegate {
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
