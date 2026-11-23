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
        let predicate = KeyBindingContextPredicate::parse(context).map_err(|e| e.to_string())?;
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
                    Some(Rc::new(predicate.clone())),
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
        div, px, Context, FocusHandle, Modifiers, Render, TestAppContext, VisualTestContext,
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
            let keys = registry
                .read(cx)
                .bindings("CommandTest && !SilvestrisTextField", cx)
                .unwrap();
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
