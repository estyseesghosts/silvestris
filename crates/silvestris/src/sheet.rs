//! Sheet presentation (#22): a document-owned surface attached under the
//! window band (title bar plus toolbar).
//!
//! `modal.rs` owns the policy (blocked commands, focus stops, result
//! delivery). This module owns where the content is drawn. A sheet is an
//! overlay inside the owning window, flush with the bottom edge of the band:
//! no scrim, no second window, square top corners, side and bottom border and
//! a shadow below and to the sides. The parent keeps its active appearance.
//!
//! The window root must call [`overlay`] from inside its content area.
//! `chrome::WindowFrame` already does.
//!
//! One sheet may be open for a window at a time (HIG p. 236). A second
//! [`present`] returns `false`; it does not queue and does not stack. Alerts
//! that must appear over a Save sheet are application-modal windows (p. 251).
//!
//! The sheet grows the owner's content area when it does not fit and restores
//! the exact previous size when it closes. GPUI exposes no window move, so the
//! growth keeps the origin and is clamped to the display; `docs/exceptions.md`
//! records that adaptation. The emerge and retract timings are estimates: no
//! 10.6 timing capture exists (`specs/sheet.md` §5 and §10).

use std::cell::Cell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    div, prelude::*, px, size, AnyElement, AnyWindowHandle, App, Context, Entity, Global, Pixels,
    Render, SharedString, Size, Task, Window, WindowId,
};
use gpui::{Animation, AnimationExt, BoxShadow, Hsla};

use crate::convert::hsla;
use crate::modal::{self, ModalResult, ModalSession};
use crate::ActiveTheme;

type Content = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// Estimate: no 10.6 timing capture exists. The sheet slides down from behind
/// the band over 200 ms and retracts in 160 ms, both with an ease-out curve.
/// Logged in `docs/exceptions.md`.
pub const EMERGE: Duration = Duration::from_millis(200);
pub const RETRACT: Duration = Duration::from_millis(160);

/// Where a sheet is in its own enter/leave animation. Modal setup starts at
/// the beginning of the emerge; the modal result is delivered after the
/// retract, or immediately when the owner window closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Emerging,
    Active,
    Retracting,
}

/// One sheet's layout and content. Create it with [`Sheet::new`] or
/// [`Sheet::new_sized`], then present it with [`present`].
pub struct Sheet {
    id: u64,
    title: SharedString,
    width: f32,
    height: Option<f32>,
    content: Content,
    session: Entity<ModalSession>,
    phase: Phase,
    /// Measured height of the card, used to slide it fully out of view.
    measured_height: Rc<Cell<f32>>,
    /// Viewport size before the sheet grew the window, once per sheet.
    original_viewport: Rc<Cell<Option<Size<Pixels>>>>,
    /// The last size this sheet asked the window for. Prevents a resize loop.
    grown_to: Rc<Cell<Option<Size<Pixels>>>>,
    task: Option<Task<()>>,
}

impl Sheet {
    /// A content-height sheet with the supplied width.
    pub fn new(
        title: impl Into<SharedString>,
        width: f32,
        session: Entity<ModalSession>,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::build(title, width, None, session, content, cx)
    }

    /// A sheet whose height is known before layout (for example a file panel).
    pub fn new_sized(
        title: impl Into<SharedString>,
        width: f32,
        height: f32,
        session: Entity<ModalSession>,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        Self::build(title, width, Some(height), session, content, cx)
    }

    fn build(
        title: impl Into<SharedString>,
        width: f32,
        height: Option<f32>,
        session: Entity<ModalSession>,
        content: impl Fn(&mut Window, &mut App) -> AnyElement + 'static,
        cx: &mut App,
    ) -> Entity<Self> {
        let title = title.into();
        cx.new(|cx| Self {
            id: cx.entity_id().as_u64(),
            title,
            width,
            height,
            content: Rc::new(content),
            session,
            phase: Phase::Emerging,
            measured_height: Rc::new(Cell::new(height.unwrap_or(0.0))),
            original_viewport: Rc::new(Cell::new(None)),
            grown_to: Rc::new(Cell::new(None)),
            task: None,
        })
    }

    #[must_use]
    pub fn session(&self) -> &Entity<ModalSession> {
        &self.session
    }

