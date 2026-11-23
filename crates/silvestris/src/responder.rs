//! Responder chain: which object handles a command given the current focus (#57).
//!
//! `CommandRegistry` keeps command metadata and the application's registered
//! callbacks. This module answers a different question: when Copy arrives, does
//! the focused text field handle it, the table behind it, the document, the
//! window or the application? A chain is built per window from the outermost
//! layer inward and resolved outward from the focused control, so the first
//! active responder that claims a command wins.
//!
//! GPUI still owns key dispatch and focus. The chain is consulted wherever an
//! invocation happens — a key binding, a menu row, a toolbar button or a
//! context menu — so presentation and dispatch share one answer.

use std::rc::Rc;

use gpui::{App, Entity, FocusHandle, SharedString, Window};
use silvestris_core::command::{CommandDescriptor, CommandId};

use crate::command::{CommandRegistry, Invocation};

/// Where a responder sits in the application. The chain order decides
/// precedence; the scope records intent and appears in diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResponderScope {
    /// A focused control: text field, table, slider, canvas.
    Control,
    /// The view that owns the control.
    View,
    /// A document controller or document window.
    Document,
    /// Window-level policy.
    Window,
    /// The application delegate, the outermost layer.
    Application,
}

/// Presentation and dispatch state for one command.
#[derive(Clone, Debug, PartialEq)]
pub struct ResponderState {
    pub enabled: bool,
    pub checked: bool,
    /// Replaces the registry label when present, for example "Undo Typing".
    pub title: Option<SharedString>,
}

impl ResponderState {
    #[must_use]
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            checked: false,
            title: None,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::enabled()
        }
    }

    #[must_use]
    pub fn checked(checked: bool) -> Self {
        Self {
            checked,
            ..Self::enabled()
        }
    }

    #[must_use]
    pub fn with_title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl Default for ResponderState {
    fn default() -> Self {
        Self::enabled()
    }
}

/// One layer that can claim commands. Implement it on the entity that owns the
/// state a command acts on, then wrap the entity in a [`ResponderEntry`].
///
/// `handles` decides ownership; `state` answers whether the command is enabled,
/// checked and what it is called right now; `perform` runs it. Validation and
/// dispatch go through the same three methods, so a menu row and a shortcut
/// cannot disagree about whether Save is available.
pub trait Responder: 'static {
    /// Whether this layer claims `id` while active. Returning false passes the
    /// command to the next layer outward.
    fn handles(&self, id: &CommandId) -> bool;

    /// Presentation state. Defaults to enabled and unchecked.
    fn state(&self, _id: &CommandId, _cx: &App) -> ResponderState {
        ResponderState::enabled()
    }

    /// Run the command. The entity read has ended before this is called.
    fn perform(&mut self, id: &CommandId, window: &mut Window, cx: &mut gpui::Context<Self>)
    where
        Self: Sized;
}

type CanHandle = Rc<dyn Fn(&CommandId, &App) -> bool>;
type Validate = Rc<dyn Fn(&CommandId, &App) -> ResponderState>;
type Perform = Rc<dyn Fn(&CommandId, &mut Window, &mut App)>;
type Active = Rc<dyn Fn(&Window, &App) -> bool>;

/// A chain member. Cheap to clone: the closures and predicates are shared.
#[derive(Clone)]
pub struct ResponderEntry {
    scope: ResponderScope,
    name: SharedString,
    can_handle: CanHandle,
    validate: Validate,
    perform: Perform,
    active: Active,
}

