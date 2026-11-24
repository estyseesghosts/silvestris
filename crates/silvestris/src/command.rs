//! Shared command dispatch (#129; handbook 12).
//! UI/shortcut adapters revalidate descriptors at invocation time, then run
//! the application callback outside the registry's entity borrow.
use crate::controls::{Button, ButtonStyle, ControlSize};
use crate::responder::ResponderChain;
use gpui::{
    prelude::*, App, Context, ElementId, Entity, KeyBinding, KeyBindingContextPredicate, Window,
};
use silvestris_core::command::{CommandDescriptor, CommandId, CommandSet};
use std::{collections::BTreeMap, rc::Rc};

/// HIG Appendix A shortcuts the system owns. A user-editable binding that
/// matches one is accepted but reported, so a preferences pane can warn.
pub const RESERVED_SHORTCUTS: &[&str] = &[
    "secondary-q",
    "secondary-w",
    "secondary-m",
    "secondary-h",
    "secondary-option-escape",
    "secondary-shift-q",
    "secondary-tab",
    "secondary-space",
    "secondary-`",
];

/// Why a rebind was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RebindError {
    Unknown(CommandId),
    InvalidShortcut(String),
    /// Another command in an overlapping context already owns the chord.
    Conflict {
        other: CommandId,
        shortcut: String,
    },
}

/// A rebind that succeeded but needs the user's attention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RebindWarning {
    /// The chord is a reserved system shortcut (HIG Appendix A).
    Reserved,
}

#[derive(Clone, Debug, PartialEq, gpui::Action)]
#[action(namespace = silvestris, no_json)]
pub struct InvokeCommand {
    pub id: String,
}

type Handler = Rc<dyn Fn(&CommandId, &mut Window, &mut App)>;

#[derive(Default)]
pub struct CommandRegistry {
    descriptors: CommandSet,
    handlers: BTreeMap<CommandId, Handler>,
    /// Focus context each command is bound in; used for conflict detection.
    binding_contexts: BTreeMap<CommandId, String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Invocation {
    Dispatched,
    Missing,
    Disabled,
}

impl CommandRegistry {
    pub fn descriptors(&self) -> &CommandSet {
        &self.descriptors
    }

    /// Reject duplicate handlers so registration order never selects behavior.
    pub fn register(
        &mut self,
        descriptor: CommandDescriptor,
        handler: impl Fn(&CommandId, &mut Window, &mut App) + 'static,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.handlers.contains_key(&descriptor.id) {
            return Err(format!("duplicate command: {}", descriptor.id));
        }
        self.handlers
            .insert(descriptor.id.clone(), Rc::new(handler));
        self.descriptors.upsert(descriptor);
        cx.notify();
        Ok(())
    }

    /// Publish app-owned enabled/checked/label state. Unknown IDs are rejected.
    pub fn publish(
        &mut self,
        descriptor: CommandDescriptor,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if !self.handlers.contains_key(&descriptor.id) {
            return Err(format!("unknown command: {}", descriptor.id));
        }
        let before = self.descriptors.version;
        self.descriptors.upsert(descriptor);
        if before != self.descriptors.version {
            cx.notify();
        }
        Ok(())
    }

    pub fn remove(&mut self, id: &CommandId, cx: &mut Context<Self>) {
        if self.handlers.remove(id).is_some() {
            self.descriptors.remove(id);
            cx.notify();
        }
    }

    /// Call this for toolbar, menu, context and shortcut activation alike.
    pub fn invoke(
        registry: &Entity<Self>,
        id: &CommandId,
        window: &mut Window,
        cx: &mut App,
    ) -> Invocation {
        let handler = {
            let state = registry.read(cx);
            let Some(descriptor) = state.descriptors.get(id) else {
                return Invocation::Missing;
            };
            if !descriptor.enabled {
                return Invocation::Disabled;
            }
            state.handlers.get(id).cloned()
        };
        let Some(handler) = handler else {
            return Invocation::Missing;
        };
        handler(id, window, cx);
        Invocation::Dispatched
    }