    #[must_use]
    pub fn phase(&self) -> &'static str {
        match self.phase {
            Phase::Emerging => "emerging",
            Phase::Active => "active",
            Phase::Retracting => "retracting",
        }
    }
}

#[derive(Default)]
struct Registry {
    /// At most one live sheet per window (HIG p. 236).
    sheets: BTreeMap<WindowId, Entity<Sheet>>,
}

impl Global for Registry {}

fn registry(cx: &mut App) -> &mut Registry {
    if cx.try_global::<Registry>().is_none() {
        cx.set_global(Registry::default());
    }
    cx.global_mut::<Registry>()
}

fn live(cx: &App, sheet: &Entity<Sheet>) -> bool {
    !sheet.read(cx).session.read(cx).is_ended()
}

/// Show `sheet` in `owner`. Returns `false` when that window already has a
/// live sheet; the caller must close the first one before presenting another
/// (HIG p. 236: "Only one sheet may be open for a window at any one time").
pub fn present(owner: AnyWindowHandle, sheet: Entity<Sheet>, cx: &mut App) -> bool {
    let id = owner.window_id();
    let existing = registry(cx).sheets.get(&id).cloned();
    if existing.as_ref().is_some_and(|existing| live(cx, existing)) {
        return false;
    }
    registry(cx).sheets.insert(id, sheet.clone());
    let session = sheet.read(cx).session.clone();
    let animate = !cx.reduce_motion();
    let sheet_for_timer = sheet.clone();
    sheet.update(cx, |sheet, cx| {
        sheet.phase = if animate {
            Phase::Emerging
        } else {
            Phase::Active
        };
        if animate {
            sheet.task = Some(cx.spawn(async move |this, cx| {
                cx.background_executor().timer(EMERGE).await;
                let _ = this.update(cx, |sheet, cx| {
                    if sheet.phase == Phase::Emerging {
                        sheet.phase = Phase::Active;
                        cx.notify();
                    }
                });
            }));
        }
        cx.notify();
    });
    // Presentation can be called from inside a window update (a command
    // handler). Re-entering that window's update would fail, so modal setup
    // runs as a deferred effect; the sheet is registered now and blocks input
    // from the first frame.
    cx.defer(move |cx| {
        let session = session.clone();
        let _ = owner.update(cx, |_, window, cx| {
            modal::present(session, window, cx);
            window.refresh();
        });
        let _ = &sheet_for_timer;
    });
    true
}

/// Re-render a window's sheet after its own state changed.
pub fn refresh(owner: AnyWindowHandle, cx: &mut App) {
    let _ = owner.update(cx, |_, window, _| window.refresh());
}

/// Remove every sheet owned by `owner` without touching the modal sessions;
/// `modal::cancel_owned` delivers the cancellations.
pub fn close_owner(id: WindowId, cx: &mut App) {
    registry(cx).sheets.remove(&id);
}

/// Remove the sheet that owns `session` immediately. Alerts use this when a
/// window is torn down rather than retracted.
pub fn close_session(session: &Entity<ModalSession>, cx: &mut App) {
    let found = cx.try_global::<Registry>().and_then(|registry| {
        registry
            .sheets
            .iter()
            .find(|(_, sheet)| sheet.read(cx).session == *session)
            .map(|(id, _)| *id)
    });
    if let Some(id) = found {
        registry(cx).sheets.remove(&id);
    }
}

/// Retract `session`'s sheet, then deliver `result` after the animation. The
/// modal session stays active while the sheet slides out, so input remains
/// blocked until it has gone (HIG p. 236). Returns false when `session` is not
/// the sheet's session.
pub fn dismiss(
    session: Entity<ModalSession>,
    result: ModalResult,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let Some((id, sheet)) = find_sheet(&session, cx) else {
        return false;
    };
    if sheet.read(cx).phase == Phase::Retracting {
        return true;
    }
    if cx.reduce_motion() {
        finish(id, sheet, session, result, window, cx);
        return true;
    }
    let owner = session.read(cx).owner();
    sheet.update(cx, |sheet, cx| {
        sheet.phase = Phase::Retracting;
        cx.notify();
    });
    let Some(owner) = owner else {
        finish(id, sheet, session, result, window, cx);
        return true;
    };
    let sheet_for_task = sheet.clone();
    let session_for_task = session.clone();
    let task = sheet.update(cx, |_, cx| {
        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(RETRACT).await;
            cx.update(|cx| {
                let _ = owner.update(cx, |_, window, cx| {
                    finish(id, sheet_for_task, session_for_task, result, window, cx);
                });
            });
        })
    });
    sheet.update(cx, |sheet, _| sheet.task = Some(task));
    true
}