impl ResponderEntry {
    /// Wrap an entity that implements [`Responder`]. The entry holds weak
    /// references: if the entity is dropped, it stops claiming commands and
    /// dispatch falls through instead of panicking.
    pub fn new<T: Responder>(
        scope: ResponderScope,
        name: impl Into<SharedString>,
        entity: &Entity<T>,
    ) -> Self {
        let claims = entity.downgrade();
        let validate = entity.downgrade();
        let perform = entity.downgrade();
        Self {
            scope,
            name: name.into(),
            can_handle: Rc::new(move |id, cx| {
                claims
                    .upgrade()
                    .is_some_and(|entity| entity.read(cx).handles(id))
            }),
            validate: Rc::new(move |id, cx| {
                validate
                    .upgrade()
                    .map_or_else(ResponderState::disabled, |entity| {
                        entity.read(cx).state(id, cx)
                    })
            }),
            perform: Rc::new(move |id, window, cx| {
                let Some(entity) = perform.upgrade() else {
                    return;
                };
                entity.update(cx, |this, cx| this.perform(id, window, cx));
            }),
            active: Rc::new(|_, _| true),
        }
    }

    /// Build a responder from closures instead of an entity. Use it for the
    /// application delegate or for a layer that only validates.
    #[must_use]
    pub fn custom(scope: ResponderScope, name: impl Into<SharedString>) -> Self {
        Self {
            scope,
            name: name.into(),
            can_handle: Rc::new(|_, _| false),
            validate: Rc::new(|_, _| ResponderState::enabled()),
            perform: Rc::new(|_, _, _| {}),
            active: Rc::new(|_, _| true),
        }
    }

    #[must_use]
    pub fn handles(mut self, f: impl Fn(&CommandId, &App) -> bool + 'static) -> Self {
        self.can_handle = Rc::new(f);
        self
    }

    #[must_use]
    pub fn validate(mut self, f: impl Fn(&CommandId, &App) -> ResponderState + 'static) -> Self {
        self.validate = Rc::new(f);
        self
    }

    #[must_use]
    pub fn perform(mut self, f: impl Fn(&CommandId, &mut Window, &mut App) + 'static) -> Self {
        self.perform = Rc::new(f);
        self
    }

    /// Only claim commands while `f` is true for the current window.
    #[must_use]
    pub fn active_when(mut self, f: impl Fn(&Window, &App) -> bool + 'static) -> Self {
        self.active = Rc::new(f);
        self
    }

    /// Only claim commands while this focus handle contains the focus.
    #[must_use]
    pub fn when_focused(self, focus: &FocusHandle) -> Self {
        let focus = focus.clone();
        self.active_when(move |window, cx| focus.contains_focused(window, cx))
    }

    /// A canvas that swallows every command except `allow`. Commands in the
    /// list pass outward, which is how an emulator canvas receives no shell
    /// shortcut while the shell still receives the ones it owns.
    #[must_use]
    pub fn capture_keys(self, allow: impl IntoIterator<Item = CommandId>) -> Self {
        let allow: Vec<CommandId> = allow.into_iter().collect();
        let mut entry = self;
        entry.can_handle = Rc::new(move |id, _| !allow.contains(id));
        entry
    }

    #[must_use]
    pub fn scope(&self) -> ResponderScope {
        self.scope
    }

    #[must_use]
    pub fn name(&self) -> &SharedString {
        &self.name
    }

    #[must_use]
    pub fn is_active(&self, window: &Window, cx: &App) -> bool {
        (self.active)(window, cx)
    }

    #[must_use]
    pub fn claims(&self, id: &CommandId, cx: &App) -> bool {
        (self.can_handle)(id, cx)
    }

    #[must_use]
    pub fn state(&self, id: &CommandId, cx: &App) -> ResponderState {
        (self.validate)(id, cx)
    }

    /// Run this entry's handler. The caller must have cloned the entry out of
    /// the chain first, so no chain borrow is live.
    pub fn perform_now(&self, id: &CommandId, window: &mut Window, cx: &mut App) {
        (self.perform)(id, window, cx);
    }
}

/// Focus restoration for transient surfaces and sheets. The owner records the
/// focus before it opens and restores it when it closes.
#[derive(Default, Clone, Debug)]
pub struct FocusReturn {
    previous: Option<FocusHandle>,
}