    /// Record the focus context a command is bound in so `rebind` can tell a
    /// real conflict from two commands in unrelated contexts.
    pub fn set_binding_context(&mut self, id: impl Into<CommandId>, context: impl Into<String>) {
        self.binding_contexts.insert(id.into(), context.into());
    }

    /// Change a command's shortcut. The caller then calls [`apply_bindings`] to
    /// replace the GPUI keymap.
    ///
    /// A chord that conflicts with another command in an overlapping context,
    /// or a shortcut that does not parse, is refused and leaves the old value.
    /// A reserved HIG shortcut is accepted with [`RebindWarning::Reserved`].
    pub fn rebind(
        &mut self,
        id: &CommandId,
        shortcut: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<Vec<RebindWarning>, RebindError> {
        let Some(mut descriptor) = self.descriptors.get(id).cloned() else {
            return Err(RebindError::Unknown(id.clone()));
        };
        let os = cx.platform_info().os;
        let mut warnings = Vec::new();
        let mut canonical = None;
        if let Some(shortcut) = shortcut {
            let keys = canonicalize(shortcut, os)?;
            canonical = Some(keys.clone());
            if RESERVED_SHORTCUTS
                .iter()
                .any(|reserved| canonicalize(reserved, os).is_ok_and(|r| r == keys))
            {
                warnings.push(RebindWarning::Reserved);
            }
            let context = self.binding_contexts.get(id).cloned().unwrap_or_default();
            for other_descriptor in self.descriptors.iter() {
                let other = &other_descriptor.id;
                if other == id {
                    continue;
                }
                let Some(other_keys) = other_descriptor.shortcut.as_deref() else {
                    continue;
                };
                let Ok(other_keys) = canonicalize(other_keys, os) else {
                    continue;
                };
                if other_keys != keys {
                    continue;
                }
                let other_context = self
                    .binding_contexts
                    .get(other)
                    .cloned()
                    .unwrap_or_default();
                if contexts_overlap(&context, &other_context) {
                    return Err(RebindError::Conflict {
                        other: other.clone(),
                        shortcut: shortcut.to_string(),
                    });
                }
            }
        }
        descriptor.shortcut = match (canonical, shortcut) {
            (Some(canonical), _) => Some(std::sync::Arc::from(canonical)),
            (None, Some(shortcut)) => Some(std::sync::Arc::from(shortcut)),
            (None, None) => None,
        };
        self.descriptors.upsert(descriptor);
        cx.notify();
        Ok(warnings)
    }

    /// The current shortcut of one command, as published.
    #[must_use]
    pub fn shortcut(&self, id: &CommandId) -> Option<&str> {
        self.descriptors
            .get(id)
            .and_then(|descriptor| descriptor.shortcut.as_deref())
    }

    /// Construct startup bindings in an explicit focus context. Loading resolves
    /// secondary- using GPUI's host OS. Consumer mounts an InvokeCommand handler
    /// on the same scope and observes the registry for rendering invalidation.
    pub fn bindings(&self, context: &str, cx: &App) -> Result<Vec<KeyBinding>, String> {
        self.bindings_where(context, |_| true, cx)
    }

    /// `bindings` for the descriptors `include` accepts. Use it to leave out
    /// commands whose IDs change at runtime (startup bindings would go stale)
    /// or whose shortcut keys must reach another responder first.
    pub fn bindings_where(
        &self,
        context: &str,
        include: impl Fn(&CommandDescriptor) -> bool,
        cx: &App,
    ) -> Result<Vec<KeyBinding>, String> {
        if context.trim().is_empty() {
            return Err("command bindings require an explicit focus context".into());
        }
        // No command shortcut fires while a text client has focus (#57): the
        // client gets every key first, as `!SilvestrisTextField` used to say by
        // hand. `SilvestrisText` covers field, area, token field and cell.
        let outside_text = KeyBindingContextPredicate::parse(&format!(
            "({context}) && !{}",
            crate::controls::text_field::KEY_CONTEXT
        ))
        .map_err(|e| e.to_string())?;
        let mut seen = BTreeMap::new();
        let mut bindings = Vec::new();
        for descriptor in self.descriptors.iter().filter(|d| include(d)) {
            let Some(keys) = &descriptor.shortcut else {
                continue;
            };
            if keys.trim().is_empty() {
                return Err(format!("empty shortcut: {}", descriptor.id));
            }
            let canonical = keys
                .split_whitespace()
                .map(|s| {
                    gpui::Keystroke::parse_with_os(s, cx.platform_info().os)
                        .map(|k| format!("{k:?}"))
                        .map_err(|e| e.to_string())
                })
                .collect::<Result<Vec<_>, _>>()?
                .join(" ");
            if let Some(other) = seen.insert(canonical, descriptor.id.clone()) {
                return Err(format!("shortcut conflict: {other} and {}", descriptor.id));
            }
            bindings.push(
                KeyBinding::load(
                    keys,
                    Box::new(InvokeCommand {
                        id: descriptor.id.to_string(),
                    }),
                    Some(Rc::new(outside_text.clone())),
                    false,
                    None,
                    cx,
                )
                .map_err(|e| e.to_string())?,
            );
        }
        Ok(bindings)
    }
}

/// Replace the GPUI command bindings from the registry while keeping every
/// non-command binding (text fields, theme actions, app shortcuts).
///
/// Call it after [`CommandRegistry::rebind`]. Each entry in `contexts` rebuilds
/// one focus context; pass the same contexts used at startup.
pub fn apply_bindings(
    registry: &Entity<CommandRegistry>,
    contexts: &[&str],
    cx: &mut App,
) -> Result<(), String> {
    let mut bindings = Vec::new();
    {
        let registry = registry.read(cx);
        for context in contexts {
            bindings.extend(registry.bindings(context, cx)?);
        }
    }
    // Collect the non-command bindings before clearing; keep the keymap borrow
    // out of the mutation below.
    let kept: Vec<KeyBinding> = {
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        keymap
            .bindings()
            .filter(|binding| {
                binding
                    .action()
                    .as_any()
                    .downcast_ref::<InvokeCommand>()
                    .is_none()
            })
            .cloned()
            .collect()
    };
    cx.clear_key_bindings();
    cx.bind_keys(kept);
    cx.bind_keys(bindings);
    Ok(())
}

fn canonicalize(shortcut: &str, os: gpui::OperatingSystem) -> Result<String, RebindError> {
    if shortcut.trim().is_empty() {
        return Err(RebindError::InvalidShortcut(shortcut.into()));
    }
    shortcut
        .split_whitespace()
        .map(|chord| {
            gpui::Keystroke::parse_with_os(chord, os)
                .map(|key| key.unparse(os))
                .map_err(|_| RebindError::InvalidShortcut(shortcut.into()))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|chords| chords.join(" "))
}

/// Whether two focus-context predicates can both be active at once. GPUI
/// predicates are general expressions; this compares the leading context name,
/// which is what command binding contexts use in practice. A command with no
/// recorded context is treated as overlapping every other.
#[must_use]
pub fn contexts_overlap(a: &str, b: &str) -> bool {
    fn base(context: &str) -> String {
        context
            .split(['&', '|', '!'])
            .next()
            .unwrap_or(context)
            .trim()
            .to_ascii_lowercase()
    }
    if a.trim().is_empty() || b.trim().is_empty() {
        return true;
    }
    let (a, b) = (base(a), base(b));
    a == b || a.starts_with(&b) || b.starts_with(&a)
}

/// A button that reads presentation from the registry and invokes the same ID
/// as menus and shortcuts. Its containing view must observe the registry.
#[derive(IntoElement)]
pub struct CommandButton {
    id: ElementId,
    command: CommandId,
    registry: Entity<CommandRegistry>,
    chain: Option<Entity<ResponderChain>>,
    style: ButtonStyle,
    size: ControlSize,
}
impl CommandButton {
    pub fn new(
        id: impl Into<ElementId>,
        command: impl Into<CommandId>,
        registry: Entity<CommandRegistry>,
    ) -> Self {
        Self {
            id: id.into(),
            command: command.into(),
            registry,
            chain: None,
            style: ButtonStyle::Textured,
            size: ControlSize::Regular,
        }
    }
    /// Resolve and validate through the window's responder chain (#57).
    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.chain = Some(chain);
        self
    }
    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
}
impl RenderOnce for CommandButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let descriptor = self
            .registry
            .read(cx)
            .descriptors
            .get(&self.command)
            .cloned();
        let label = descriptor
            .as_ref()
            .map(|d| d.menu_label())
            .unwrap_or_else(|| self.command.to_string());
        let enabled = descriptor.as_ref().is_some_and(|d| d.enabled);
        let checked = descriptor.as_ref().is_some_and(|d| d.checked);
        Button::new(self.id, label)
            .style(self.style)
            .size(self.size)
            .disabled(!enabled)
            .selected(checked)
            .on_click(move |_, window, cx| {
                cx.stop_propagation();
                match self.chain.clone() {
                    Some(chain) => {
                        let chain = chain.read(cx).clone();
                        chain.dispatch(&self.registry, &self.command, window, cx);
                    }
                    None => {
                        CommandRegistry::invoke(&self.registry, &self.command, window, cx);
                    }
                }
            })
    }
}

