//! One transient-surface stack per window (#60).
//!
//! A menu, submenu, combo-box list, popover, tooltip or completion list is a
//! *surface*: it sits above the content, owns the pointer until it closes, and
//! dismisses for one reason (outside click, Escape, window deactivation,
//! scroll, focus loss). Before this module each control kept its own open flag,
//! shield, Escape branch and cancellation, so two transients could be open at
//! once and Escape closed the wrong one.
//!
//! `TransientLayer` is an entity with one stack. Controls push a surface when
//! they open, render the panel, and read `is_open` back instead of keeping a
//! second flag. The layer:
//!
//! - orders surfaces, so only the innermost one handles a dismissal;
//! - applies the policy for the surface kind;
//! - hands out a [`CancelToken`] that is cancelled when the surface closes, so
//!   async work cannot complete into a closed surface;
//! - freezes the responder chain (#57) while any surface owns the keyboard and
//!   restores focus when the stack empties;
//! - hides tooltips while a menu or popover is open.
//!
//! Rendering the panel and the full-window shield stays with the surface owner,
//! because it owns the geometry and z-order. Dismissal is not duplicated: the
//! owner attaches [`dismiss_on_pointer`] / [`dismiss_on_scroll`] to its shield
//! and calls [`TransientLayer::escape`] from its key handler.
//!
//! Escape order is defined here: innermost transient, then the sheet layer
//! (#21), then the application. A sheet is not a transient; when the stack is
//! empty `escape` returns false and the caller passes the key outward.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{div, prelude::*, Bounds, Context, Entity, MouseButton, Pixels, SharedString, Window};

use crate::responder::{FocusReturn, ResponderChain, ResponderScope};

/// What kind of surface is open. The kind selects the default policy and
/// appears in diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransientKind {
    /// A menu opened from a button, the menu bar or a shortcut.
    Menu,
    /// A submenu that belongs to an open menu.
    Submenu,
    /// A popover: combo-box list, colour picker, search suggestions.
    Popover,
    /// A help tag.
    Tooltip,
    /// An inline completion list.
    Completion,
}

impl TransientKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            TransientKind::Menu => "menu",
            TransientKind::Submenu => "submenu",
            TransientKind::Popover => "popover",
            TransientKind::Tooltip => "tooltip",
            TransientKind::Completion => "completion",
        }
    }

    #[must_use]
    pub fn default_policy(self) -> DismissPolicy {
        match self {
            TransientKind::Menu | TransientKind::Submenu => DismissPolicy::menu(),
            TransientKind::Popover | TransientKind::Completion => DismissPolicy::popover(),
            TransientKind::Tooltip => DismissPolicy::tooltip(),
        }
    }
}

/// Why a surface can close. Each flag maps to one input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DismissPolicy {
    pub outside_click: bool,
    pub escape: bool,
    pub window_deactivation: bool,
    pub scroll: bool,
    pub focus_loss: bool,
}

impl DismissPolicy {
    /// A menu closes on any outside click, Escape, deactivation or scroll.
    #[must_use]
    pub const fn menu() -> Self {
        Self {
            outside_click: true,
            escape: true,
            window_deactivation: true,
            scroll: true,
            focus_loss: false,
        }
    }

    /// A popover or completion list has the menu policy.
    #[must_use]
    pub const fn popover() -> Self {
        Self::menu()
    }

    /// A tooltip disappears when the pointer leaves, the window deactivates or
    /// the owner scrolls, but not because of a click elsewhere or Escape: the
    /// click belongs to the control under the pointer.
    #[must_use]
    pub const fn tooltip() -> Self {
        Self {
            outside_click: false,
            escape: false,
            window_deactivation: true,
            scroll: false,
            focus_loss: true,
        }
    }

    #[must_use]
    pub const fn allows(self, trigger: DismissTrigger) -> bool {
        match trigger {
            DismissTrigger::OutsideClick => self.outside_click,
            DismissTrigger::Escape => self.escape,
            DismissTrigger::WindowDeactivation => self.window_deactivation,
            DismissTrigger::Scroll => self.scroll,
            DismissTrigger::FocusLoss => self.focus_loss,
        }
    }
}

impl Default for DismissPolicy {
    fn default() -> Self {
        Self::menu()
    }
}