impl FocusReturn {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the window's current focus. Call before moving focus into the
    /// transient.
    pub fn remember(&mut self, window: &Window, cx: &App) {
        self.previous = window.focused(cx);
    }

    /// Return focus to the recorded handle. Returns false when nothing was
    /// recorded. A dropped handle is harmless: GPUI clears the stale focus id
    /// on the next frame.
    pub fn restore(&mut self, window: &mut Window, cx: &mut App) -> bool {
        let Some(handle) = self.previous.take() else {
            return false;
        };
        window.focus(&handle, cx);
        true
    }

    #[must_use]
    pub fn recorded(&self) -> Option<&FocusHandle> {
        self.previous.as_ref()
    }

    pub fn forget(&mut self) {
        self.previous = None;
    }
}

/// An ordered set of responders for one window. Push from the outermost layer
/// inward; resolution walks back from the most recently pushed entry.
#[derive(Default, Clone)]
pub struct ResponderChain {
    entries: Vec<ResponderEntry>,
    /// Active layers captured when the innermost transient opened. While a
    /// transient or sheet owns the keyboard, commands still target the control
    /// that had focus when it opened (HIG: a menu does not change the first
    /// responder).
    frozen: Option<Vec<bool>>,
    transient_depth: usize,
}

impl ResponderChain {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a layer with higher precedence than the ones already present. Build
    /// a window as application → window → document → view → control.
    pub fn push(&mut self, entry: ResponderEntry) -> &mut Self {
        self.entries.push(entry);
        self
    }

    /// Replace an existing layer with the same scope and name, keeping its
    /// position. Use it when a view re-renders and re-registers.
    pub fn replace(&mut self, entry: ResponderEntry) -> &mut Self {
        if let Some(slot) = self
            .entries
            .iter_mut()
            .find(|existing| existing.scope == entry.scope && existing.name == entry.name)
        {
            *slot = entry;
        } else {
            self.entries.push(entry);
        }
        self
    }

    #[must_use]
    pub fn entries(&self) -> &[ResponderEntry] {
        &self.entries
    }

    /// Freeze resolution while a transient surface or sheet opens. Call it
    /// before moving focus into the transient. Nested calls are counted, so a
    /// submenu opening over a menu does not re-record the focus.
    pub fn begin_transient(&mut self, window: &Window, cx: &App) {
        if self.transient_depth == 0 {
            self.frozen = Some(
                self.entries
                    .iter()
                    .map(|entry| entry.is_active(window, cx))
                    .collect(),
            );
        }
        self.transient_depth += 1;
    }

    /// Release the freeze. The transient itself restores focus.
    pub fn end_transient(&mut self) {
        self.transient_depth = self.transient_depth.saturating_sub(1);
        if self.transient_depth == 0 {
            self.frozen = None;
        }
    }

    #[must_use]
    pub fn is_transient(&self) -> bool {
        self.transient_depth > 0
    }