/// Place at the command responder boundary. Commands, including disabled or
/// removed IDs, are consumed here rather than bubbling toward guest input.
/// Text fields and transient menus should take priority via keymap predicates.
#[derive(IntoElement)]
pub struct CommandScope {
    id: ElementId,
    context: gpui::SharedString,
    registry: Entity<CommandRegistry>,
    chain: Option<Entity<ResponderChain>>,
    children: Vec<gpui::AnyElement>,
}
impl CommandScope {
    pub fn new(
        id: impl Into<ElementId>,
        context: impl Into<gpui::SharedString>,
        registry: Entity<CommandRegistry>,
    ) -> Self {
        Self {
            id: id.into(),
            context: context.into(),
            registry,
            chain: None,
            children: Vec::new(),
        }
    }
    /// Resolve InvokeCommand actions through the responder chain (#57) before
    /// falling back to the registry callback.
    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.chain = Some(chain);
        self
    }
}
impl ParentElement for CommandScope {
    fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.children.extend(elements);
    }
}
impl RenderOnce for CommandScope {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let chain = self.chain.clone();
        gpui::div()
            .id(self.id)
            .size_full()
            .key_context(self.context.as_ref())
            .on_action(move |action: &InvokeCommand, window, cx| {
                cx.stop_propagation();
                let id = CommandId::new(&action.id);
                match chain.clone() {
                    Some(chain) => {
                        let chain = chain.read(cx).clone();
                        chain.dispatch(&self.registry, &id, window, cx);
                    }
                    None => {
                        CommandRegistry::invoke(&self.registry, &id, window, cx);
                    }
                }
            })
            .children(self.children)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        div, px, Context, FocusHandle, Focusable, Modifiers, Render, TestAppContext,
        VisualTestContext,
    };
    use std::cell::Cell;
    struct Harness {
        registry: Entity<CommandRegistry>,
        focus: FocusHandle,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            CommandScope::new("scope", "CommandTest", self.registry.clone()).child(
                div().size_full().track_focus(&self.focus).child(
                    div()
                        .debug_selector(|| "command-button".into())
                        .w(px(120.))
                        .h(px(24.))
                        .child(CommandButton::new("run", "test.run", self.registry.clone())),
                ),
            )
        }
    }
    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    struct TextHarness {
        registry: Entity<CommandRegistry>,
        focus: FocusHandle,
        area: Entity<crate::controls::TextArea>,
    }
    impl Render for TextHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            CommandScope::new("scope", "CommandTest", self.registry.clone()).child(
                div()
                    .size_full()
                    .track_focus(&self.focus)
                    .child(div().w(px(200.)).child(self.area.clone())),
            )
        }
    }

    /// #57: no consumer writes `!SilvestrisTextField`. Command shortcuts stay
    /// out of every text client, the multiline area included, which the
    /// hand-written clause missed.
    #[test]
    fn command_shortcuts_stay_out_of_every_text_client() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            crate::controls::text_field::bind_keys(cx);
        });
        let bare = Rc::new(Cell::new(0));
        let chord = Rc::new(Cell::new(0));
        let registry = app.update(|cx| {
            let registry = cx.new(|_| CommandRegistry::default());
            let (b, c) = (bare.clone(), chord.clone());
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("test.bare", "Bare").shortcut("1"),
                    move |_, _, _| b.set(b.get() + 1),
                    cx,
                )
                .unwrap();
                r.register(
                    CommandDescriptor::action("test.chord", "Chord").shortcut("secondary-k"),
                    move |_, _, _| c.set(c.get() + 1),
                    cx,
                )
                .unwrap();
            });
            let keys = registry.read(cx).bindings("CommandTest", cx).unwrap();
            cx.bind_keys(keys);
            registry
        });
        let (view, cx) = app.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            TextHarness {
                registry: registry.clone(),
                focus,
                area: cx.new(|cx| crate::controls::TextArea::new(window, cx)),
            }
        });
        draw(cx);
        cx.simulate_keystrokes("1");
        draw(cx);
        assert_eq!(bare.get(), 1, "outside text the bare key is a command");

        cx.update(|window, cx| {
            let focus = view.read(cx).area.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        });
        draw(cx);
        cx.simulate_input("1");
        cx.simulate_keystrokes("1");
        draw(cx);
        assert_eq!(bare.get(), 1, "inside a text area the bare key types");
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-k"
        } else {
            "ctrl-k"
        });
        draw(cx);
        assert_eq!(
            chord.get(),
            0,
            "a chord does not leave the text area either"
        );
        assert_eq!(
            view.read_with(cx, |view, cx| view.area.read(cx).text().to_string()),
            "11"
        );
    }

    #[test]
    fn button_shortcut_and_direct_invocation_share_handler_and_live_enabled_state() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let hits = Rc::new(Cell::new(0));
        let registry = app.update(|cx| {
            let registry = cx.new(|_| CommandRegistry::default());
            let count = hits.clone();
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("test.run", "Run").shortcut("secondary-r"),
                    move |_, _, _| count.set(count.get() + 1),
                    cx,
                )
                .unwrap()
            });
            let keys = registry.read(cx).bindings("CommandTest", cx).unwrap();
            cx.bind_keys(keys);
            registry
        });
        let (view, cx) = app.add_window_view(|window, cx| {
            cx.observe(&registry, |_, _, cx| cx.notify()).detach();
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Harness {
                registry: registry.clone(),
                focus,
            }
        });
        draw(cx);
        let button = cx.debug_bounds("command-button").unwrap();
        cx.simulate_click(button.center(), Modifiers::default());
        draw(cx);
        assert_eq!(hits.get(), 1);
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-r"
        } else {
            "ctrl-r"
        });
        draw(cx);
        assert_eq!(hits.get(), 2);
        cx.update(|window, cx| {
            assert_eq!(
                CommandRegistry::invoke(&registry, &"test.run".into(), window, cx),
                Invocation::Dispatched
            )
        });
        assert_eq!(hits.get(), 3);
        cx.update(|_, cx| {
            registry.update(cx, |r, cx| {
                r.publish(
                    CommandDescriptor::action("test.run", "Run").enabled(false),
                    cx,
                )
                .unwrap()
            })
        });
        // Dispatch immediately, before a redraw of the stale enabled button.
        cx.update(|window, cx| {
            assert_eq!(
                CommandRegistry::invoke(&registry, &"test.run".into(), window, cx),
                Invocation::Disabled
            )
        });
        draw(cx);
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-r"
        } else {
            "ctrl-r"
        });
        assert_eq!(hits.get(), 3);
        cx.update(|_, cx| registry.update(cx, |r, cx| r.remove(&"test.run".into(), cx)));
        cx.update(|window, cx| {
            assert_eq!(
                CommandRegistry::invoke(&registry, &"test.run".into(), window, cx),
                Invocation::Missing
            )
        });
        cx.update(|_, cx| {
            assert_eq!(
                view.read(cx).registry.read(cx).descriptors().iter().count(),
                0
            )
        });
    }

    #[test]
    fn handler_can_publish_to_its_own_registry_and_conflicts_are_rejected() {
        let mut app = TestAppContext::single();
        let registry = app.update(|cx| cx.new(|_| CommandRegistry::default()));
        let weak = registry.downgrade();
        app.update(|cx| {
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("test.run", "Run").shortcut("secondary-r"),
                    move |id, _, cx| {
                        weak.update(cx, |r, cx| {
                            r.publish(
                                CommandDescriptor::action(id.as_str(), "Finished").enabled(false),
                                cx,
                            )
                            .unwrap()
                        })
                        .unwrap();
                    },
                    cx,
                )
                .unwrap();
                assert!(r
                    .register(
                        CommandDescriptor::action("test.run", "Duplicate"),
                        |_, _, _| {},
                        cx
                    )
                    .is_err());
                r.register(
                    CommandDescriptor::action("test.other", "Other").shortcut(
                        if cfg!(target_os = "macos") {
                            "cmd-r"
                        } else {
                            "ctrl-r"
                        },
                    ),
                    |_, _, _| {},
                    cx,
                )
                .unwrap();
                assert!(r.bindings("CommandTest", cx).is_err());
                assert!(r.bindings("", cx).is_err());
                // Filtering out one side of the conflict leaves one binding.
                let only = r
                    .bindings_where("CommandTest", |d| d.id.as_str() == "test.other", cx)
                    .unwrap();
                assert_eq!(only.len(), 1);
            })
        });
        let window = app.add_window(|_, cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            Harness {
                registry: registry.clone(),
                focus: cx.focus_handle(),
            }
        });
        window
            .update(&mut app, |_, window, cx| {
                assert_eq!(
                    CommandRegistry::invoke(&registry, &"test.run".into(), window, cx),
                    Invocation::Dispatched
                );
                assert!(
                    !registry
                        .read(cx)
                        .descriptors()
                        .get(&"test.run".into())
                        .unwrap()
                        .enabled
                );
            })
            .unwrap();
    }
}

