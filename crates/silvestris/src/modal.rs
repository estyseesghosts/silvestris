//! Modal sessions: document-modal sheets and application-modal dialogs (#21).
//!
//! A modal session is the policy half of a sheet or app-modal dialog. It does
//! not draw anything (#22 owns the sheet layout and #23 the alert layout); it
//! answers four questions:
//!
//! - which commands the covered scope may still run while it is up;
//! - where Tab and Shift-Tab move inside the modal content;
//! - what happens when a second sheet targets the same document (it queues)
//!   or when an app-modal dialog opens over a sheet (it stacks on top);
//! - where the typed result goes, and that it is delivered as `Cancelled` if
//!   the owning window closes first.
//!
//! Evidence: HIG p. 235 (types of dialogs), p. 236 (sheets), p. 241 (dismissing
//! dialogs: Escape cancels, document-modal affects one document). The blocking
//! rule follows the HIG scope table: a document-modal sheet makes the covered
//! document modal but leaves sibling documents usable.
//!
//! ## Blocking
//!
//! `ResponderChain::dispatch` and `present` consult [`command_allowed`]. While a
//! session covers the window, a command resolves as `Disabled` unless it is in
//! the session's allow-list (editing commands by default, so a sheet's text
//! field keeps Copy/Paste). Document scope covers one window; application scope
//! covers every window. Nothing is blocked when no session is installed.
//!
//! ## Result
//!
//! `ModalSession::on_end` stores a one-shot callback receiving [`ModalResult`].
//! `finish` marks the session ended, removes it from the registry, restores
//! focus and invokes the callback. `cancel_owned` uses the same path with
//! `Cancelled`, which is what an application calls from
//! `on_window_should_close` before the window goes away.

use std::collections::{BTreeMap, BTreeSet};

use gpui::{AnyWindowHandle, App, Entity, FocusHandle, Global, Window, WindowId};
use silvestris_core::command::CommandId;

use crate::responder::FocusReturn;

/// What a session covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalScope {
    /// One document window. Siblings stay usable.
    Document,
    /// The whole application. Every window is blocked.
    Application,
}

/// How a session ended. `Cancelled` is the owner closing or a forced teardown;
/// `Rejected` is the user's Cancel or Escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalResult {
    Accepted,
    Rejected,
    Cancelled,
}

/// Lifecycle of one session. A queued session has not taken focus yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalPhase {
    Queued,
    Active,
    Ended(ModalResult),
}

type OnEnd = Box<dyn FnOnce(ModalResult)>;

/// The policy and result delivery for one sheet or app-modal dialog. The
/// application owns a `Entity<ModalSession>` and renders its content while the
/// phase is `Active`.
pub struct ModalSession {
    scope: ModalScope,
    phase: ModalPhase,
    allowed: BTreeSet<CommandId>,
    stops: Vec<FocusHandle>,
    focus: FocusReturn,
    owner: Option<AnyWindowHandle>,
    on_end: Option<OnEnd>,
}

impl ModalSession {
    /// A session with the default editing allow-list and no focus stops yet.
    #[must_use]
    pub fn new(scope: ModalScope) -> Self {
        let allowed = [
            "edit.copy",
            "edit.cut",
            "edit.paste",
            "edit.select-all",
            "edit.undo",
            "edit.redo",
        ]
        .into_iter()
        .map(CommandId::new)
        .collect();
        Self {
            scope,
            phase: ModalPhase::Queued,
            allowed,
            stops: Vec::new(),
            focus: FocusReturn::new(),
            owner: None,
            on_end: None,
        }
    }

    /// Allow one more command through the block. Use it for commands the
    /// modal content itself runs through the registry.
    #[must_use]
    pub fn allow(mut self, id: impl Into<CommandId>) -> Self {
        self.allowed.insert(id.into());
        self
    }

    /// Replace the allow-list. An empty list blocks every command while the
    /// session is active.
    #[must_use]
    pub fn allow_only(mut self, ids: impl IntoIterator<Item = CommandId>) -> Self {
        self.allowed = ids.into_iter().collect();
        self
    }

    /// The focus stops inside the modal content, in Tab order. Tab wraps from
    /// the last to the first and Shift-Tab from the first to the last.
    #[must_use]
    pub fn with_focus_stops(mut self, stops: Vec<FocusHandle>) -> Self {
        self.stops = stops;
        self
    }

    /// Replace the Tab stops after presentation. Views whose content changes
    /// (a panel switching list/column mode) use this so Tab never lands on a
    /// hidden control.
    pub fn set_focus_stops(&mut self, stops: Vec<FocusHandle>) {
        self.stops = stops;
    }