    fn entry_is_active(&self, index: usize, window: &Window, cx: &App) -> bool {
        match &self.frozen {
            Some(active) => active.get(index).copied().unwrap_or(false),
            None => self.entries[index].is_active(window, cx),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The first active layer, from the inside out, that claims `id`.
    #[must_use]
    pub fn resolve(&self, id: &CommandId, window: &Window, cx: &App) -> Option<ResponderEntry> {
        self.entries
            .iter()
            .enumerate()
            .rev()
            .find(|(index, entry)| self.entry_is_active(*index, window, cx) && entry.claims(id, cx))
            .map(|(_, entry)| entry.clone())
    }

    /// Presentation state from the same resolver dispatch uses. `None` means no
    /// layer claims the command.
    #[must_use]
    pub fn validate(&self, id: &CommandId, window: &Window, cx: &App) -> Option<ResponderState> {
        self.resolve(id, window, cx)
            .map(|entry| entry.state(id, cx))
    }

    /// Invoke through the chain, falling back to the registry's registered
    /// callback when no layer claims the command. A claiming but disabled layer
    /// returns `Disabled` without falling through.
    pub fn dispatch(
        &self,
        registry: &Entity<CommandRegistry>,
        id: &CommandId,
        window: &mut Window,
        cx: &mut App,
    ) -> Invocation {
        let Some(entry) = self.resolve(id, window, cx) else {
            return CommandRegistry::invoke(registry, id, window, cx);
        };
        if !entry.state(id, cx).enabled {
            return Invocation::Disabled;
        }
        entry.perform_now(id, window, cx);
        Invocation::Dispatched
    }

    /// Registry metadata with the resolver's enabled, checked and title applied.
    /// Menus and toolbars use this so they show what dispatch will do.
    #[must_use]
    pub fn present(
        &self,
        registry: &Entity<CommandRegistry>,
        id: &CommandId,
        window: &Window,
        cx: &App,
    ) -> Option<CommandDescriptor> {
        let mut descriptor = registry.read(cx).descriptors().get(id)?.clone();
        if let Some(state) = self.validate(id, window, cx) {
            descriptor.enabled = state.enabled;
            descriptor.checked = state.checked;
            if let Some(title) = state.title {
                descriptor.label = title.to_string();
            }
        }
        Some(descriptor)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use gpui::{div, prelude::*, Context, Entity, Render, TestAppContext};

    use super::*;

    struct Target {
        focus: FocusHandle,
        log: Rc<RefCell<Vec<String>>>,
        enabled: Rc<Cell<bool>>,
        claims: Rc<Vec<CommandId>>,
    }

    impl Target {
        fn new(cx: &mut Context<Self>, claims: &[&str]) -> Self {
            Self {
                focus: cx.focus_handle(),
                log: Rc::new(RefCell::new(Vec::new())),
                enabled: Rc::new(Cell::new(true)),
                claims: Rc::new(claims.iter().map(CommandId::new).collect()),
            }
        }
    }

    impl Render for Target {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().track_focus(&self.focus)
        }
    }

    impl Responder for Target {
        fn handles(&self, id: &CommandId) -> bool {
            self.claims.contains(id)
        }

        fn state(&self, _id: &CommandId, _cx: &App) -> ResponderState {
            ResponderState {
                enabled: self.enabled.get(),
                ..ResponderState::enabled()
            }
        }

        fn perform(&mut self, id: &CommandId, _window: &mut Window, _cx: &mut Context<Self>) {
            self.log.borrow_mut().push(id.to_string());
        }
    }

    fn test_app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app
    }

    #[test]
    fn focus_selects_the_innermost_active_responder() {
        let mut app = test_app();
        let (field, field_log, field_focus) = app.update(|cx| {
            let field = cx.new(|cx| Target::new(cx, &["edit.copy"]));
            let target = field.read(cx);
            (field.clone(), target.log.clone(), target.focus.clone())
        });
        let (table, table_log, table_focus) = app.update(|cx| {
            let table = cx.new(|cx| Target::new(cx, &["edit.copy"]));
            let target = table.read(cx);
            (table.clone(), target.log.clone(), target.focus.clone())
        });
        let mut chain = ResponderChain::new();
        chain.push(ResponderEntry::new(ResponderScope::View, "table", &table));
        chain.push(
            ResponderEntry::new(ResponderScope::Control, "field", &field)
                .when_focused(&field_focus),
        );
        let registry = app.update(|cx| cx.new(|_| CommandRegistry::default()));
        let window = app.add_window(|_, cx| Target::new(cx, &[]));
        window
            .update(&mut app, |_, window, cx| {
                window.focus(&field_focus, cx);
                assert_eq!(
                    chain.dispatch(&registry, &"edit.copy".into(), window, cx),
                    Invocation::Dispatched
                );
                assert_eq!(*field_log.borrow(), vec!["edit.copy".to_string()]);
                assert!(table_log.borrow().is_empty());

                window.focus(&table_focus, cx);
                assert_eq!(
                    chain.dispatch(&registry, &"edit.copy".into(), window, cx),
                    Invocation::Dispatched
                );
                assert_eq!(*table_log.borrow(), vec!["edit.copy".to_string()]);
            })
            .unwrap();
    }

    #[test]
    fn a_disabled_responder_does_not_fall_through() {
        let mut app = test_app();
        let (field, focus, log, enabled) = app.update(|cx| {
            let field = cx.new(|cx| Target::new(cx, &["edit.copy"]));
            let target = field.read(cx);
            (
                field.clone(),
                target.focus.clone(),
                target.log.clone(),
                target.enabled.clone(),
            )
        });
        let mut chain = ResponderChain::new();
        chain.push(
            ResponderEntry::new(ResponderScope::Control, "field", &field).when_focused(&focus),
        );
        let registry = app.update(|cx| {
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("edit.copy", "Copy"),
                    |_, _, _| {},
                    cx,
                )
                .unwrap()
            });
            registry
        });
        enabled.set(false);
        let window = app.add_window(|_, cx| Target::new(cx, &[]));
        window
            .update(&mut app, |_, window, cx| {
                window.focus(&focus, cx);
                assert_eq!(
                    chain.dispatch(&registry, &"edit.copy".into(), window, cx),
                    Invocation::Disabled
                );
                assert!(log.borrow().is_empty());
            })
            .unwrap();
    }

    #[test]
    fn a_dropped_responder_passes_outward_without_panicking() {
        let mut app = test_app();
        let (chain, focus, app_log, _keep_alive): (
            ResponderChain,
            FocusHandle,
            Rc<RefCell<Vec<String>>>,
            Entity<Target>,
        ) = app.update(|cx| {
            let app_layer = cx.new(|cx| Target::new(cx, &["edit.copy"]));
            let app_log = app_layer.read(cx).log.clone();
            let field = cx.new(|cx| Target::new(cx, &["edit.copy"]));
            let focus = field.read(cx).focus.clone();
            let mut chain = ResponderChain::new();
            chain.push(ResponderEntry::new(
                ResponderScope::Application,
                "app",
                &app_layer,
            ));
            chain.push(
                ResponderEntry::new(ResponderScope::Control, "field", &field).when_focused(&focus),
            );
            drop(field);
            (chain, focus, app_log, app_layer)
        });
        let registry = app.update(|cx| cx.new(|_| CommandRegistry::default()));
        let window = app.add_window(|_, cx| Target::new(cx, &[]));
        window
            .update(&mut app, |_, window, cx| {
                window.focus(&focus, cx);
                assert_eq!(
                    chain.dispatch(&registry, &"edit.copy".into(), window, cx),
                    Invocation::Dispatched
                );
                assert_eq!(*app_log.borrow(), vec!["edit.copy".to_string()]);
            })
            .unwrap();
    }

    #[test]
    fn capture_swallows_commands_but_lets_the_allow_list_through() {
        let mut app = test_app();
        let (app_layer, app_log) = app.update(|cx| {
            let app_layer = cx.new(|cx| Target::new(cx, &["edit.copy", "file.save"]));
            let log = app_layer.read(cx).log.clone();
            (app_layer.clone(), log)
        });
        let (canvas_focus, chain) = app.update(|cx| {
            let focus = cx.focus_handle();
            let mut chain = ResponderChain::new();
            chain.push(ResponderEntry::new(
                ResponderScope::Application,
                "app",
                &app_layer,
            ));
            chain.push(
                ResponderEntry::custom(ResponderScope::Control, "canvas")
                    .when_focused(&focus)
                    .capture_keys([CommandId::new("file.save")]),
            );
            (focus, chain)
        });
        let registry = app.update(|cx| cx.new(|_| CommandRegistry::default()));
        let window = app.add_window(|_, cx| Target::new(cx, &[]));
        window
            .update(&mut app, |_, window, cx| {
                window.focus(&canvas_focus, cx);
                assert_eq!(
                    chain.dispatch(&registry, &"edit.copy".into(), window, cx),
                    Invocation::Dispatched
                );
                assert!(
                    app_log.borrow().is_empty(),
                    "the canvas swallowed Copy; the app must not see it"
                );
                assert_eq!(
                    chain.dispatch(&registry, &"file.save".into(), window, cx),
                    Invocation::Dispatched
                );
                assert_eq!(*app_log.borrow(), vec!["file.save".to_string()]);
                window.focus(&canvas_focus, cx);
                assert!(chain.claims_active(&"edit.copy".into(), window, cx));
            })
            .unwrap();
    }

    impl ResponderChain {
        fn claims_active(&self, id: &CommandId, window: &Window, cx: &App) -> bool {
            self.resolve(id, window, cx).is_some()
        }
    }

    #[test]
    fn present_applies_the_resolver_title_and_enabled_state() {
        let mut app = test_app();
        let (layer, focus, enabled) = app.update(|cx| {
            let layer = cx.new(|cx| {
                let mut target = Target::new(cx, &["edit.undo"]);
                target.enabled = Rc::new(Cell::new(false));
                target
            });
            let target = layer.read(cx);
            (layer.clone(), target.focus.clone(), target.enabled.clone())
        });
        let registry = app.update(|cx| {
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("edit.undo", "Undo"),
                    |_, _, _| {},
                    cx,
                )
                .unwrap()
            });
            registry
        });
        let mut chain = ResponderChain::new();
        chain.push(
            ResponderEntry::new(ResponderScope::Control, "field", &layer)
                .when_focused(&focus)
                .validate(|_, _| ResponderState::disabled().with_title("Undo Typing")),
        );
        let window = app.add_window(|_, cx| Target::new(cx, &[]));
        window
            .update(&mut app, |_, window, cx| {
                window.focus(&focus, cx);
                let descriptor = chain
                    .present(&registry, &"edit.undo".into(), window, cx)
                    .expect("descriptor");
                assert!(!descriptor.enabled);
                assert_eq!(descriptor.label, "Undo Typing");
            })
            .unwrap();
        enabled.set(true);
    }
}