fn find_sheet(session: &Entity<ModalSession>, cx: &App) -> Option<(WindowId, Entity<Sheet>)> {
    let registry = cx.try_global::<Registry>()?;
    registry
        .sheets
        .iter()
        .find(|(_, sheet)| sheet.read(cx).session == *session)
        .map(|(id, sheet)| (*id, sheet.clone()))
}

fn finish(
    id: WindowId,
    sheet: Entity<Sheet>,
    session: Entity<ModalSession>,
    result: ModalResult,
    window: &mut Window,
    cx: &mut App,
) {
    let original = sheet.read(cx).original_viewport.get();
    if let Some(original) = original {
        window.resize(original);
    }
    if let Some(registry) = cx.try_global::<Registry>() {
        if registry
            .sheets
            .get(&id)
            .is_some_and(|current| *current == sheet)
        {
            cx.global_mut::<Registry>().sheets.remove(&id);
        }
    }
    modal::finish(session, result, window, cx);
}

/// The overlay for `window`, or `None` when no sheet is live. Called by
/// `WindowFrame`; a custom window root draws the same element at the top of
/// its content area.
pub fn overlay(window: &Window, cx: &mut App) -> Option<AnyElement> {
    let id = window.window_handle().window_id();
    let sheet = {
        let registry = cx.try_global::<Registry>()?;
        registry.sheets.get(&id).cloned()?
    };
    if sheet.read(cx).session.read(cx).is_ended() {
        registry(cx).sheets.remove(&id);
        return None;
    }
    Some(sheet.into_any_element())
}

/// The sheet's chrome without content: square top corners, side and bottom
/// border, window fill and a shadow below/sides. Shared by live presentation
/// and the golden capture so the two cannot drift.
#[must_use]
pub fn card_shell(width: f32, height: Option<f32>, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    let radius = theme.metrics.panel_radius;
    div()
        .flex()
        .flex_none()
        .flex_col()
        .w(px(width))
        .when_some(height, |el, height| el.h(px(height)))
        .rounded_t(px(0.))
        .rounded_b(px(radius))
        .border_1()
        .border_t_0()
        .border_color(hsla(theme.colors.group_border))
        .bg(hsla(theme.colors.window))
        // Estimate (docs/exceptions.md): the capture shows a short, soft
        // shadow at the sides and below; no top shadow.
        .shadow(vec![BoxShadow::new(
            px(0.),
            px(5.),
            Hsla {
                h: 0.,
                s: 0.,
                l: 0.,
                a: 0.22,
            },
        )
        .blur_radius(px(12.))])
}