    /// Change the covered scope. A panel decides sheet vs app-modal before the
    /// deferred presentation runs.
    pub fn set_scope(&mut self, scope: ModalScope) {
        self.scope = scope;
    }

    /// The one-shot result callback.
    #[must_use]
    pub fn on_end(mut self, f: impl FnOnce(ModalResult) + 'static) -> Self {
        self.on_end = Some(Box::new(f));
        self
    }

    #[must_use]
    pub fn scope(&self) -> ModalScope {
        self.scope
    }

    #[must_use]
    pub fn phase(&self) -> ModalPhase {
        self.phase
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.phase == ModalPhase::Active
    }

    #[must_use]
    pub fn is_ended(&self) -> bool {
        matches!(self.phase, ModalPhase::Ended(_))
    }

    #[must_use]
    pub fn owner(&self) -> Option<AnyWindowHandle> {
        self.owner
    }

    #[must_use]
    pub fn allows(&self, id: &CommandId) -> bool {
        self.allowed.contains(id)
    }

    #[must_use]
    pub fn focus_stops(&self) -> &[FocusHandle] {
        &self.stops
    }

    /// Move focus to the first stop, or blur when the content declares none.
    pub fn focus_first(&self, window: &mut Window, cx: &mut App) {
        match self.stops.first() {
            Some(stop) => window.focus(stop, cx),
            None => window.blur(cx),
        }
    }
}

/// Tab (`forward`) or Shift-Tab inside the session's declared stops. A free
/// function because focusing needs `&mut App` while the session is borrowable.
pub fn cycle_focus(
    session: &Entity<ModalSession>,
    forward: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let stops = session.read(cx).stops.clone();
    if stops.is_empty() {
        return;
    }
    let current = stops.iter().position(|stop| stop.is_focused(window));
    let next = match (current, forward) {
        (Some(index), true) => (index + 1) % stops.len(),
        (Some(0), false) => stops.len() - 1,
        (Some(index), false) => index - 1,
        (None, _) => 0,
    };
    window.focus(&stops[next], cx);
}

/// Escape on an active session.
pub fn escape_session(session: Entity<ModalSession>, window: &mut Window, cx: &mut App) -> bool {
    if !session.read(cx).is_active() {
        return false;
    }
    finish(session, ModalResult::Rejected, window, cx);
    true
}

#[derive(Default)]
struct Registry {
    /// Active session first, then queued sessions, per document window.
    documents: BTreeMap<WindowId, Vec<Entity<ModalSession>>>,
    /// Application-modals in opening order; the last is the innermost.
    applications: Vec<Entity<ModalSession>>,
}

impl Global for Registry {}

fn ensure(cx: &mut App) {
    if cx.try_global::<Registry>().is_none() {
        cx.set_global(Registry::default());
    }
}

/// Wire the modal registry. `silvestris::init` calls this; tests that do not
/// run `init` get it lazily on the first `present`.
pub fn install(cx: &mut App) {
    ensure(cx);
}

/// Show a session. A document session on a window that already has one queues
/// behind it; an application session always becomes active on top.
pub fn present(session: Entity<ModalSession>, window: &mut Window, cx: &mut App) {
    ensure(cx);
    let scope = session.read(cx).scope;
    let handle = window.window_handle();
    let id = handle.window_id();
    session.update(cx, |session, cx| {
        session.owner = Some(handle);
        cx.notify();
    });
    match scope {
        ModalScope::Document => {
            let queued = {
                let registry = cx.global_mut::<Registry>();
                let entries = registry.documents.entry(id).or_default();
                let queued = !entries.is_empty();
                entries.push(session.clone());
                queued
            };
            if queued {
                session.update(cx, |session, cx| {
                    session.phase = ModalPhase::Queued;
                    cx.notify();
                });
            } else {
                activate(&session, window, cx);
            }
        }
        ModalScope::Application => {
            cx.global_mut::<Registry>()
                .applications
                .push(session.clone());
            activate(&session, window, cx);
        }
    }
}

fn activate(session: &Entity<ModalSession>, window: &mut Window, cx: &mut App) {
    let stops = session.update(cx, |session, cx| {
        session.phase = ModalPhase::Active;
        session.focus.remember(window, cx);
        cx.notify();
        session.stops.clone()
    });
    match stops.first() {
        Some(stop) => window.focus(stop, cx),
        None => window.blur(cx),
    }
}