#[cfg(test)]
mod entry_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{
        deferred, div, point, prelude::*, px, Context, FocusHandle, KeyBinding, Modifiers, Render,
        TestAppContext, VisualTestContext, Window,
    };
    use silvestris_core::command::{CommandDescriptor, CommandId, MenuEntry, MenuModel};

    use super::{Responder, ResponderChain, ResponderEntry, ResponderScope, ResponderState};
    use crate::command::{CommandButton, CommandRegistry, CommandScope, InvokeCommand};
    use crate::menu::PopupMenu;

    struct AppLayer {
        focus: FocusHandle,
        hits: Rc<RefCell<Vec<String>>>,
    }
    impl Render for AppLayer {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().track_focus(&self.focus)
        }
    }
    impl Responder for AppLayer {
        fn handles(&self, id: &CommandId) -> bool {
            id.as_str() == "test.copy"
        }

        fn perform(&mut self, _: &CommandId, _: &mut Window, _: &mut Context<Self>) {
            self.hits.borrow_mut().push("app".to_string());
        }
    }

    struct EntryPoints {
        focus: FocusHandle,
        hits: Rc<RefCell<Vec<String>>>,
        app: gpui::Entity<AppLayer>,
        registry: gpui::Entity<CommandRegistry>,
        chain: gpui::Entity<ResponderChain>,
        menu: gpui::Entity<PopupMenu>,
    }
    impl Responder for EntryPoints {
        fn handles(&self, id: &CommandId) -> bool {
            id.as_str() == "test.copy"
        }

        fn state(&self, _id: &CommandId, _cx: &gpui::App) -> ResponderState {
            ResponderState::enabled()
        }

        fn perform(&mut self, _: &CommandId, _: &mut Window, _: &mut Context<Self>) {
            self.hits.borrow_mut().push("view".to_string());
        }
    }
    impl Render for EntryPoints {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            CommandScope::new("scope", "ResponderTest", self.registry.clone())
                .chain(self.chain.clone())
                .child(
                    div()
                        .size_full()
                        .track_focus(&self.focus)
                        .child(
                            div()
                                .debug_selector(|| "resolver-button".into())
                                .w(px(120.))
                                .h(px(24.))
                                .child(
                                    CommandButton::new("run", "test.copy", self.registry.clone())
                                        .chain(self.chain.clone()),
                                ),
                        )
                        .child(deferred(self.menu.clone()).priority(1)),
                )
        }
    }

    fn model() -> MenuModel {
        MenuModel {
            label: "Edit".into(),
            entries: vec![MenuEntry::Command("test.copy".into())],
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    #[test]
    fn button_shortcut_menu_and_direct_call_share_one_resolver() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            let hits = Rc::new(RefCell::new(Vec::new()));
            let app_hits = Rc::new(RefCell::new(Vec::new()));
            let app_layer = cx.new(|cx| AppLayer {
                focus: cx.focus_handle(),
                hits: app_hits.clone(),
            });
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("test.copy", "Copy"),
                    |_, _, _| {},
                    cx,
                )
                .unwrap()
            });
            let this = cx.entity();
            let chain = cx.new(|_| ResponderChain::new());
            chain.update(cx, |chain, _cx| {
                chain.push(ResponderEntry::new(
                    ResponderScope::Application,
                    "app",
                    &app_layer,
                ));
                chain.push(
                    ResponderEntry::new(ResponderScope::Control, "view", &this)
                        .when_focused(&focus),
                );
            });
            window.focus(&focus, cx);
            let chord = if cfg!(target_os = "macos") {
                "cmd-r"
            } else {
                "ctrl-r"
            };
            cx.bind_keys([KeyBinding::new(
                chord,
                InvokeCommand {
                    id: "test.copy".into(),
                },
                Some("ResponderTest"),
            )]);
            let menu = cx.new(|cx| {
                let mut menu = PopupMenu::new(model(), registry.clone(), window, cx);
                menu.set_chain(chain.clone(), cx);
                menu
            });
            EntryPoints {
                focus,
                hits,
                app: app_layer,
                registry,
                chain,
                menu,
            }
        });
        draw(cx);

        let button = cx.debug_bounds("resolver-button").unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        draw(cx);
        assert_eq!(h.read_with(cx, |view, _| view.hits.borrow().len()), 1);

        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-r"
        } else {
            "ctrl-r"
        });
        draw(cx);
        assert_eq!(h.read_with(cx, |view, _| view.hits.borrow().len()), 2);

        cx.update(|window, cx| {
            let menu = h.read(cx).menu.clone();
            menu.update(cx, |menu, cx| {
                menu.open(point(px(20.), px(20.)), window, cx)
            });
        });
        draw(cx);
        let row = cx.debug_bounds("menu-item-0-0").unwrap();
        cx.simulate_click(row.center(), Modifiers::default());
        draw(cx);
        assert_eq!(h.read_with(cx, |view, _| view.hits.borrow().len()), 3);

        cx.update(|window, cx| {
            let chain = h.read(cx).chain.read(cx).clone();
            let registry = h.read(cx).registry.clone();
            assert_eq!(
                chain.dispatch(&registry, &CommandId::new("test.copy"), window, cx),
                crate::command::Invocation::Dispatched
            );
        });
        assert_eq!(
            h.read_with(cx, |view, _| view.hits.borrow().clone()),
            vec!["view", "view", "view", "view"]
        );

        cx.update(|window, cx| window.blur(cx));
        draw(cx);
        cx.update(|window, cx| {
            let chain = h.read(cx).chain.read(cx).clone();
            let registry = h.read(cx).registry.clone();
            assert_eq!(
                chain.dispatch(&registry, &CommandId::new("test.copy"), window, cx),
                crate::command::Invocation::Dispatched
            );
        });
        assert_eq!(
            h.read_with(cx, |view, _| view.hits.borrow().clone()),
            vec!["view", "view", "view", "view"]
        );
        assert_eq!(
            h.read_with(cx, |view, cx| view.app.read(cx).hits.borrow().clone()),
            vec!["app"]
        );
    }
}