/// The event that asks a surface to close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissTrigger {
    OutsideClick,
    Escape,
    WindowDeactivation,
    Scroll,
    FocusLoss,
}

/// What to open. Build it with [`SurfaceSpec::new`] and the chained setters.
#[derive(Clone, Debug)]
pub struct SurfaceSpec {
    kind: TransientKind,
    policy: DismissPolicy,
    anchor: Option<Bounds<Pixels>>,
    owner: Option<ResponderScope>,
    title: Option<SharedString>,
}

impl SurfaceSpec {
    /// A surface of `kind` with that kind's default policy.
    #[must_use]
    pub fn new(kind: TransientKind) -> Self {
        Self {
            kind,
            policy: kind.default_policy(),
            anchor: None,
            owner: None,
            title: None,
        }
    }

    #[must_use]
    pub fn policy(mut self, policy: DismissPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn anchor(mut self, anchor: Bounds<Pixels>) -> Self {
        self.anchor = Some(anchor);
        self
    }

    #[must_use]
    pub fn owner(mut self, owner: ResponderScope) -> Self {
        self.owner = Some(owner);
        self
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// A handle to one open surface. Dropping it does not close the surface; call
/// [`TransientLayer::close`] for that. The token is what async content keeps.
#[derive(Clone, Debug)]
pub struct TransientHandle {
    id: SurfaceId,
    kind: TransientKind,
    cancel: CancelToken,
}

impl TransientHandle {
    #[must_use]
    pub fn id(&self) -> SurfaceId {
        self.id
    }

    #[must_use]
    pub fn kind(&self) -> TransientKind {
        self.kind
    }

    /// Clone this into a spawned task. Check it before applying the result.
    #[must_use]
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }
}

/// Identifies a surface within one layer. Stable for the surface's lifetime.
pub type SurfaceId = u64;

/// Cancellation state for async population. The layer cancels it when the
/// surface closes; the content checks it before touching UI or state.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Rc<Cell<bool>>);

impl CancelToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.get()
    }

    pub fn cancel(&self) {
        self.0.set(true);
    }
}

struct Surface {
    id: SurfaceId,
    kind: TransientKind,
    policy: DismissPolicy,
    anchor: Option<Bounds<Pixels>>,
    owner: Option<ResponderScope>,
    title: Option<SharedString>,
    cancel: CancelToken,
}

impl Surface {
    fn handle(&self) -> TransientHandle {
        TransientHandle {
            id: self.id,
            kind: self.kind,
            cancel: self.cancel.clone(),
        }
    }
}

/// One window's stack of transient surfaces.
pub struct TransientLayer {
    stack: Vec<Surface>,
    next_id: SurfaceId,
    chain: Option<Entity<ResponderChain>>,
    focus: FocusReturn,
}

impl Default for TransientLayer {
    fn default() -> Self {
        Self {
            stack: Vec::new(),
            next_id: 1,
            chain: None,
            focus: FocusReturn::new(),
        }
    }
}

