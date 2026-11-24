//! Push and textured buttons (HIG p. 265-269; handbook 12).
//!
//! - Height per control size: `Metrics::push_height` / `push_small_height` /
//!   `push_mini_height` (M: button.push.default, button.push.normal).
//! - Width = label + 2 × `Metrics::push_padding` (M: button.push.widths), or
//!   wider via `min_width` for equal-width groups (HIG PDF p. 359, "Similar
//!   controls have the same width").
//! - Label font: system, small system, mini system for regular, small, mini
//!   (HIG p. 269).
//! - A pointer press fades into the pressed bevel and holds it while the
//!   button is down; keyboard presses and reduced motion show it immediately
//!   (HIG p. 265; fade observed on 10.6). GPUI still decides whether the
//!   release activates the button. Successful activation eases back to the
//!   resting bevel when motion is enabled.
//! - The focus ring shows only after keyboard input (handbook 12 focus manager).
//! - Disabled buttons are not focusable and have no click handler. GPUI at the
//!   pin has no `aria_disabled`, so assistive tech is not told the state; this
//!   is a recorded gap (docs/exceptions.md).

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    canvas, div, prelude::*, px, Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId,
    FocusHandle, KeyDownEvent, MouseButton, MouseExitEvent, MouseMoveEvent, MouseUpEvent,
    SharedString, Window,
};
use silvestris_core::tokens::{Bevel, Theme};
use silvestris_core::{Gradient, Rgba8};

use crate::convert::hsla;
use crate::paint;
use crate::theme::ActiveTheme;

/// HIG control sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlSize {
    #[default]
    Regular,
    Small,
    Mini,
}

impl ControlSize {
    fn index(self) -> usize {
        match self {
            ControlSize::Regular => 0,
            ControlSize::Small => 1,
            ControlSize::Mini => 2,
        }
    }

    pub fn label_size(self, theme: &Theme) -> f32 {
        match self {
            ControlSize::Regular => theme.type_.size_regular,
            ControlSize::Small => theme.type_.size_small,
            ControlSize::Mini => theme.type_.size_mini,
        }
    }
}

/// Bezel family.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    /// Rounded push button for dialogs and content (HIG p. 265).
    #[default]
    Push,
    /// Textured rounded button for window-frame areas (toolbars, bottom bars).
    Textured,
    /// Square gradient control for actions adjoining a list (HIG pp. 273–276).
    Gradient,
    /// Square bevel button (Address Book/Network preferences).
    Bevel,
    /// Circular textured capsule control (HIG p. 261).
    Round,
    /// Inset/recessed control (HIG pp. 257–261).
    Recessed,
    /// Purple question-mark help button.
    Help,
}