/// End the session and deliver `result` once. A session that already ended is
/// ignored. The callback runs after the session is out of the registry and
/// focus is restored; it must not try to borrow the session.
pub fn finish(
    session: Entity<ModalSession>,
    result: ModalResult,
    window: &mut Window,
    cx: &mut App,
) {
    if session.read(cx).is_ended() {
        return;
    }
    let (scope, owner) = session.update(cx, |session, cx| {
        session.phase = ModalPhase::Ended(result);
        cx.notify();
        (session.scope, session.owner)
    });
    let id = owner.map(|handle| handle.window_id());
    {
        let registry = cx.global_mut::<Registry>();
        match scope {
            ModalScope::Document => {
                if let Some(id) = id {
                    if let Some(entries) = registry.documents.get_mut(&id) {
                        entries.retain(|entry| entry != &session);
                        if entries.is_empty() {
                            registry.documents.remove(&id);
                        }
                    }
                }
            }
            ModalScope::Application => {
                registry.applications.retain(|entry| entry != &session);
            }
        }
    }
    // Restore focus only when this session was the one holding it.
    session.update(cx, |session, cx| {
        if session.focus.recorded().is_some() {
            session.focus.restore(window, cx);
        }
        cx.notify();
    });
    advance(id, window, cx);

    let callback = session.update(cx, |session, _| session.on_end.take());
    if let Some(callback) = callback {
        callback(result);
    }
}

fn advance(id: Option<WindowId>, window: &mut Window, cx: &mut App) {
    let next = {
        let registry = cx.try_global::<Registry>();
        let Some(registry) = registry else {
            return;
        };
        if !registry.applications.is_empty() {
            return;
        }
        id.and_then(|id| registry.documents.get(&id))
            .and_then(|entries| entries.first())
            .cloned()
    };
    if let Some(next) = next {
        if !next.read(cx).is_active() {
            activate(&next, window, cx);
        }
    }
}

/// Whether `id` may run in `window` while a session covers it.
#[must_use]
pub fn command_allowed(window: &Window, id: &CommandId, cx: &App) -> bool {
    let Some(registry) = cx.try_global::<Registry>() else {
        return true;
    };
    if let Some(session) = registry.applications.last() {
        return session.read(cx).allows(id);
    }
    let window_id = window.window_handle().window_id();
    if let Some(session) = registry
        .documents
        .get(&window_id)
        .and_then(|entries| entries.first())
    {
        return session.read(cx).allows(id);
    }
    true
}

/// Whether any session covers `window`.
#[must_use]
pub fn is_modal(window: &Window, cx: &App) -> bool {
    let Some(registry) = cx.try_global::<Registry>() else {
        return false;
    };
    if !registry.applications.is_empty() {
        return true;
    }
    let window_id = window.window_handle().window_id();
    registry
        .documents
        .get(&window_id)
        .is_some_and(|entries| !entries.is_empty())
}

/// The session that currently owns `window`, if any.
#[must_use]
pub fn active(window: &Window, cx: &App) -> Option<Entity<ModalSession>> {
    let registry = cx.try_global::<Registry>()?;
    if let Some(session) = registry.applications.last() {
        return Some(session.clone());
    }
    let window_id = window.window_handle().window_id();
    registry
        .documents
        .get(&window_id)
        .and_then(|entries| entries.first())
        .cloned()
}

/// Cancel every session owned by `owner`, whether active or queued. Call this
/// from the application's window-close policy before the window is removed.
pub fn cancel_owned(owner: AnyWindowHandle, cx: &mut App) -> usize {
    ensure(cx);
    let id = owner.window_id();
    let mut sessions: Vec<Entity<ModalSession>> = Vec::new();
    let applications = {
        let registry = cx.global_mut::<Registry>();
        if let Some(entries) = registry.documents.remove(&id) {
            sessions.extend(entries);
        }
        std::mem::take(&mut registry.applications)
    };
    let mut keep = Vec::new();
    for session in applications {
        if session.read(cx).owner == Some(owner) {
            sessions.push(session);
        } else {
            keep.push(session);
        }
    }
    cx.global_mut::<Registry>().applications = keep;
    let count = sessions.len();
    for session in sessions {
        if session.read(cx).is_ended() {
            continue;
        }
        session.update(cx, |session, cx| {
            session.phase = ModalPhase::Ended(ModalResult::Cancelled);
            session.focus.forget();
            cx.notify();
        });
        let callback = session.update(cx, |session, _| session.on_end.take());
        if let Some(callback) = callback {
            callback(ModalResult::Cancelled);
        }
    }
    // A closing owner takes its sheet surface with it (the session policy
    // above still delivers Cancelled).
    crate::sheet::close_owner(id, cx);
    count
}