impl TransientLayer {
    /// Creates a layer that dismisses deactivation-only surfaces when the
    /// window becomes inactive.
    #[must_use]
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let layer = Self::default();
        cx.observe_window_activation(window, |layer, window, cx| {
            if !window.is_window_active() {
                layer.dismiss_all(DismissTrigger::WindowDeactivation, window, cx, false);
            }
        })
        .detach();
        layer
    }

    /// Share the window's responder chain so opening the first surface freezes
    /// the command target and closing the last one restores it.
    pub fn set_chain(&mut self, chain: Entity<ResponderChain>) {
        self.chain = Some(chain);
    }

    /// Push a surface. The caller moves focus into its panel afterwards; the
    /// layer records the focus to return to.
    pub fn push(
        &mut self,
        spec: SurfaceSpec,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> TransientHandle {
        if self.stack.is_empty() {
            self.focus.remember(window, cx);
            if let Some(chain) = self.chain.clone() {
                chain.update(cx, |chain, cx| chain.begin_transient(window, cx));
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        let surface = Surface {
            id,
            kind: spec.kind,
            policy: spec.policy,
            anchor: spec.anchor,
            owner: spec.owner,
            title: spec.title,
            cancel: CancelToken::new(),
        };
        let handle = surface.handle();
        self.stack.push(surface);
        cx.notify();
        handle
    }

    /// Close one surface by id. A surface that is not open is a no-op, which
    /// makes a late async completion harmless. `restore` returns focus to the
    /// handle recorded when the stack opened.
    pub fn close(
        &mut self,
        id: SurfaceId,
        restore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = self.stack.iter().position(|surface| surface.id == id) else {
            return false;
        };
        let surface = self.stack.remove(index);
        surface.cancel.cancel();
        if self.stack.is_empty() {
            if let Some(chain) = self.chain.clone() {
                chain.update(cx, |chain, _| chain.end_transient());
            }
            if restore {
                if !self.focus.restore(window, cx) {
                    window.blur(cx);
                }
            } else {
                self.focus.forget();
            }
        }
        cx.notify();
        true
    }

    /// Close the innermost surface.
    pub fn close_top(
        &mut self,
        restore: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(top) = self.stack.last().map(|surface| surface.id) else {
            return false;
        };
        self.close(top, restore, window, cx)
    }

    /// Escape: close the innermost surface if its policy allows Escape, else
    /// leave the event for the sheet layer (#21) and then the application.
    pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.dismiss_top(DismissTrigger::Escape, window, cx, true)
    }

    /// Apply one dismissal trigger to the innermost surface. Returns whether a
    /// surface closed, so a shield handler can stop the event.
    pub fn dismiss_top(
        &mut self,
        trigger: DismissTrigger,
        window: &mut Window,
        cx: &mut Context<Self>,
        restore: bool,
    ) -> bool {
        let Some(top) = self.stack.last() else {
            return false;
        };
        if !top.policy.allows(trigger) {
            return false;
        }
        let id = top.id;
        self.close(id, restore, window, cx)
    }

    /// Apply a trigger to every surface whose policy allows it. Window
    /// deactivation uses this so a stack of menus closes as one.
    pub fn dismiss_all(
        &mut self,
        trigger: DismissTrigger,
        window: &mut Window,
        cx: &mut Context<Self>,
        restore: bool,
    ) -> usize {
        let ids: Vec<SurfaceId> = self
            .stack
            .iter()
            .filter(|surface| surface.policy.allows(trigger))
            .map(|surface| surface.id)
            .collect();
        let count = ids.len();
        for id in ids {
            self.close(id, restore, window, cx);
        }
        count
    }

    /// Dismiss a specific surface only if it is the innermost one. A shield
    /// from an older surface cannot close the newer surface above it.
    pub fn dismiss_surface(
        &mut self,
        id: SurfaceId,
        trigger: DismissTrigger,
        window: &mut Window,
        cx: &mut Context<Self>,
        restore: bool,
    ) -> bool {
        if self.stack.last().map(|surface| surface.id) != Some(id) {
            return false;
        }
        self.dismiss_top(trigger, window, cx, restore)
    }

    #[must_use]
    pub fn is_open(&self, id: SurfaceId) -> bool {
        self.stack.iter().any(|surface| surface.id == id)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    #[must_use]
    pub fn top_kind(&self) -> Option<TransientKind> {
        self.stack.last().map(|surface| surface.kind)
    }

    #[must_use]
    pub fn top_id(&self) -> Option<SurfaceId> {
        self.stack.last().map(|surface| surface.id)
    }

    #[must_use]
    pub fn top_anchor(&self) -> Option<Bounds<Pixels>> {
        self.stack.last().and_then(|surface| surface.anchor)
    }

    #[must_use]
    pub fn top_owner(&self) -> Option<ResponderScope> {
        self.stack.last().and_then(|surface| surface.owner)
    }

    #[must_use]
    pub fn top_title(&self) -> Option<&SharedString> {
        self.stack.last().and_then(|surface| surface.title.as_ref())
    }

    /// A tooltip must not appear over an open menu, submenu, popover or
    /// completion list.
    #[must_use]
    pub fn allows_tooltip(&self) -> bool {
        !self.stack.iter().any(|surface| {
            matches!(
                surface.kind,
                TransientKind::Menu
                    | TransientKind::Submenu
                    | TransientKind::Popover
                    | TransientKind::Completion
            )
        })
    }

    /// Drop every surface without restoring focus, for a window that is
    /// closing.
    pub fn clear(&mut self) {
        for surface in self.stack.drain(..) {
            surface.cancel.cancel();
        }
        self.focus.forget();
    }
}

/// Attach the layer's outside-click dismissal to a surface's shield. The
/// surface owner still draws the shield; the policy and stack live here.
#[must_use]
pub fn dismiss_on_pointer(
    element: gpui::Div,
    layer: Entity<TransientLayer>,
    surface: SurfaceId,
) -> gpui::Div {
    let left = layer.clone();
    let right = layer.clone();
    element
        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
            cx.stop_propagation();
            left.update(cx, |layer, cx| {
                layer.dismiss_surface(surface, DismissTrigger::OutsideClick, window, cx, true);
            });
        })
        .on_mouse_down(MouseButton::Right, move |_, window, cx| {
            cx.stop_propagation();
            right.update(cx, |layer, cx| {
                layer.dismiss_surface(surface, DismissTrigger::OutsideClick, window, cx, true);
            });
        })
}

