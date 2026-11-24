//! Undo and redo with groups, coalescing and a limit (#39).
//!
//! `UndoManager` is an entity so a menu, a toolbar and the responder chain can
//! observe it. It stores reversible closures: text sessions record the change
//! they made, documents record their own save/revert steps, and an app with its
//! own history (chaus's Ropey editor) pushes entries directly.
//!
//! Coalescing is explicit. A text control merges contiguous typing into one
//! entry and re-amends the top with the merged range; a caller passes a
//! `coalesce` key to [`UndoManager::record`]. Any different key, a new group or
//! a redo clears the merge.
//!
//! `Responder` implements `edit.undo` and `edit.redo`, so a focused text field
//! can put its local stack at Control scope and a document can put its stack at
//! Document scope; the chain (#57) picks the innermost active one and the menu
//! title comes from the same `state`.

use std::ops::Range;
use std::rc::Rc;

use gpui::{App, AppContext, Context, Entity, SharedString};
use silvestris_core::command::CommandId;

use crate::responder::{Responder, ResponderScope, ResponderState};
use crate::text::{Change, EditSession};

/// One reversible step. `undo` and `redo` run with the app, after the manager's
/// borrow has ended.
pub type UndoCallback = Rc<dyn Fn(&mut App)>;

struct Entry {
    name: SharedString,
    coalesce: Option<u64>,
    undo: UndoCallback,
    redo: UndoCallback,
}

struct Group {
    name: SharedString,
    entries: Vec<Entry>,
}

/// Grouped undo/redo history.
pub struct UndoManager {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
    group: Option<Group>,
    limit: usize,
    revision: u64,
}