impl Render for Sheet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.session.read(cx).is_ended() {
            return div().id("sheet-gone").into_any_element();
        }
        let content = (self.content)(window, cx);
        let title = self.title.clone();
        let width = self.width;
        let height = self.height;
        let measured = self.measured_height.clone();
        let original = self.original_viewport.clone();
        let grown_to = self.grown_to.clone();
        let session = self.session.clone();
        let phase = self.phase;
        let slide = {
            let measured = measured.get();
            if measured > 0.0 {
                measured
            } else {
                height.unwrap_or(320.0)
            }
        };
        let animate_id = format!("sheet-{}-{phase:?}", self.id);

        let card = card_shell(width, height, cx)
            .id("silvestris-sheet")
            .debug_selector(|| "silvestris-sheet".into())
            .role(gpui::Role::AlertDialog)
            .aria_label(title)
            .relative()
            .child(content)
            .child({
                let measured = measured.clone();
                let original = original.clone();
                let grown_to = grown_to.clone();
                gpui::canvas(
                    |_, _, _| (),
                    move |bounds, _, window, cx| {
                        measured.set(f32::from(bounds.size.height));
                        // The canvas is inside the 1 px border; the window must
                        // fit the border box.
                        grow_window(
                            size(bounds.size.width + px(2.), bounds.size.height + px(2.)),
                            &original,
                            &grown_to,
                            window,
                            cx,
                        );
                    },
                )
                .absolute()
                .inset_0()
            });

        let animated: AnyElement = match phase {
            Phase::Emerging => card
                .with_animation(
                    gpui::ElementId::Name(animate_id.into()),
                    Animation::new(EMERGE).with_easing(gpui::ease_out_quint()),
                    move |el, t| el.mt(px(-slide * (1.0 - t))),
                )
                .into_any_element(),
            Phase::Active => card.into_any_element(),
            Phase::Retracting => card
                .with_animation(
                    gpui::ElementId::Name(animate_id.into()),
                    Animation::new(RETRACT).with_easing(gpui::linear),
                    move |el, t| el.mt(px(-slide * t)).opacity(1.0 - t * 0.25),
                )
                .into_any_element(),
        };

        div()
            .id("silvestris-sheet-overlay")
            .debug_selector(|| "silvestris-sheet-overlay".into())
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .items_start()
            .overflow_hidden()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(gpui::MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    dismiss(session.clone(), ModalResult::Rejected, window, cx);
                }
            })
            .child(animated)
            .into_any_element()
    }
}

/// Grow the owner to hold the sheet, keeping the origin and staying inside
/// the display. GPUI exposes resize but no reposition, so this is the
/// in-window adaptation of HIG p. 236's "move the window away from the edge".
fn grow_window(
    sheet: Size<Pixels>,
    original: &Rc<Cell<Option<Size<Pixels>>>>,
    grown_to: &Rc<Cell<Option<Size<Pixels>>>>,
    window: &mut Window,
    cx: &mut App,
) {
    let viewport = window.viewport_size();
    let needs_grow = sheet.width > viewport.width || sheet.height > viewport.height;
    if !needs_grow {
        return;
    }
    // Room for the side/bottom shadow so a grown window does not clip it.
    const SHADOW_MARGIN: f32 = 12.0;
    let mut target = size(
        (sheet.width + px(SHADOW_MARGIN * 2.)).max(viewport.width),
        (sheet.height + px(SHADOW_MARGIN)).max(viewport.height),
    );
    if let Some(display) = window.display(cx) {
        let visible = display.visible_bounds();
        let bounds = window.bounds();
        let room_w =
            viewport.width + (visible.origin.x + visible.size.width - bounds.right()).max(px(0.));
        let room_h = viewport.height
            + (visible.origin.y + visible.size.height - bounds.bottom()).max(px(0.));
        target.width = target.width.min(room_w.max(px(320.)));
        target.height = target.height.min(room_h.max(px(200.)));
    }
    if grown_to.get() == Some(target) || target == viewport {
        return;
    }
    if original.get().is_none() {
        original.set(Some(viewport));
    }
    grown_to.set(Some(target));
    window.resize(target);
}

/// Finish an active session as rejected and retract its sheet.
pub fn reject(session: Entity<ModalSession>, window: &mut Window, cx: &mut App) {
    if !dismiss(session.clone(), ModalResult::Rejected, window, cx) {
        modal::escape_session(session, window, cx);
    }
}

/// The sheet currently covering `window`, if any. Tests use it to assert the
/// one-sheet rule; the overlay uses the same registry.
#[doc(hidden)]
pub fn active_sessions(window: &Window, cx: &App) -> Vec<Entity<ModalSession>> {
    let id = window.window_handle().window_id();
    let Some(registry) = cx.try_global::<Registry>() else {
        return Vec::new();
    };
    registry
        .sheets
        .get(&id)
        .map(|sheet| sheet.read(cx).session.clone())
        .filter(|session| !session.read(cx).is_ended())
        .into_iter()
        .collect()
}

/// The sheet's phase for tests and diagnostics.
#[doc(hidden)]
pub fn phase_for(session: &Entity<ModalSession>, cx: &App) -> Option<&'static str> {
    let (_, sheet) = find_sheet(session, cx)?;
    Some(sheet.read(cx).phase())
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