/// Attach the layer's scroll dismissal to a surface's shield.
#[must_use]
pub fn dismiss_on_scroll(
    element: gpui::Div,
    layer: Entity<TransientLayer>,
    surface: SurfaceId,
) -> gpui::Div {
    element.on_scroll(move |_, window, cx| {
        cx.stop_propagation();
        layer.update(cx, |layer, cx| {
            layer.dismiss_surface(surface, DismissTrigger::Scroll, window, cx, true);
        });
    })
}

/// A full-window shield for a surface. Place it in the deferred overlay under
/// the panel. It blocks pointer events and applies outside-click and scroll
/// dismissal through the layer.
#[must_use]
pub fn shield(
    layer: Entity<TransientLayer>,
    surface: SurfaceId,
    window: &Window,
    offset: gpui::Point<Pixels>,
) -> gpui::Stateful<gpui::Div> {
    let element = div()
        .absolute()
        .top(offset.y)
        .left(offset.x)
        .w(window.viewport_size().width)
        .h(window.viewport_size().height)
        .occlude();
    let element = dismiss_on_pointer(element, layer.clone(), surface);
    dismiss_on_scroll(element, layer, surface)
        .id(("transient-shield", surface))
        .debug_selector(move || format!("transient-shield-{surface}"))
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{Bounds, Context, Render, TestAppContext, Window};

    use super::*;
    use crate::responder::{ResponderChain, ResponderScope};

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

    #[test]
    fn escape_closes_the_innermost_surface_first() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        app.update(|cx| {
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    let layer_entity = layer.clone();
                    layer_entity.update(cx, |layer, cx| {
                        let menu = layer.push(
                            SurfaceSpec::new(TransientKind::Menu)
                                .owner(ResponderScope::Window)
                                .title("View"),
                            window,
                            cx,
                        );
                        let submenu = layer.push(
                            SurfaceSpec::new(TransientKind::Submenu)
                                .owner(ResponderScope::Window)
                                .title("Appearance"),
                            window,
                            cx,
                        );
                        assert!(layer.is_open(menu.id()));
                        assert!(layer.is_open(submenu.id()));
                        assert_eq!(layer.len(), 2);

                        assert!(layer.escape(window, cx), "closes the submenu");
                        assert!(!layer.is_open(submenu.id()));
                        assert!(layer.is_open(menu.id()));
                        assert_eq!(layer.top_kind(), Some(TransientKind::Menu));

                        assert!(layer.escape(window, cx), "closes the menu");
                        assert!(layer.is_empty());
                        assert!(!layer.escape(window, cx), "the sheet or app is next");
                    });
                })
                .unwrap();
        });
    }

    #[test]
    fn closing_a_surface_cancels_its_token() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let applied = Rc::new(Cell::new(false));
        let guard = applied.clone();
        app.update(|cx| {
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    layer.update(cx, |layer, cx| {
                        let handle = layer.push(
                            SurfaceSpec::new(TransientKind::Popover).title("search"),
                            window,
                            cx,
                        );
                        let token = handle.cancel_token();
                        assert!(!token.is_cancelled());
                        layer.close(handle.id(), false, window, cx);
                        assert!(token.is_cancelled());
                        // The async completion is a guard like a spawned task:
                        // it must not touch state after the surface closes.
                        if !token.is_cancelled() {
                            guard.set(true);
                        }
                    });
                })
                .unwrap();
        });
        assert!(!applied.get(), "closed surface must not apply its result");
    }

    #[test]
    fn a_late_close_is_a_no_op() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        app.update(|cx| {
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    layer.update(cx, |layer, cx| {
                        let handle = layer.push(SurfaceSpec::new(TransientKind::Menu), window, cx);
                        assert!(layer.close(handle.id(), false, window, cx));
                        assert!(!layer.close(handle.id(), false, window, cx));
                    });
                })
                .unwrap();
        });
    }

    #[test]
    fn a_tooltip_ignores_outside_clicks_and_escape_but_closes_on_deactivation() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        app.update(|cx| {
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    layer.update(cx, |layer, cx| {
                        let tooltip =
                            layer.push(SurfaceSpec::new(TransientKind::Tooltip), window, cx);
                        assert!(!layer.escape(window, cx));
                        assert!(!layer.dismiss_top(DismissTrigger::OutsideClick, window, cx, true));
                        assert!(layer.is_open(tooltip.id()));
                        assert_eq!(
                            layer.dismiss_all(
                                DismissTrigger::WindowDeactivation,
                                window,
                                cx,
                                false
                            ),
                            1
                        );
                        assert!(layer.is_empty());
                    });
                })
                .unwrap();
        });
    }

    #[test]
    fn tooltips_are_suppressed_while_a_menu_is_open() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        app.update(|cx| {
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    layer.update(cx, |layer, cx| {
                        assert!(layer.allows_tooltip());
                        let menu = layer.push(SurfaceSpec::new(TransientKind::Menu), window, cx);
                        assert!(!layer.allows_tooltip());
                        layer.close(menu.id(), false, window, cx);
                        assert!(layer.allows_tooltip());
                    });
                })
                .unwrap();
        });
    }

    #[test]
    fn opening_the_first_surface_freezes_the_responder_chain() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        app.update(|cx| {
            let chain = cx.new(|_| ResponderChain::new());
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    layer.update(cx, |layer, cx| {
                        layer.set_chain(chain.clone());
                        let first = layer.push(SurfaceSpec::new(TransientKind::Menu), window, cx);
                        assert!(chain.read(cx).is_transient());
                        let second =
                            layer.push(SurfaceSpec::new(TransientKind::Submenu), window, cx);
                        layer.close(second.id(), false, window, cx);
                        assert!(chain.read(cx).is_transient());
                        layer.close(first.id(), false, window, cx);
                        assert!(!chain.read(cx).is_transient());
                    });
                })
                .unwrap();
        });
    }

    #[test]
    fn only_the_top_surface_can_be_dismissed_by_its_shield() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        app.update(|cx| {
            let layer = cx.new(|_| TransientLayer::default());
            window
                .update(cx, |_, window, cx| {
                    layer.update(cx, |layer, cx| {
                        let older = layer.push(SurfaceSpec::new(TransientKind::Menu), window, cx);
                        let newer =
                            layer.push(SurfaceSpec::new(TransientKind::Popover), window, cx);
                        assert!(!layer.dismiss_surface(
                            older.id(),
                            DismissTrigger::OutsideClick,
                            window,
                            cx,
                            true
                        ));
                        assert!(layer.is_open(older.id()));
                        assert!(layer.dismiss_surface(
                            newer.id(),
                            DismissTrigger::OutsideClick,
                            window,
                            cx,
                            true
                        ));
                    });
                })
                .unwrap();
        });
    }

    #[test]
    fn the_layer_keeps_the_anchor_owner_and_title_for_diagnostics() {
        let mut layer = TransientLayer::default();
        // Pure state assertions do not need a window.
        layer.stack.push(Surface {
            id: 7,
            kind: TransientKind::Menu,
            policy: DismissPolicy::menu(),
            anchor: Some(Bounds {
                origin: gpui::point(gpui::px(1.), gpui::px(2.)),
                size: gpui::size(gpui::px(3.), gpui::px(4.)),
            }),
            owner: Some(ResponderScope::Window),
            title: Some("View".into()),
            cancel: CancelToken::new(),
        });
        assert_eq!(layer.top_id(), Some(7));
        assert_eq!(layer.top_owner(), Some(ResponderScope::Window));
        assert_eq!(layer.top_title().map(|t| t.as_ref()), Some("View"));
        assert_eq!(layer.top_anchor().map(|b| b.size.width), Some(gpui::px(3.)));
    }
}