type Icon = Box<dyn Fn(gpui::Rgba) -> AnyElement>;

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    label: SharedString,
    style: ButtonStyle,
    size: ControlSize,
    default: bool,
    cancel: bool,
    throb: bool,
    disabled: bool,
    selected: bool,
    min_width: Option<f32>,
    icon: Option<Icon>,
    tooltip: Option<SharedString>,
    destructive: bool,
    focus: Option<FocusHandle>,
    on_click: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Button {
            id: id.into(),
            label: label.into(),
            style: ButtonStyle::Push,
            size: ControlSize::Regular,
            default: false,
            cancel: false,
            throb: true,
            disabled: false,
            selected: false,
            min_width: None,
            icon: None,
            tooltip: None,
            destructive: false,
            focus: None,
            on_click: None,
        }
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// The default button of a dialog: drawn with `Controls::push_default`.
    /// Return-key routing belongs to the dialog that owns it (handbook 12).
    pub fn default_button(mut self, default: bool) -> Self {
        self.default = default;
        self
    }

    /// Escape / Cmd-period key equivalent (dialogs).
    pub fn cancel_button(mut self, cancel: bool) -> Self {
        self.cancel = cancel;
        self
    }

    /// Disable the default-button throb (goldens and still captures).
    pub fn throb(mut self, throb: bool) -> Self {
        self.throb = throb;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Toggled state for textured buttons (the `selected` bevel).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Minimum width in logical pixels, for equal-width button groups.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Replace the visible label with an icon. The label names the control
    /// for accessibility. The icon receives the current state's ink colour.
    pub fn icon<E: IntoElement>(mut self, icon: impl Fn(gpui::Rgba) -> E + 'static) -> Self {
        self.icon = Some(Box::new(move |ink| icon(ink).into_any_element()));
        self
    }

    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }

    pub fn destructive(mut self, destructive: bool) -> Self {
        self.destructive = destructive;
        self
    }

    /// Supply a responder handle when a parent owns focus order.
    pub fn focus_handle(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

// Local timing estimates. A pointer press fades into the pressed bevel and
// back out on release; the command still runs on release (HIG p. 265).
// The fades were observed on a 10.6 VM; no frame timing was captured, so the
// durations are recorded in docs/exceptions.md.
const PRESS_DURATION: Duration = Duration::from_millis(90);
const RELEASE_DURATION: Duration = Duration::from_millis(120);

#[derive(Clone, Copy, Default, PartialEq)]
enum Press {
    #[default]
    Resting,
    Pointer {
        inside: bool,
        since: Instant,
    },
    Keyboard,
    /// Fading out from `from` (the amount shown when the press ended).
    Released {
        start: Instant,
        from: f32,
    },
}

fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

impl Press {
    fn progress(start: Instant, duration: Duration, window: &Window, cx: &App) -> f32 {
        let elapsed = cx
            .background_executor()
            .now()
            .saturating_duration_since(start);
        let t = (elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0);
        if t < 1.0 {
            window.request_animation_frame();
        }
        smoothstep(t)
    }

    fn amount(self, window: &Window, cx: &App) -> f32 {
        match self {
            Self::Keyboard => 1.0,
            Self::Pointer {
                inside: true,
                since,
            } => {
                if cx.reduce_motion() {
                    1.0
                } else {
                    Self::progress(since, PRESS_DURATION, window, cx)
                }
            }
            Self::Released { start, from } if !cx.reduce_motion() => {
                from * (1.0 - Self::progress(start, RELEASE_DURATION, window, cx))
            }
            _ => 0.0,
        }
    }
}

fn blend_bevel(resting: Bevel, pressed: Bevel, amount: f32) -> Bevel {
    let blend = |a: Gradient, b: Gradient| {
        Gradient::new(a.from.mix(b.from, amount), a.to.mix(b.to, amount))
    };
    Bevel {
        border: blend(resting.border, pressed.border),
        upper: blend(resting.upper, pressed.upper),
        lower: blend(resting.lower, pressed.lower),
        split: resting.split + (pressed.split - resting.split) * amount,
        emboss: resting.emboss.mix(pressed.emboss, amount),
        text: resting.text.mix(pressed.text, amount),
    }
}

/// Default-button throb. The measured default bevel is the midpoint; the
/// gloss breathes either side of it, lighter and less saturated, then deeper
/// and more saturated (observed on a 10.6 VM: Mail's Welcome assistant,
/// Continue). The swing is small, about 8% toward white and 8% in
/// saturation and depth. Period and amplitude are estimates recorded in
/// docs/exceptions.md.
fn throb_bevel(resting: Bevel, phase: f32) -> Bevel {
    const SWING: f32 = 0.08;
    let wave = (phase * std::f32::consts::TAU).sin();
    let white = Rgba8::hex(0xffffff).alpha(255);
    let shift = |c: Rgba8| {
        if wave >= 0.0 {
            c.mix(white.alpha(c.a), wave * SWING)
        } else {
            deepen(c, -wave * SWING)
        }
    };
    let band = |g: Gradient| Gradient::new(shift(g.from), shift(g.to));
    Bevel {
        border: resting.border,
        upper: band(resting.upper),
        lower: band(resting.lower),
        split: resting.split,
        emboss: resting.emboss,
        text: resting.text,
    }
}

/// Push a colour away from its grey (more saturated) and slightly darker.
fn deepen(c: Rgba8, amount: f32) -> Rgba8 {
    let grey = (c.r as f32 + c.g as f32 + c.b as f32) / 3.0;
    let channel = |v: u8| {
        let v = v as f32;
        let saturated = v + (v - grey) * amount * 2.0;
        (saturated * (1.0 - amount * 0.5)).round().clamp(0.0, 255.0) as u8
    };
    Rgba8::rgba(channel(c.r), channel(c.g), channel(c.b), c.a)
}

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let (states, height, radius) = match self.style {
            ButtonStyle::Push => (
                &theme.controls.push,
                match self.size {
                    ControlSize::Regular => m.push_height,
                    ControlSize::Small => m.push_small_height,
                    ControlSize::Mini => m.push_mini_height,
                },
                m.push_radius,
            ),
            ButtonStyle::Textured => (
                &theme.controls.textured,
                m.textured_height,
                m.textured_radius,
            ),
            ButtonStyle::Gradient | ButtonStyle::Bevel => (
                &theme.controls.gradient,
                match self.size {
                    ControlSize::Regular => 21.0,
                    ControlSize::Small => 19.0,
                    ControlSize::Mini => 17.0,
                },
                0.0,
            ),
            ButtonStyle::Round => (
                &theme.controls.round,
                m.textured_height,
                m.textured_height / 2.0,
            ),
            ButtonStyle::Recessed => (
                &theme.controls.recessed,
                match self.size {
                    ControlSize::Regular => m.push_height,
                    ControlSize::Small => m.push_small_height,
                    ControlSize::Mini => m.push_mini_height,
                },
                m.textured_radius,
            ),
            ButtonStyle::Help => (
                &theme.controls.help.states,
                match self.size {
                    ControlSize::Regular => m.push_height,
                    ControlSize::Small => m.push_small_height,
                    ControlSize::Mini => m.push_mini_height,
                },
                m.push_height / 2.0,
            ),
        };
        let base: Bevel = if self.disabled {
            states.disabled
        } else if self.selected {
            states.selected
        } else if self.default && self.style == ButtonStyle::Push {
            theme.controls.push_default
        } else {
            states.normal
        };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            (
                cx.focus_handle().tab_stop(true),
                Rc::new(Cell::new(Press::Resting)),
            )
        });
        let (default_focus, feedback) = {
            let state = state.read(cx);
            (state.0.clone(), state.1.clone())
        };
        let focus = self.focus.unwrap_or_else(|| default_focus.clone());
        let enabled = !self.disabled;
        if self.default && enabled && window.is_window_active() && window.focused(cx).is_none() {
            window.focus(&focus, cx);
        }
        let handler = self.on_click.filter(|_| enabled);
        if handler.is_none()
            || !window.is_window_active()
            || (feedback.get() == Press::Keyboard && !focus.is_focused(window))
        {
            feedback.set(Press::Resting);
        }
        let amount = feedback.get().amount(window, cx);
        if amount == 0.0 && matches!(feedback.get(), Press::Released { .. }) {
            feedback.set(Press::Resting);
        }
        let feedback = handler.as_ref().map(|_| feedback.clone());
        let throb = self.throb
            && self.default
            && self.style == ButtonStyle::Push
            && enabled
            && window.is_window_active()
            && !cx.reduce_motion()
            && amount == 0.0;
        let throb_phase = Rc::new(Cell::new(0.0));
        let throb_for_paint = throb_phase.clone();
        let bezel = if amount == 0.0 {
            base
        } else {
            let mut pressed = states.pressed;
            if base == pressed {
                // The theme can share its pressed material with a latched toggle.
                // Deepen the inset so the toggle still responds to a fresh press.
                let black = Rgba8::hex(0);
                let deepen = |band: Gradient| {
                    Gradient::new(band.from.mix(black, 0.22), band.to.mix(black, 0.22))
                };
                pressed.upper = deepen(pressed.upper);
                pressed.lower = deepen(pressed.lower);
            }
            blend_bevel(base, pressed, amount)
        };
        let ink = if self.destructive && enabled {
            theme.colors.error.mix(states.pressed.text, amount)
        } else {
            bezel.text
        };
        let ring_focus = focus.clone();
        let pointer_focus = focus.clone();
        let ring_color = theme.colors.focus_ring;
        let ring_profile = m.focus_ring;
        let label_size = self.size.label_size(&theme);
        let icon_only = self.icon.is_some();
        let help_style = self.style == ButtonStyle::Help;
        let padding = if icon_only || help_style {
            0.0
        } else {
            m.push_padding[self.size.index()]
        };
        let icon = self.icon.map(|draw| draw(crate::convert::rgba(ink)));
        let label_dy = if self.style == ButtonStyle::Push {
            m.push_label_dy[self.size.index()]
        } else {
            0.0
        };
        let pointer_down = feedback.clone();
        let key_down = feedback.clone();
        let activation = feedback.clone();
        let key_handler = handler.clone();
        let help = self.style == ButtonStyle::Help;

        div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .role(gpui::Role::Button)
            .aria_label(self.label.clone())
            // Block an enclosing native titlebar's Drag hitbox (same contract
            // as chrome::traffic_lights); controls must remain clickable.
            .occlude()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(height))
            .when(icon_only || help, |el| el.w(px(height)))
            .px(px(padding))
            .when_some(self.min_width, |el, w| el.min_w(px(w)))
            .when_some(self.tooltip, super::tooltip::attach)
            .font_family(theme.type_.ui)
            .text_size(px(label_size))
            .when(enabled, |el| el.track_focus(&focus))
            .when(self.cancel && enabled, |el| {
                let handler = key_handler.clone();
                let key_focus = focus.clone();
                el.on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if !key_focus.is_focused(window) {
                        return;
                    }
                    let stroke = &event.keystroke;
                    if (stroke.key == "escape" && !stroke.modifiers.modified())
                        || (stroke.key == "."
                            && (stroke.modifiers.platform || stroke.modifiers.control))
                    {
                        cx.stop_propagation();
                        if let Some(handler) = &handler {
                            handler(&ClickEvent::default(), window, cx);
                        }
                    }
                })
            })
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if enabled {
                    window.focus(&pointer_focus, cx);
                }
                if let Some(feedback) = &pointer_down {
                    feedback.set(Press::Pointer {
                        inside: true,
                        since: cx.background_executor().now(),
                    });
                    window.refresh();
                }
                cx.stop_propagation();
            })
            .when_some(key_down, |el, feedback| {
                let key_focus = focus.clone();
                let key_up = feedback.clone();
                el.on_key_down(move |event, window, _| {
                    if !key_focus.is_focused(window) {
                        return;
                    }
                    let stroke = &event.keystroke;
                    if !window.default_prevented()
                        && !stroke.modifiers.modified()
                        && matches!(stroke.key.as_str(), "enter" | "space")
                    {
                        feedback.set(Press::Keyboard);
                        window.refresh();
                    } else if feedback.get() == Press::Keyboard {
                        feedback.set(Press::Resting);
                        window.refresh();
                    }
                })
                .on_key_up(move |_, window, _| {
                    if key_up.get() == Press::Keyboard {
                        key_up.set(Press::Resting);
                        window.refresh();
                    }
                })
            })
            .when_some(handler.zip(activation), |el, (handler, feedback)| {
                el.on_click(move |event, window, cx| {
                    if !cx.reduce_motion() {
                        let from = match feedback.get() {
                            Press::Resting => 1.0,
                            press => press.amount(window, cx),
                        };
                        feedback.set(Press::Released {
                            start: cx.background_executor().now(),
                            from,
                        });
                    }
                    window.refresh();
                    handler(event, window, cx);
                })
            })
            .child({
                let bezel_canvas = canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _cx| {
                        let painted = if throb {
                            throb_bevel(bezel, throb_for_paint.get())
                        } else {
                            bezel
                        };
                        paint::bevel(window, bounds, radius, &painted);
                        if let Some(feedback) = &feedback {
                            // Capture release even when another control consumes the mouse-up.
                            // GPUI's later ClickEvent starts the animation for a valid release.
                            let release = feedback.clone();
                            window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _| {
                                if phase.capture()
                                    && event.button == MouseButton::Left
                                    && matches!(release.get(), Press::Pointer { .. })
                                {
                                    release.set(Press::Resting);
                                    window.refresh();
                                }
                            });
                            if matches!(feedback.get(), Press::Pointer { .. }) {
                                let pointer = feedback.clone();
                                window.on_mouse_event(
                                    move |event: &MouseMoveEvent, phase, window, _| {
                                        let Press::Pointer { inside, since } = pointer.get() else {
                                            return;
                                        };
                                        let now = bounds.contains(&event.position);
                                        if phase.capture() && now != inside {
                                            pointer.set(Press::Pointer { inside: now, since });
                                            window.refresh();
                                        }
                                    },
                                );
                                let pointer = feedback.clone();
                                window.on_mouse_event(
                                    move |_: &MouseExitEvent, phase, window, _| {
                                        if let Press::Pointer {
                                            inside: true,
                                            since,
                                        } = pointer.get()
                                        {
                                            if phase.capture() {
                                                pointer.set(Press::Pointer {
                                                    inside: false,
                                                    since,
                                                });
                                                window.refresh();
                                            }
                                        }
                                    },
                                );
                            }
                        }
                    },
                )
                .absolute()
                .inset_0();
                if throb {
                    let phase = throb_phase.clone();
                    bezel_canvas
                        .with_animation(
                            "default-throb",
                            Animation::new(Duration::from_millis(1100)).repeat(),
                            move |el, t| {
                                phase.set(t);
                                el
                            },
                        )
                        .into_any_element()
                } else {
                    bezel_canvas.into_any_element()
                }
            })
            .when_some(icon, |el, icon| el.child(div().relative().child(icon)))
            .when(help, |el| {
                el.child(
                    div()
                        .relative()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(hsla(theme.controls.help.glyph))
                        .child("?"),
                )
            })
            .when(!icon_only && !help, |el| {
                el.child(
                    div()
                        .relative()
                        .top(px(label_dy))
                        .text_color(hsla(ink))
                        .child(self.label),
                )
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                            paint::focus_ring(
                                window,
                                bounds,
                                radius.min(bounds.size.height.as_f32() / 2.0),
                                ring_color,
                                ring_profile,
                            );
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext};
    struct Harness {
        focus: FocusHandle,
        activations: usize,
        background_downs: usize,
        selected: bool,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_mouse_down(
                    gpui::MouseButton::Left,
                    cx.listener(|h, _, window, cx| {
                        h.background_downs += 1;
                        window.focus(&h.focus, cx);
                    }),
                )
                .child(
                    Button::new("tool", "Toolbar control")
                        .style(ButtonStyle::Textured)
                        .selected(self.selected)
                        .on_click(cx.listener(|h, _, _, cx| {
                            h.activations += 1;
                            cx.notify();
                        })),
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
    fn toolbar_button_keeps_focus_from_a_background_input_responder() {
        for theme in crate::ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(theme), cx));
            let (h, cx) = app.add_window_view(|_, cx| Harness {
                focus: cx.focus_handle(),
                activations: 0,
                background_downs: 0,
                selected: false,
            });
            draw(cx);
            let button = cx.debug_bounds("tool").unwrap();
            cx.simulate_click(button.center(), Modifiers::default());
            draw(cx);
            cx.update(|window, cx| {
                assert_eq!(h.read(cx).background_downs, 0);
                assert_eq!(h.read(cx).activations, 1);
                assert!(!h.read(cx).focus.is_focused(window));
                assert!(window.focused(cx).is_some());
            });
            cx.simulate_keystrokes("space");
            cx.simulate_event(gpui::KeyUpEvent {
                keystroke: gpui::Keystroke::parse("space").unwrap(),
            });
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).activations, 2));
        }
    }

    #[test]
    fn press_feedback_tracks_activation_cancellation_and_reduced_motion() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(
                crate::ThemePreference::explicit(crate::ThemeId::AQUA_LIGHT),
                cx,
            )
        });
        let (h, cx) = app.add_window_view(|_, cx| Harness {
            focus: cx.focus_handle(),
            activations: 0,
            background_downs: 0,
            selected: false,
        });
        cx.update(|window, _| window.activate_window());
        draw(cx);
        let button = cx.debug_bounds("tool").unwrap();
        let fill = |cx: &mut VisualTestContext| {
            cx.update(|window, _| {
                let bounds = paint::inset(button, crate::convert::hairline(window))
                    .scale(window.scale_factor());
                window
                    .painted_quads()
                    .iter()
                    .find(|quad| quad.bounds == bounds)
                    .unwrap()
                    .background
            })
        };
        let resting = fill(cx);
        let advance = |cx: &mut VisualTestContext, by: std::time::Duration| {
            cx.update(|window, cx| {
                cx.background_executor().advance_clock(by);
                window.simulate_next_frame(cx);
            });
            draw(cx);
        };
        let position = button.center();
        let outside = gpui::point(position.x, button.bottom() + px(8.));
        let modifiers = Modifiers::default();

        // A pointer press fades in from the resting bevel.
        cx.simulate_mouse_down(position, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(fill(cx), resting);
        advance(cx, PRESS_DURATION / 2);
        let pressing = fill(cx);
        assert_ne!(pressing, resting);
        advance(cx, PRESS_DURATION / 2);
        let held = fill(cx);
        assert_ne!(held, pressing);
        assert_ne!(held, resting);
        cx.simulate_mouse_move(outside, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(fill(cx), resting);
        cx.simulate_mouse_up(outside, MouseButton::Left, modifiers);
        draw(cx);
        cx.update(|_, cx| assert_eq!(h.read(cx).activations, 0));

        // Holding keeps the pressed bevel; release fades back out.
        cx.simulate_mouse_down(position, MouseButton::Left, modifiers);
        draw(cx);
        advance(cx, PRESS_DURATION * 4);
        assert_eq!(fill(cx), held);
        cx.simulate_mouse_up(position, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(fill(cx), held);
        cx.update(|_, cx| assert_eq!(h.read(cx).activations, 1));
        advance(cx, RELEASE_DURATION / 2);
        let halfway = fill(cx);
        assert_ne!(halfway, held);
        assert_ne!(halfway, resting);
        advance(cx, RELEASE_DURATION / 2);
        assert_eq!(fill(cx), resting);

        cx.update(|_, cx| cx.set_reduce_motion(true));
        cx.simulate_keystrokes("space");
        draw(cx);
        assert_eq!(fill(cx), held);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse("space").unwrap(),
        });
        draw(cx);
        assert_eq!(fill(cx), resting);
        cx.update(|_, cx| assert_eq!(h.read(cx).activations, 2));

        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.selected = true;
                cx.notify();
            });
        });
        draw(cx);
        let selected = fill(cx);
        cx.simulate_mouse_down(position, MouseButton::Left, modifiers);
        draw(cx);
        advance(cx, PRESS_DURATION);
        assert_ne!(fill(cx), selected);
        cx.simulate_mouse_up(position, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(fill(cx), selected);
    }

    #[test]
    fn throb_breathes_lighter_and_deeper_around_the_measured_default() {
        let theme = silvestris_core::aqua::aqua_light();
        let rest = theme.controls.push_default;
        let sum = |c: Rgba8| c.r as u32 + c.g as u32 + c.b as u32;
        let spread = |c: Rgba8| c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
        assert_eq!(throb_bevel(rest, 0.0), rest);
        let light = throb_bevel(rest, 0.25).lower.from;
        let deep = throb_bevel(rest, 0.75).lower.from;
        let base = rest.lower.from;
        assert!(sum(light) > sum(base) && spread(light) < spread(base));
        assert!(sum(deep) < sum(base) && spread(deep) > spread(base));
        // Subtle: no channel moves more than about a tenth of its range.
        for c in [light, deep] {
            for (x, y) in [(c.r, base.r), (c.g, base.g), (c.b, base.b)] {
                assert!(x.abs_diff(y) <= 26, "{c:?} vs {base:?}");
            }
        }
    }

    #[test]
    fn default_and_cancel_key_equivalents_activate_from_an_unfocused_dialog() {
        struct Dialog {
            focus: FocusHandle,
            accepted: usize,
            cancelled: usize,
        }
        impl Render for Dialog {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .size_full()
                    .track_focus(&self.focus)
                    .child(
                        Button::new("ok", "OK")
                            .default_button(true)
                            .on_click(cx.listener(|d, _, _, cx| {
                                d.accepted += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("cancel", "Cancel")
                            .cancel_button(true)
                            .on_click(cx.listener(|d, _, _, cx| {
                                d.cancelled += 1;
                                cx.notify();
                            })),
                    )
            }
        }
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (dialog, cx) = app.add_window_view(|_, cx| Dialog {
            focus: cx.focus_handle(),
            accepted: 0,
            cancelled: 0,
        });
        cx.update(|window, _| {
            window.activate_window();
        });
        draw(cx);
        cx.simulate_keystrokes("enter");
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse("enter").unwrap(),
        });
        draw(cx);
        // Tab leaves the default button; Escape still reaches the cancel
        // button's key equivalent wherever focus sits in the dialog.
        cx.update(|window, cx| window.focus_next(cx));
        draw(cx);
        cx.simulate_keystrokes("escape");
        draw(cx);
        cx.update(|_, cx| {
            assert_eq!(dialog.read(cx).accepted, 1);
            assert_eq!(dialog.read(cx).cancelled, 1);
        });
    }
}
