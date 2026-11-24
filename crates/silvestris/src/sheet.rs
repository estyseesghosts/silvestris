//! Sheet presentation (#22): a document-owned surface attached under the
//! title bar.
//!
//! `modal.rs` owns the policy (blocked commands, focus stops, result
//! delivery). This module owns where the content is drawn: an overlay at the
//! top of the owning window's content area, centred, above a dimmed parent
//! (HIG p. 236). A sheet resizes with the window because it is laid out inside
//! it, never in a second window.
//!
//! The window root must call [`overlay`] from inside its content area.
//! `chrome::WindowFrame` already does. An application-modal surface uses a
//! role window instead (see [`present_window`]).
//!
//! The slide-in animation is deliberately not implemented. The only 10.6
//! evidence we have is still captures; a guessed duration would be an
//! exception with no measurement.

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, AnyWindowHandle, App, Entity, Global, Render, SharedString,
    Window, WindowId,
};

use crate::convert::hsla;
use crate::modal::{self, ModalPhase, ModalResult, ModalSession};
use crate::ActiveTheme;

type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One sheet's layout and content. Create it with [`Sheet::new`], then present
/// it with [`present`].
pub struct Sheet {
    title: SharedString,
    width: f32,
    content: Content,
    session: Entity<ModalSession>,
}

impl Sheet {
    pub fn new(
        title: impl Into<SharedString>,
        width: f32,
        session: Entity<ModalSession>,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        cx.new(|_| Self {
            title: title.into(),
            width,
            content: Rc::new(content),
            session,
        })
    }

    #[must_use]
    pub fn session(&self) -> &Entity<ModalSession> {
        &self.session
    }
}

#[derive(Default)]
struct Registry {
    /// Bottom sheet first, alert on top.
    sheets: BTreeMap<WindowId, Vec<Entity<Sheet>>>,
}

impl Global for Registry {}

fn registry(cx: &mut App) -> &mut Registry {
    if cx.try_global::<Registry>().is_none() {
        cx.set_global(Registry::default());
    }
    cx.global_mut::<Registry>()
}

/// Show `sheet` in `owner` and present its modal session. The sheet is
/// removed from the registry once the session ends.
pub fn present(owner: AnyWindowHandle, sheet: Entity<Sheet>, cx: &mut App) -> bool {
    let id = owner.window_id();
    let session = sheet.read(cx).session.clone();
    registry(cx)
        .sheets
        .entry(id)
        .or_default()
        .push(sheet.clone());
    // Presentation can be called from inside a window update (a command
    // handler). Re-entering that window's update would fail, so modal setup
    // runs as a deferred effect; the sheet is registered now.
    cx.defer(move |cx| {
        let session = session.clone();
        let _ = owner.update(cx, |_, window, cx| {
            modal::present(session, window, cx);
            window.refresh();
        });
    });
    true
}

/// Re-render a window's sheets after a sheet's own state changed.
pub fn refresh(owner: AnyWindowHandle, cx: &mut App) {
    let _ = owner.update(cx, |_, window, _| window.refresh());
}

/// Remove every sheet owned by `owner` without touching the modal sessions;
/// `modal::cancel_owned` delivers the cancellations.
pub fn close_owner(id: WindowId, cx: &mut App) {
    registry(cx).sheets.remove(&id);
}

/// Remove the sheet that owns `session`.
pub fn close_session(session: &Entity<ModalSession>, cx: &mut App) {
    let entries: Vec<(WindowId, Vec<Entity<Sheet>>)> = {
        let Some(registry) = cx.try_global::<Registry>() else {
            return;
        };
        registry
            .sheets
            .iter()
            .map(|(id, sheets)| (*id, sheets.clone()))
            .collect()
    };
    let mut updated = BTreeMap::new();
    for (id, sheets) in entries {
        let alive: Vec<Entity<Sheet>> = sheets
            .into_iter()
            .filter(|sheet| sheet.read(cx).session != *session)
            .collect();
        if !alive.is_empty() {
            updated.insert(id, alive);
        }
    }
    if cx.try_global::<Registry>().is_some() {
        cx.global_mut::<Registry>().sheets = updated;
    }
}