/// Clear every registry entry. For tests and application shutdown.
pub fn reset(cx: &mut App) {
    ensure(cx);
    let registry = cx.global_mut::<Registry>();
    registry.documents.clear();
    registry.applications.clear();
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use gpui::{div, prelude::*, Context, Render, TestAppContext, Window};

    use super::*;
    use crate::command::{CommandRegistry, Invocation};
    use crate::responder::ResponderChain;
    use silvestris_core::command::CommandDescriptor;

    struct View;

    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    fn test_app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app
    }

    fn registry(
        cx: &mut App,
    ) -> (
        Entity<CommandRegistry>,
        Entity<ResponderChain>,
        Rc<Cell<usize>>,
    ) {
        let hits = Rc::new(Cell::new(0));
        let counter = hits.clone();
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::action("file.save", "Save"),
                    move |_, _, _| counter.set(counter.get() + 1),
                    cx,
                )
                .unwrap();
            registry
                .register(
                    CommandDescriptor::action("edit.copy", "Copy"),
                    |_, _, _| {},
                    cx,
                )
                .unwrap();
        });
        let chain = cx.new(|_| ResponderChain::new());
        (registry, chain, hits)
    }

    fn dispatch(
        window: gpui::WindowHandle<View>,
        app: &mut TestAppContext,
        registry: &Entity<CommandRegistry>,
        chain: &Entity<ResponderChain>,
        id: &str,
    ) -> Invocation {
        window
            .update(app, |_, window, cx| {
                let chain = chain.read(cx).clone();
                chain.dispatch(registry, &id.into(), window, cx)
            })
            .unwrap()
    }

    #[test]
    fn a_document_session_blocks_its_window_and_leaves_the_sibling_usable() {
        let mut app = test_app();
        let first = app.add_window(|_, _| View);
        let second = app.add_window(|_, _| View);
        let (first_registry, first_chain, first_hits) = app.update(registry);
        let (second_registry, second_chain, second_hits) = app.update(registry);
        let session = app.update(|cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
        first
            .update(&mut app, |_, window, cx| {
                present(session.clone(), window, cx)
            })
            .unwrap();
        let blocked = dispatch(first, &mut app, &first_registry, &first_chain, "file.save");
        assert_eq!(blocked, Invocation::Disabled);
        assert_eq!(first_hits.get(), 0);

        let allowed = dispatch(first, &mut app, &first_registry, &first_chain, "edit.copy");
        assert_eq!(allowed, Invocation::Dispatched);

        let sibling = dispatch(
            second,
            &mut app,
            &second_registry,
            &second_chain,
            "file.save",
        );
        assert_eq!(sibling, Invocation::Dispatched);
        assert_eq!(second_hits.get(), 1);

        app.update(|cx| {
            first
                .update(cx, |_, window, cx| {
                    finish(session, ModalResult::Accepted, window, cx)
                })
                .unwrap();
        });
        assert_eq!(
            dispatch(first, &mut app, &first_registry, &first_chain, "file.save"),
            Invocation::Dispatched
        );
        assert_eq!(first_hits.get(), 1);
    }

    #[test]
    fn an_application_session_blocks_every_window_and_stacks_over_a_sheet() {
        let mut app = test_app();
        let first = app.add_window(|_, _| View);
        let second = app.add_window(|_, _| View);
        let (first_registry, first_chain, _) = app.update(registry);
        let (second_registry, second_chain, _) = app.update(registry);
        let sheet = app.update(|cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
        let alert = app.update(|cx| cx.new(|_| ModalSession::new(ModalScope::Application)));
        first
            .update(&mut app, |_, window, cx| {
                present(sheet.clone(), window, cx);
                present(alert.clone(), window, cx);
            })
            .unwrap();
        assert_eq!(
            dispatch(first, &mut app, &first_registry, &first_chain, "file.save"),
            Invocation::Disabled
        );
        assert_eq!(
            dispatch(
                second,
                &mut app,
                &second_registry,
                &second_chain,
                "file.save"
            ),
            Invocation::Disabled
        );
        first
            .update(&mut app, |_, window, cx| {
                assert_eq!(
                    active(window, cx).map(|s| s.read(cx).scope()),
                    Some(ModalScope::Application)
                );
            })
            .unwrap();
    }

    #[test]
    fn a_second_document_sheet_queues_and_advances_on_finish() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let results = Rc::new(RefCell::new(Vec::new()));
        let one = results.clone();
        let two = results.clone();
        let first = app.update(|cx| {
            cx.new(|_| {
                ModalSession::new(ModalScope::Document)
                    .on_end(move |result| one.borrow_mut().push(result))
            })
        });
        let second = app.update(|cx| {
            cx.new(|_| {
                ModalSession::new(ModalScope::Document)
                    .on_end(move |result| two.borrow_mut().push(result))
            })
        });
        window
            .update(&mut app, |_, window, cx| {
                present(first.clone(), window, cx);
                present(second.clone(), window, cx);
                assert_eq!(first.read(cx).phase(), ModalPhase::Active);
                assert_eq!(second.read(cx).phase(), ModalPhase::Queued);
                assert_eq!(active(window, cx), Some(first.clone()));
            })
            .unwrap();
        window
            .update(&mut app, |_, window, cx| {
                finish(first, ModalResult::Accepted, window, cx);
                assert_eq!(active(window, cx), Some(second.clone()));
                assert_eq!(second.read(cx).phase(), ModalPhase::Active);
            })
            .unwrap();
        window
            .update(&mut app, |_, window, cx| {
                finish(second, ModalResult::Rejected, window, cx);
                assert!(active(window, cx).is_none());
            })
            .unwrap();
        assert_eq!(
            *results.borrow(),
            vec![ModalResult::Accepted, ModalResult::Rejected]
        );
    }

    #[test]
    fn tab_cycles_inside_the_declared_stops_and_wraps() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let (one, two, three, session) = app.update(|cx| {
            let one = cx.focus_handle();
            let two = cx.focus_handle();
            let three = cx.focus_handle();
            let session = cx.new(|_| {
                ModalSession::new(ModalScope::Document).with_focus_stops(vec![
                    one.clone(),
                    two.clone(),
                    three.clone(),
                ])
            });
            (one, two, three, session)
        });
        window
            .update(&mut app, |_, window, cx| {
                present(session.clone(), window, cx);
                assert!(one.is_focused(window), "the first stop takes focus");
                cycle_focus(&session, true, window, cx);
                assert!(two.is_focused(window));
                cycle_focus(&session, true, window, cx);
                assert!(three.is_focused(window));
                cycle_focus(&session, true, window, cx);
                assert!(one.is_focused(window), "Tab wraps to the first stop");
                cycle_focus(&session, false, window, cx);
                assert!(three.is_focused(window), "Shift-Tab wraps to the last stop");
                finish(session, ModalResult::Rejected, window, cx);
            })
            .unwrap();
    }

    #[test]
    fn owner_close_cancels_active_and_queued_sessions() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let results = Rc::new(RefCell::new(Vec::new()));
        let one = results.clone();
        let two = results.clone();
        let sheet = app.update(|cx| {
            cx.new(|_| {
                ModalSession::new(ModalScope::Document).on_end(move |r| one.borrow_mut().push(r))
            })
        });
        let alert = app.update(|cx| {
            cx.new(|_| {
                ModalSession::new(ModalScope::Application).on_end(move |r| two.borrow_mut().push(r))
            })
        });
        window
            .update(&mut app, |_, window, cx| {
                present(sheet.clone(), window, cx);
                present(alert.clone(), window, cx);
            })
            .unwrap();
        let owner = *window;
        app.update(|cx| assert_eq!(cancel_owned(owner, cx), 2));
        assert_eq!(
            *results.borrow(),
            vec![ModalResult::Cancelled, ModalResult::Cancelled]
        );
        window
            .update(&mut app, |_, window, cx| {
                assert!(active(window, cx).is_none());
                assert!(!is_modal(window, cx));
                assert!(sheet.read(cx).is_ended());
                assert!(alert.read(cx).is_ended());
            })
            .unwrap();
    }

    #[test]
    fn escape_rejects_only_the_active_session() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let first = app.update(|cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
        let second = app.update(|cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
        window
            .update(&mut app, |_, window, cx| {
                present(first.clone(), window, cx);
                present(second.clone(), window, cx);
                assert!(!escape_session(second.clone(), window, cx));
                assert!(escape_session(first.clone(), window, cx));
                assert!(first.read(cx).is_ended());
                assert_eq!(second.read(cx).phase(), ModalPhase::Active);
            })
            .unwrap();
    }

    #[test]
    fn presentation_reads_disabled_while_a_session_blocks_it() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let (registry, chain, _) = app.update(registry);
        let session = app.update(|cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
        window
            .update(&mut app, |_, window, cx| {
                present(session, window, cx);
                let descriptor = chain
                    .read(cx)
                    .present(&registry, &"file.save".into(), window, cx)
                    .expect("descriptor");
                assert!(!descriptor.enabled);
                let copy = chain
                    .read(cx)
                    .present(&registry, &"edit.copy".into(), window, cx)
                    .expect("descriptor");
                assert!(copy.enabled);
            })
            .unwrap();
    }
}