#[cfg(test)]
mod rebind_tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{div, KeyBinding, TestAppContext, VisualTestContext};

    use super::*;
    use silvestris_core::command::CommandDescriptor;

    gpui::actions!(rebind_tests, [Other]);

    struct View {
        registry: Entity<CommandRegistry>,
        focus: gpui::FocusHandle,
    }

    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let registry = self.registry.clone();
            div()
                .size_full()
                .track_focus(&self.focus)
                .key_context("RebindTest")
                .on_action(move |action: &InvokeCommand, window, cx| {
                    cx.stop_propagation();
                    CommandRegistry::invoke(&registry, &CommandId::new(&action.id), window, cx);
                })
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    fn registered(app: &mut TestAppContext) -> (Entity<CommandRegistry>, Rc<Cell<usize>>) {
        app.update(|cx| {
            let hits = Rc::new(Cell::new(0));
            let counter = hits.clone();
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |registry, cx| {
                registry
                    .register(
                        CommandDescriptor::action("file.save", "Save").shortcut("secondary-a"),
                        move |_, _, _| counter.set(counter.get() + 1),
                        cx,
                    )
                    .unwrap();
                registry
                    .register(
                        CommandDescriptor::action("file.close", "Close").shortcut("secondary-b"),
                        |_, _, _| {},
                        cx,
                    )
                    .unwrap();
                registry.set_binding_context("file.save", "RebindTest");
                registry.set_binding_context("file.close", "RebindTest");
            });
            (registry, hits)
        })
    }

    #[test]
    fn a_conflict_in_the_same_context_is_refused_and_the_old_chord_remains() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (registry, _) = registered(&mut app);
        let save = CommandId::new("file.save");
        let before = app.update(|cx| registry.read(cx).shortcut(&save).map(str::to_string));
        let result = registry.update(&mut app, |registry, cx| {
            registry.rebind(&save, Some("secondary-b"), cx)
        });
        assert!(matches!(result, Err(RebindError::Conflict { .. })));
        let after = app.update(|cx| registry.read(cx).shortcut(&save).map(str::to_string));
        assert_eq!(before, after);
    }

    #[test]
    fn different_contexts_may_share_a_chord_and_reserved_shortcuts_are_flagged() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (registry, _) = registered(&mut app);
        registry.update(&mut app, |registry, _| {
            registry.set_binding_context("file.close", "OtherPane");
        });
        let save = CommandId::new("file.save");
        let allowed = registry.update(&mut app, |registry, cx| {
            registry.rebind(&save, Some("secondary-b"), cx)
        });
        assert_eq!(allowed, Ok(Vec::new()));

        let reserved = registry.update(&mut app, |registry, cx| {
            registry.rebind(&save, Some("secondary-q"), cx)
        });
        assert_eq!(reserved, Ok(vec![RebindWarning::Reserved]));

        let invalid = registry.update(&mut app, |registry, cx| {
            registry.rebind(&save, Some("   "), cx)
        });
        assert!(matches!(invalid, Err(RebindError::InvalidShortcut(_))));

        let unknown = registry.update(&mut app, |registry, cx| {
            registry.rebind(&CommandId::new("nope"), Some("secondary-k"), cx)
        });
        assert!(matches!(unknown, Err(RebindError::Unknown(_))));
    }

    #[test]
    fn applying_bindings_replaces_the_old_chord_and_keeps_other_bindings() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (registry, hits) = registered(&mut app);
        let (_view, cx) = app.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            View {
                registry: registry.clone(),
                focus,
            }
        });
        draw(cx);
        cx.update(|_, cx| {
            cx.bind_keys([KeyBinding::new("ctrl-z", Other, None)]);
            apply_bindings(&registry, &["RebindTest"], cx).expect("apply");
        });
        draw(cx);

        cx.simulate_keystrokes("secondary-a");
        draw(cx);
        assert_eq!(hits.get(), 1);

        let save = CommandId::new("file.save");
        registry.update(cx, |registry, cx| {
            registry
                .rebind(&save, Some("secondary-k"), cx)
                .expect("rebind")
        });
        cx.update(|_, cx| {
            apply_bindings(&registry, &["RebindTest"], cx).expect("reapply");
        });
        draw(cx);

        cx.simulate_keystrokes("secondary-a");
        draw(cx);
        assert_eq!(hits.get(), 1, "the old chord is gone");
        cx.simulate_keystrokes("secondary-k");
        draw(cx);
        assert_eq!(hits.get(), 2, "the new chord dispatches immediately");

        let kept = cx.update(|_, cx| {
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            keymap
                .bindings()
                .filter(|binding| binding.action().as_any().downcast_ref::<Other>().is_some())
                .count()
        });
        assert_eq!(kept, 1, "non-command bindings survive the rebuild");
    }
}