impl Default for UndoManager {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            group: None,
            limit: 100,
            revision: 0,
        }
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit.max(1);
        self.trim();
    }

    #[must_use]
    pub fn limit(&self) -> usize {
        self.limit
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[must_use]
    pub fn undo_name(&self) -> Option<&str> {
        self.undo.last().map(|entry| entry.name.as_ref())
    }

    #[must_use]
    pub fn redo_name(&self) -> Option<&str> {
        self.redo.last().map(|entry| entry.name.as_ref())
    }

    /// Push one step. Recording clears the redo stack unless `coalesce` matches
    /// the current top, which re-amends it (typing).
    pub fn record(
        &mut self,
        name: impl Into<SharedString>,
        coalesce: Option<u64>,
        undo: impl Fn(&mut App) + 'static,
        redo: impl Fn(&mut App) + 'static,
    ) {
        let entry = Entry {
            name: name.into(),
            coalesce,
            undo: Rc::new(undo),
            redo: Rc::new(redo),
        };
        if let Some(group) = self.group.as_mut() {
            group.entries.push(entry);
            return;
        }
        if let Some(top) = self.undo.last_mut() {
            if coalesce.is_some() && top.coalesce == coalesce {
                top.name = entry.name;
                top.undo = entry.undo;
                top.redo = entry.redo;
                self.redo.clear();
                self.revision += 1;
                return;
            }
        }
        self.undo.push(entry);
        self.redo.clear();
        self.trim();
        self.revision += 1;
    }

    /// Start a group. Entries recorded until [`end_group`](Self::end_group) undo
    /// and redo together; nested groups are flattened into the outer one.
    pub fn begin_group(&mut self, name: impl Into<SharedString>) {
        if self.group.is_none() {
            self.group = Some(Group {
                name: name.into(),
                entries: Vec::new(),
            });
        }
    }

    pub fn end_group(&mut self) {
        let Some(group) = self.group.take() else {
            return;
        };
        if group.entries.is_empty() {
            return;
        }
        let name = group.name;
        let entries = Rc::new(group.entries);
        let undo_entries = entries.clone();
        self.record_group(name, undo_entries);
    }

    fn record_group(&mut self, name: SharedString, entries: Rc<Vec<Entry>>) {
        let undo_list = entries.clone();
        let redo_list = entries;
        let undo = Rc::new(move |cx: &mut App| {
            for entry in undo_list.iter().rev() {
                (entry.undo)(cx);
            }
        });
        let redo = Rc::new(move |cx: &mut App| {
            for entry in redo_list.iter() {
                (entry.redo)(cx);
            }
        });
        self.undo.push(Entry {
            name,
            coalesce: None,
            undo,
            redo,
        });
        self.redo.clear();
        self.trim();
        self.revision += 1;
    }

    /// Undo the innermost step. Returns whether anything ran.
    pub fn undo(&mut self, cx: &mut App) -> bool {
        let Some(entry) = self.undo.pop() else {
            return false;
        };
        (entry.undo)(cx);
        self.redo.push(entry);
        self.revision += 1;
        true
    }

    /// Redo the innermost undone step.
    pub fn redo(&mut self, cx: &mut App) -> bool {
        let Some(entry) = self.redo.pop() else {
            return false;
        };
        (entry.redo)(cx);
        self.undo.push(entry);
        self.revision += 1;
        true
    }

    /// Clear both stacks. Call it after save or revert, as the app asks.
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.group = None;
        self.revision += 1;
    }

    fn trim(&mut self) {
        if self.undo.len() > self.limit {
            let excess = self.undo.len() - self.limit;
            self.undo.drain(0..excess);
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.undo.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

impl Responder for UndoManager {
    fn handles(&self, id: &CommandId) -> bool {
        matches!(id.as_str(), "edit.undo" | "edit.redo")
    }

    fn state(&self, id: &CommandId, _cx: &App) -> ResponderState {
        match id.as_str() {
            "edit.undo" => match self.undo_name() {
                Some(name) => ResponderState::enabled().with_title(format!("Undo {name}")),
                None => ResponderState::disabled().with_title("Undo"),
            },
            "edit.redo" => match self.redo_name() {
                Some(name) => ResponderState::enabled().with_title(format!("Redo {name}")),
                None => ResponderState::disabled().with_title("Redo"),
            },
            _ => ResponderState::disabled(),
        }
    }

    fn perform(&mut self, id: &CommandId, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        if id.as_str() == "edit.undo" {
            self.undo(cx);
        } else if id.as_str() == "edit.redo" {
            self.redo(cx);
        }
        cx.notify();
    }
}

/// A text control that exposes its session to [`TextHistory`].
pub trait TextHost: 'static {
    fn session(&self) -> &EditSession;
    fn session_mut(&mut self) -> &mut EditSession;
    /// Emit the client's Changed event and ask for a repaint after an undo or
    /// redo changed the buffer.
    fn after_history_change(&mut self, cx: &mut Context<Self>)
    where
        Self: Sized;
}

/// The local stack of one text control: an [`UndoManager`] plus the merging
/// state that turns a run of typing into one entry.
pub struct TextHistory {
    manager: Entity<UndoManager>,
    typing: Option<(Range<usize>, String, String)>,
    serial: u64,
}

impl TextHistory {
    #[must_use]
    pub fn new(cx: &mut App) -> Self {
        Self {
            manager: cx.new(|_| UndoManager::new()),
            typing: None,
            serial: 0,
        }
    }

    #[must_use]
    pub fn manager(&self) -> Entity<UndoManager> {
        self.manager.clone()
    }

    #[must_use]
    pub fn entry(
        &self,
        scope: ResponderScope,
        name: impl Into<SharedString>,
    ) -> crate::responder::ResponderEntry {
        entry(&self.manager, scope, name)
    }

    /// Record one session change. Composing text is skipped; a contiguous or
    /// cumulative insert amends the previous typing entry.
    pub fn record<T: TextHost>(&mut self, entity: Entity<T>, change: &Change, cx: &mut App) {
        if change.composing {
            return;
        }
        let insert = !change.inserted.is_empty();
        let (contiguous, cumulative) =
            self.typing
                .as_ref()
                .map_or((false, false), |(range, text, _)| {
                    if !insert {
                        return (false, false);
                    }
                    let contiguous = range.end == change.range.start && text.len() == range.len();
                    // Platform input may send cumulative replacements ("a", then
                    // "ab" over 0..1) rather than inserting "b" at 1.
                    let cumulative = change.range.start <= range.start
                        && change.inserted.starts_with(text.as_str());
                    (contiguous, cumulative)
                });
        let (range, removed, text, coalesce) = if contiguous || cumulative {
            let previous = self.typing.take();
            if cumulative {
                (
                    change.range.clone(),
                    change.removed.clone(),
                    change.inserted.clone(),
                    Some(self.serial),
                )
            } else {
                let (range, text, removed) = previous.unwrap_or_else(|| {
                    (
                        change.inserted_range(),
                        change.inserted.clone(),
                        String::new(),
                    )
                });
                (
                    range.start..range.start + text.len() + change.inserted.len(),
                    removed,
                    format!("{text}{}", change.inserted),
                    Some(self.serial),
                )
            }
        } else {
            self.serial += 1;
            (
                change.range.clone(),
                change.removed.clone(),
                change.inserted.clone(),
                None,
            )
        };
        self.typing = insert.then(|| {
            let stored = if contiguous || cumulative {
                range.clone()
            } else {
                change.inserted_range()
            };
            (stored, text.clone(), removed.clone())
        });
        let name = if insert { "Typing" } else { "Delete" };
        let coalesce = coalesce.or_else(|| insert.then_some(self.serial));
        let undo_entity = entity.clone();
        let undo_range = range.clone();
        let undo_text = text.clone();
        let undo_removed = removed.clone();
        let redo_entity = entity;
        let redo_range = range;
        let redo_text = text;
        let redo_removed = removed;
        self.manager.update(cx, |manager, _| {
            manager.record(
                name,
                coalesce,
                move |cx| {
                    undo_entity.update(cx, |host, cx| {
                        cancel_composition(host.session_mut());
                        let start = undo_range.start;
                        let end = start + undo_text.len();
                        host.session_mut()
                            .replace_range(start..end, &undo_removed, false);
                        host.after_history_change(cx);
                    });
                },
                move |cx| {
                    redo_entity.update(cx, |host, cx| {
                        cancel_composition(host.session_mut());
                        let start = redo_range.start;
                        let end = start + redo_removed.len();
                        host.session_mut()
                            .replace_range(start..end, &redo_text, false);
                        host.after_history_change(cx);
                    });
                },
            );
        });
    }

    /// Forget the typing merge, for example after the caret moved.
    pub fn forget_typing(&mut self) {
        self.typing = None;
    }
}

/// Drop an in-progress IME composition before history touches the buffer.
/// Marked text is never recorded (`TextHistory::record` skips composing
/// changes), so leaving it in place would shift every recorded range after it;
/// removing it restores the buffer the history describes (#39 trap: undo during
/// composition commits or cancels it first; Silvestris cancels).
fn cancel_composition(session: &mut EditSession) {
    if let Some(marked) = session.marked_range() {
        session.replace_range(marked, "", false);
        session.clear_marked();
    }
}

/// A responder entry for a manager at `scope`. Use Control for a focused text
/// session, Document for a document's own history, Application for an app-wide
/// one.
#[must_use]
pub fn entry(
    manager: &gpui::Entity<UndoManager>,
    scope: ResponderScope,
    name: impl Into<SharedString>,
) -> crate::responder::ResponderEntry {
    crate::responder::ResponderEntry::new(scope, name, manager)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{AppContext, TestAppContext};

    use super::*;

    fn app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app
    }

    #[test]
    fn undo_redo_run_in_reverse_and_forward_order() {
        let app = app();
        let log = Rc::new(RefCell::new(Vec::new()));
        let manager = app.update(|cx| cx.new(|_| UndoManager::new()));
        {
            let undo1 = log.clone();
            let redo1 = log.clone();
            let undo2 = log.clone();
            let redo2 = log.clone();
            app.update(|cx| {
                manager.update(cx, |manager, _| {
                    manager.record(
                        "One",
                        None,
                        move |_| undo1.borrow_mut().push("undo1"),
                        move |_| redo1.borrow_mut().push("redo1"),
                    );
                    manager.record(
                        "Two",
                        None,
                        move |_| undo2.borrow_mut().push("undo2"),
                        move |_| redo2.borrow_mut().push("redo2"),
                    );
                });
            });
        }
        app.update(|cx| {
            manager.update(cx, |manager, cx| {
                assert!(manager.undo(cx));
                assert!(manager.undo(cx));
                assert!(!manager.undo(cx));
                assert!(manager.redo(cx));
                assert!(manager.redo(cx));
            });
        });
        assert_eq!(*log.borrow(), vec!["undo2", "undo1", "redo1", "redo2"]);
    }

    #[test]
    fn contiguous_typing_amends_one_entry_and_the_limit_drops_the_oldest() {
        let app = app();
        let manager = app.update(|cx| cx.new(|_| UndoManager::new()));
        app.update(|cx| {
            manager.update(cx, |manager, _| {
                manager.set_limit(3);
                for _ in 0..3 {
                    manager.record("Typing", Some(7), |_| {}, |_| {});
                }
                assert_eq!(manager.undo_name(), Some("Typing"));
                assert_eq!(manager.undo.len(), 1, "typing coalesced");
                manager.record("Other", Some(8), |_| {}, |_| {});
                manager.begin_group("Paste");
                manager.record("a", None, |_| {}, |_| {});
                manager.record("b", None, |_| {}, |_| {});
                manager.end_group();
                for _ in 0..5 {
                    manager.record("Filler", None, |_| {}, |_| {});
                }
                assert!(manager.undo.len() <= 3);
            });
        });
    }

    #[test]
    fn a_group_undoes_and_redoes_as_one_step() {
        let app = app();
        let log = Rc::new(RefCell::new(Vec::new()));
        let manager = app.update(|cx| cx.new(|_| UndoManager::new()));
        {
            let a = log.clone();
            let b = log.clone();
            let c = log.clone();
            let d = log.clone();
            app.update(|cx| {
                manager.update(cx, |manager, _| {
                    manager.begin_group("Paste");
                    manager.record(
                        "a",
                        None,
                        move |_| a.borrow_mut().push("undo-a"),
                        move |_| b.borrow_mut().push("redo-a"),
                    );
                    manager.record(
                        "b",
                        None,
                        move |_| c.borrow_mut().push("undo-b"),
                        move |_| d.borrow_mut().push("redo-b"),
                    );
                    manager.end_group();
                });
            });
        }
        app.update(|cx| {
            manager.update(cx, |manager, cx| {
                assert_eq!(manager.undo_name(), Some("Paste"));
                manager.undo(cx);
                manager.redo(cx);
            });
        });
        assert_eq!(*log.borrow(), vec!["undo-b", "undo-a", "redo-a", "redo-b"]);
    }

    #[test]
    fn clear_empties_both_stacks_and_titles_project() {
        let app = app();
        let manager = app.update(|cx| cx.new(|_| UndoManager::new()));
        app.update(|cx| {
            manager.update(cx, |manager, _| {
                manager.record("Typing", None, |_| {}, |_| {});
            });
        });
        app.update(|cx| {
            let undo = manager.read(cx).state(&"edit.undo".into(), cx);
            assert!(undo.enabled);
            assert_eq!(undo.title.as_deref(), Some("Undo Typing"));
        });
        app.update(|cx| {
            manager.update(cx, |manager, cx| {
                manager.undo(cx);
            });
        });
        app.update(|cx| {
            let redo = manager.read(cx).state(&"edit.redo".into(), cx);
            assert_eq!(redo.title.as_deref(), Some("Redo Typing"));
            let undo = manager.read(cx).state(&"edit.undo".into(), cx);
            assert!(!undo.enabled);
        });
        app.update(|cx| {
            manager.update(cx, |manager, _| {
                manager.clear();
                assert!(!manager.can_undo() && !manager.can_redo());
            });
        });
    }

    #[test]
    fn a_manager_is_a_responder_for_undo_and_redo() {
        let app = app();
        let manager = app.update(|cx| cx.new(|_| UndoManager::new()));
        app.update(|cx| {
            let entry = entry(&manager, ResponderScope::Control, "field");
            assert!(entry.claims(&"edit.undo".into(), cx));
            assert!(!entry.claims(&"edit.save".into(), cx));
            let state = entry.state(&"edit.undo".into(), cx);
            assert!(!state.enabled);
        });
        app.update(|cx| {
            manager.update(cx, |manager, _| {
                manager.record("Typing", None, |_| {}, |_| {});
            });
        });
        app.update(|cx| {
            let entry = entry(&manager, ResponderScope::Control, "field");
            let state = entry.state(&"edit.undo".into(), cx);
            assert!(state.enabled);
            assert_eq!(state.title.as_deref(), Some("Undo Typing"));
        });
    }

    /// An editor with its own history (chaus's Ropey editor) is an undo
    /// provider: it implements `Responder` for `edit.undo`/`edit.redo` and
    /// never touches `UndoManager`. The chain picks it over the app's stack and
    /// the menu title comes from it (#39 done-when: provider bridged in a test).
    #[test]
    fn an_editor_with_its_own_history_is_an_undo_provider() {
        use crate::command::{CommandRegistry, Invocation};
        use crate::responder::{ResponderChain, ResponderEntry};
        use silvestris_core::command::CommandDescriptor;

        struct OwnHistory {
            done: Vec<&'static str>,
            undone: Vec<&'static str>,
        }
        impl Responder for OwnHistory {
            fn handles(&self, id: &CommandId) -> bool {
                matches!(id.as_str(), "edit.undo" | "edit.redo")
            }
            fn state(&self, id: &CommandId, _: &App) -> ResponderState {
                let top = match id.as_str() {
                    "edit.undo" => self.done.last(),
                    _ => self.undone.last(),
                };
                let verb = if id.as_str() == "edit.undo" {
                    "Undo"
                } else {
                    "Redo"
                };
                match top {
                    Some(name) => ResponderState::enabled().with_title(format!("{verb} {name}")),
                    None => ResponderState::disabled().with_title(verb),
                }
            }
            fn perform(&mut self, id: &CommandId, _: &mut gpui::Window, _: &mut Context<Self>) {
                let (from, to) = if id.as_str() == "edit.undo" {
                    (&mut self.done, &mut self.undone)
                } else {
                    (&mut self.undone, &mut self.done)
                };
                if let Some(step) = from.pop() {
                    to.push(step);
                }
            }
        }

        let mut app = app();
        let (editor, app_stack, registry) = app.update(|cx| {
            let editor = cx.new(|_| OwnHistory {
                done: vec!["Rename"],
                undone: Vec::new(),
            });
            let app_stack = cx.new(|_| UndoManager::new());
            app_stack.update(cx, |m, _| m.record("App step", None, |_| {}, |_| {}));
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |r, cx| {
                for (id, label) in [("edit.undo", "Undo"), ("edit.redo", "Redo")] {
                    r.register(CommandDescriptor::action(id, label), |_, _, _| {}, cx)
                        .unwrap();
                }
            });
            (editor, app_stack, registry)
        });
        let mut chain = ResponderChain::new();
        chain.push(entry(&app_stack, ResponderScope::Application, "app"));
        chain.push(ResponderEntry::new(
            ResponderScope::Document,
            "editor",
            &editor,
        ));
        let cx = app.add_empty_window();
        cx.update(|window, cx| {
            let undo = chain
                .present(&registry, &"edit.undo".into(), window, cx)
                .expect("descriptor");
            assert_eq!(undo.label, "Undo Rename");
            assert_eq!(
                chain.dispatch(&registry, &"edit.undo".into(), window, cx),
                Invocation::Dispatched
            );
            let redo = chain
                .present(&registry, &"edit.redo".into(), window, cx)
                .expect("descriptor");
            assert_eq!(redo.label, "Redo Rename");
        });
        assert_eq!(
            editor.read_with(cx, |e, _| e.undone.clone()),
            vec!["Rename"]
        );
        assert!(
            app_stack.read_with(cx, |m, _| m.can_undo()),
            "the built-in stack is not used"
        );
    }

    #[test]
    fn limit_is_at_least_one() {
        let mut manager = UndoManager::new();
        manager.set_limit(0);
        assert_eq!(manager.limit(), 1);
        manager.record("x", None, |_| {}, |_| {});
        manager.record("y", None, |_| {}, |_| {});
        assert_eq!(manager.undo.len(), 1);
    }
}