/// The overlay for `window`, or `None` when no active sheet exists. Called by
/// `WindowFrame`; a custom window root draws the same element at the top of
/// its content area.
pub fn overlay(window: &Window, cx: &mut App) -> Option<AnyElement> {
    let id = window.window_handle().window_id();
    let entries = cx
        .try_global::<Registry>()
        .and_then(|registry| registry.sheets.get(&id).cloned())
        .unwrap_or_default();

    if entries.is_empty() {
        return None;
    }
    let mut alive = Vec::new();
    let mut elements = Vec::new();
    for sheet in entries {
        let ended = sheet.read(cx).session.read(cx).is_ended();
        if ended {
            continue;
        }
        alive.push(sheet.clone());
        elements.push(sheet.into_any_element());
    }
    if cx.try_global::<Registry>().is_some() {
        let registry = cx.global_mut::<Registry>();
        if alive.is_empty() {
            registry.sheets.remove(&id);
        } else {
            registry.sheets.insert(id, alive);
        }
    }
    if elements.is_empty() {
        return None;
    }
    Some(
        div()
            .id("silvestris-sheets")
            .debug_selector(|| "silvestris-sheets".into())
            .absolute()
            .inset_0()
            .children(elements)
            .into_any_element(),
    )
}

impl Render for Sheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let phase = self.session.read(cx).phase();

        if !matches!(phase, ModalPhase::Active) {
            return div().id("sheet-hidden").into_any_element();
        }
        let theme = cx.theme().clone();
        let session = self.session.clone();
        let content = (self.content)(window, cx);
        let width = self.width;
        let title = self.title.clone();
        let radius = theme.metrics.panel_radius;
        div()
            .id("silvestris-sheet")
            .debug_selector(|| "silvestris-sheet".into())
            .role(gpui::Role::AlertDialog)
            .aria_label(title)
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .items_start()
            .bg(hsla(theme.colors.scrim))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    modal::escape_session(session.clone(), window, cx);
                }
            })
            .child(
                div()
                    .w(px(width))
                    .max_w_full()
                    .flex()
                    .flex_col()
                    .rounded_b(px(radius))
                    .border_1()
                    .border_t_0()
                    .border_color(hsla(theme.colors.group_border))
                    .bg(hsla(theme.colors.window))
                    .shadow_lg()
                    .child(content),
            )
            .into_any_element()
    }
}

/// Present `content` as an application-modal window instead of a sheet. The
/// caller owns the role window's policy; this is the fallback for an app with
/// no document window.
pub fn dialog_options(
    title: impl Into<SharedString>,
    width: f32,
    height: f32,
    cx: &App,
) -> gpui::WindowOptions {
    let bounds = gpui::Bounds::new(
        gpui::point(px(0.), px(0.)),
        gpui::size(px(width), px(height)),
    );
    crate::window_role::role_options(crate::window_role::WindowRole::Dialog, title, bounds, cx)
}

/// The sheet currently covering `window`, bottom first. Tests use it to assert
/// stacking; the overlay uses the same registry.
#[doc(hidden)]
pub fn active_sessions(window: &Window, cx: &App) -> Vec<Entity<ModalSession>> {
    let id = window.window_handle().window_id();
    let Some(registry) = cx.try_global::<Registry>() else {
        return Vec::new();
    };
    let Some(sheets) = registry.sheets.get(&id) else {
        return Vec::new();
    };
    sheets
        .iter()
        .map(|sheet| sheet.read(cx).session.clone())
        .filter(|session| !session.read(cx).is_ended())
        .collect()
}

/// Finish an active session as rejected and let the registry prune it.
pub fn reject(session: Entity<ModalSession>, window: &mut Window, cx: &mut App) {
    modal::finish(session, ModalResult::Rejected, window, cx);
}