#[cfg(test)]
mod document_tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{div, prelude::*, Context, FocusHandle, Render, Window};
    use silvestris_core::command::CommandId;

    use super::{Responder, ResponderChain, ResponderEntry, ResponderScope, ResponderState};
    use crate::command::{CommandRegistry, Invocation};
    use crate::test_support::scenario::Scenario;

    struct Document {
        focus: FocusHandle,
        log: Rc<RefCell<Vec<&'static str>>>,
    }

    struct DocWindow {
        document: gpui::Entity<Document>,
    }

    impl Render for DocWindow {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.document.clone().into_any_element()
        }
    }

    impl Render for Document {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().track_focus(&self.focus)
        }
    }

    impl Responder for Document {
        fn handles(&self, id: &CommandId) -> bool {
            matches!(id.as_str(), "edit.copy" | "file.save" | "file.close")
        }

        fn state(&self, id: &CommandId, _cx: &gpui::App) -> ResponderState {
            if id.as_str() == "file.save" {
                ResponderState::disabled()
            } else {
                ResponderState::enabled()
            }
        }

        fn perform(&mut self, id: &CommandId, _: &mut Window, _: &mut Context<Self>) {
            assert!(id.as_str() != "file.save", "disabled command was performed");
            self.log.borrow_mut().push("document");
        }
    }

    #[test]
    fn each_document_window_resolves_through_its_own_chain() {
        let mut scenario = Scenario::new();
        let mut builds = Vec::new();
        for _ in 0..2 {
            let log: Rc<RefCell<Vec<&'static str>>> = Rc::new(RefCell::new(Vec::new()));
            let build_log = log.clone();
            let slot: Rc<RefCell<Option<(FocusHandle, ResponderChain)>>> =
                Rc::new(RefCell::new(None));
            let build_slot = slot.clone();
            scenario.open(move |window, cx| {
                let focus = cx.focus_handle();
                let document = cx.new(|_| Document {
                    focus: focus.clone(),
                    log: build_log,
                });
                let mut chain = ResponderChain::new();
                chain.push(
                    ResponderEntry::new(ResponderScope::Document, "document", &document)
                        .when_focused(&focus),
                );
                let mut chain_cell = build_slot.borrow_mut();
                *chain_cell = Some((document.read(cx).focus.clone(), chain));
                drop(chain_cell);
                let handle = document.read(cx).focus.clone();
                window.focus(&handle, cx);
                DocWindow { document }
            });
            let (focus, chain) = slot.borrow_mut().take().expect("chain");
            builds.push((focus, chain, log));
        }
        let registry = scenario.update(|_, cx| cx.new(|_| CommandRegistry::default()));

        scenario.activate(scenario.windows()[0]);
        let (focus, chain, log) = &builds[0];
        scenario.focus(focus);
        scenario.update(|window, cx| {
            assert_eq!(
                chain.dispatch(&registry, &"edit.copy".into(), window, cx),
                Invocation::Dispatched
            );
            assert_eq!(
                chain.dispatch(&registry, &"file.save".into(), window, cx),
                Invocation::Disabled
            );
        });
        assert_eq!(*log.borrow(), vec!["document"]);
        assert!(builds[1].2.borrow().is_empty());

        scenario.activate(scenario.windows()[1]);
        let (other_focus, other_chain, other_log) = &builds[1];
        scenario.focus(other_focus);
        scenario.update(|window, cx| {
            assert_eq!(
                other_chain.dispatch(&registry, &"file.close".into(), window, cx),
                Invocation::Dispatched
            );
        });
        assert_eq!(*other_log.borrow(), vec!["document"]);
        assert_eq!(*log.borrow(), vec!["document"]);
    }
}
