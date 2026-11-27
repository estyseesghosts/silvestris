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
    /// Fixed circular Snow Leopard installer Continue button. It draws its own
    /// two-dot-and-triangle glyph and never shows a text label; the supplied
    /// label remains the accessible name. Regular size only.
    InstallerContinue,
}

impl ButtonStyle {
    /// Styles whose default material breathes. The installer control and the
    /// blue push button share `throb_bevel`; no other style animates.
    fn breathes_when_default(self) -> bool {
        matches!(self, Self::Push | Self::InstallerContinue)
    }
}

/// Single eligibility rule for default-button breathing. Keeping it in one
/// place means a new capability cannot enable the animation for toggles,
/// segments or help buttons by accident.
fn should_throb(
    style: ButtonStyle,
    default: bool,
    throb: bool,
    enabled: bool,
    active_window: bool,
    reduce_motion: bool,
    press_amount: f32,
) -> bool {
    throb
        && default
        && style.breathes_when_default()
        && enabled
        && active_window
        && !reduce_motion
        && press_amount == 0.0
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
    show_label_with_icon: bool,
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
            show_label_with_icon: false,
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

    /// Show the visible label beside an icon, for toolbar Icon & Label mode.
    pub fn icon_and_label(mut self, show: bool) -> Self {
        self.show_label_with_icon = show;
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

/// Glyph geometry in the 30 px reference diameter. Shared by the painter and
/// the geometry test so the measured spacing cannot drift.
const INSTALLER_GLYPH_DOT: f32 = 4.0;
const INSTALLER_GLYPH_DOTS: [(f32, f32); 2] = [(7.0, 13.0), (13.0, 13.0)];
const INSTALLER_GLYPH_TRIANGLE: [(f32, f32); 3] = [(19.0, 10.0), (24.5, 15.0), (19.0, 20.0)];

/// Snow Leopard installer Continue glyph: two filled dots and a solid
/// right-pointing triangle, measured from the supplied close-up.
///
/// The glyph is 17.5 x 10 px at the 30 px reference diameter: 4 px dots at
/// x 7 and 13 (2 px gap), then a 2 px gap and a triangle whose left edge starts
/// at x 19 and whose apex sits at x 24.5. It is centred on the button by these
/// fixed offsets in every state; only the host ink changes.
fn paint_installer_continue_glyph(
    window: &mut Window,
    bounds: gpui::Bounds<gpui::Pixels>,
    ink: Rgba8,
) {
    let unit = bounds.size.width.as_f32() / 30.0;
    if unit <= 0.0 {
        return;
    }
    let at = |x: f32, y: f32| {
        gpui::point(
            bounds.origin.x + px(x * unit),
            bounds.origin.y + px(y * unit),
        )
    };
    let mut dot = |(x, y): (f32, f32)| {
        let size = px(INSTALLER_GLYPH_DOT * unit);
        window.paint_quad(
            gpui::fill(
                gpui::Bounds::new(at(x, y), gpui::size(size, size)),
                hsla(ink),
            )
            .corner_radii(px(INSTALLER_GLYPH_DOT / 2.0 * unit)),
        );
    };
    for position in INSTALLER_GLYPH_DOTS {
        dot(position);
    }
    let points: Vec<_> = INSTALLER_GLYPH_TRIANGLE
        .iter()
        .map(|&(x, y)| at(x, y))
        .collect();
    crate::paint::polygon(window, &points, ink);
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
                match self.size {
                    ControlSize::Regular => m.textured_height,
                    ControlSize::Small => m.push_small_height,
                    ControlSize::Mini => m.push_mini_height,
                },
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
            ButtonStyle::InstallerContinue => {
                let diameter = m.installer_continue_diameter;
                // The dedicated installer material is selected below from the
                // `installer_continue` token; `states` only provides the shared
                // pressed/text slots for the code path.
                (&theme.controls.push, diameter, diameter / 2.0)
            }
        };
        let installer = (self.style == ButtonStyle::InstallerContinue)
            .then_some(theme.controls.installer_continue);
        let base: Bevel = if let Some(installer) = installer {
            if self.disabled {
                installer.disabled
            } else if self.default {
                installer.default
            } else {
                installer.normal
            }
        } else if self.disabled {
            states.disabled
        } else if self.selected {
            states.selected
        } else if self.default && self.style.breathes_when_default() {
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
        let throb = should_throb(
            self.style,
            self.default,
            self.throb,
            enabled,
            window.is_window_active(),
            cx.reduce_motion(),
            amount,
        );
        let throb_phase = Rc::new(Cell::new(0.0));
        let throb_for_paint = throb_phase.clone();
        let pressed_bevel = installer.map_or(states.pressed, |installer| installer.pressed);
        let bezel = if amount == 0.0 {
            base
        } else {
            let mut pressed = pressed_bevel;
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
            theme.colors.error.mix(pressed_bevel.text, amount)
        } else {
            bezel.text
        };
        // The installer glyph uses its own per-state ink; it never follows the
        // push-button label tone.
        let installer_ink = installer.map(|token| {
            if self.disabled {
                token.glyph_disabled
            } else if self.default {
                token.glyph_default.mix(token.glyph_pressed, amount)
            } else {
                token.glyph.mix(token.glyph_pressed, amount)
            }
        });
        let ring_focus = focus.clone();
        let pointer_focus = focus.clone();
        let ring_color = theme.colors.focus_ring;
        let ring_profile = m.focus_ring;
        let label_size = self.size.label_size(&theme);
        let installer_glyph = installer.is_some();
        let has_icon = self.icon.is_some() || installer_glyph;
        let icon_and_label = has_icon && self.show_label_with_icon && !installer_glyph;
        let icon_only = has_icon && (!self.show_label_with_icon || installer_glyph);
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
            .when_some(self.min_width.filter(|_| !installer_glyph), |el, w| {
                el.min_w(px(w))
            })
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
            .when_some(icon, |el, icon| {
                el.child(
                    div()
                        .relative()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(icon)
                        .when(icon_and_label, |el| {
                            el.child(
                                div()
                                    .relative()
                                    .top(px(label_dy))
                                    .text_color(hsla(ink))
                                    .child(self.label.clone()),
                            )
                        }),
                )
            })
            .when(help, |el| {
                el.child(
                    // M: the Dock pane's glyph is 24 of the ring's 42 rows
                    // (systempreferences-dock.png, x 1306, rows 670..693);
                    // the inherited 13 pt face drew it 19 rows tall.
                    div()
                        .relative()
                        .text_size(px(height * 13.0 * 24.0 / 19.0 / 21.0))
                        .line_height(px(height))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(hsla(theme.controls.help.glyph))
                        .child("?"),
                )
            })
            .when(installer_glyph, |el| {
                let glyph_ink = installer_ink.unwrap_or(ink);
                el.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            paint_installer_continue_glyph(window, bounds, glyph_ink);
                        },
                    )
                    .flex_none()
                    .size(px(height)),
                )
            })
            .when(!icon_only && !icon_and_label && !help, |el| {
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

    #[test]
    fn installer_continue_glyph_matches_the_measured_spacing() {
        let [first, second] = INSTALLER_GLYPH_DOTS;
        assert_eq!(first.1, second.1, "dots share a baseline");
        let [top, apex, bottom] = INSTALLER_GLYPH_TRIANGLE;
        assert_eq!(top.0, bottom.0, "triangle has a vertical left edge");
        assert!((apex.1 - (top.1 + bottom.1) / 2.0).abs() < f32::EPSILON);
        assert!(
            (first.1 + INSTALLER_GLYPH_DOT / 2.0 - apex.1).abs() < f32::EPSILON,
            "dots align with the triangle's vertical centre"
        );
        // 2 px gap between the dots, then 2 px before the triangle.
        assert_eq!(second.0 - (first.0 + INSTALLER_GLYPH_DOT), 2.0);
        assert_eq!(top.0 - (second.0 + INSTALLER_GLYPH_DOT), 2.0);
        // Complete glyph: 17.5 x 10 px, matching the 17 x 10 capture.
        assert!((apex.0 - first.0 - 17.5).abs() < f32::EPSILON);
        assert!((bottom.1 - top.1 - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn installer_continue_throb_varies_only_the_blue_bands() {
        let token = silvestris_core::aqua::aqua_light()
            .controls
            .installer_continue;
        let rest = token.default;
        let light = throb_bevel(rest, 0.25);
        let deep = throb_bevel(rest, 0.75);
        assert_eq!(throb_bevel(rest, 0.0), rest);
        for (phase, bevel) in [(0.25, light), (0.75, deep)] {
            assert_ne!(bevel.upper.from, rest.upper.from, "phase {phase} upper");
            assert_ne!(bevel.upper.to, rest.upper.to, "phase {phase} upper end");
            assert_ne!(bevel.lower.from, rest.lower.from, "phase {phase} lower");
            assert_ne!(bevel.lower.to, rest.lower.to, "phase {phase} lower end");
            // Rim, emboss and glyph ink never move.
            assert_eq!(bevel.border, rest.border);
            assert_eq!(bevel.emboss, rest.emboss);
            assert_eq!(bevel.split, rest.split);
            assert_eq!(bevel.text, rest.text);
        }
        // The default token is its own material, not the push button's.
        assert_ne!(
            rest.upper,
            silvestris_core::aqua::aqua_light()
                .controls
                .push_default
                .upper
        );
    }

    #[test]
    fn installer_continue_breathing_eligibility_matches_the_contract() {
        let installer = ButtonStyle::InstallerContinue;
        assert!(should_throb(installer, true, true, true, true, false, 0.0));
        assert!(!should_throb(
            installer, false, true, true, true, false, 0.0
        ));
        assert!(!should_throb(
            installer, true, false, true, true, false, 0.0
        ));
        assert!(!should_throb(
            installer, true, true, false, true, false, 0.0
        ));
        assert!(!should_throb(
            installer, true, true, true, false, false, 0.0
        ));
        assert!(!should_throb(installer, true, true, true, true, true, 0.0));
        // A held press pauses the breathing; focus elsewhere does not.
        assert!(!should_throb(installer, true, true, true, true, false, 0.5));
        assert!(should_throb(installer, true, true, true, true, false, 0.0));
        // Only the blue push button and the installer control opt in.
        assert!(should_throb(
            ButtonStyle::Push,
            true,
            true,
            true,
            true,
            false,
            0.0
        ));
        for style in [
            ButtonStyle::Textured,
            ButtonStyle::Gradient,
            ButtonStyle::Bevel,
            ButtonStyle::Round,
            ButtonStyle::Recessed,
            ButtonStyle::Help,
        ] {
            assert!(
                !should_throb(style, true, true, true, true, false, 0.0),
                "{style:?} must not breathe"
            );
        }
    }

    struct InstallerHarness {
        focus: FocusHandle,
        activations: usize,
        disabled: bool,
        label: &'static str,
    }

    impl Render for InstallerHarness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().track_focus(&self.focus).child(
                Button::new("installer", self.label)
                    .style(ButtonStyle::InstallerContinue)
                    .default_button(true)
                    .throb(false)
                    .disabled(self.disabled)
                    .on_click(cx.listener(|h, _, _, cx| {
                        h.activations += 1;
                        cx.notify();
                    })),
            )
        }
    }

    fn installer_view<'a>(
        app: &'a mut TestAppContext,
        label: &'static str,
    ) -> (gpui::Entity<InstallerHarness>, &'a mut VisualTestContext) {
        app.update(|cx| {
            crate::theme::install(
                crate::ThemePreference::explicit(crate::ThemeId::AQUA_LIGHT),
                cx,
            )
        });
        let (harness, cx) = app.add_window_view(|_, cx| InstallerHarness {
            focus: cx.focus_handle(),
            activations: 0,
            disabled: false,
            label,
        });
        cx.update(|window, _| window.activate_window());
        draw(cx);
        (harness, cx)
    }

    #[test]
    fn installer_continue_bounds_stay_square_at_every_golden_scale() {
        for scale in [1.0_f32, 1.25, 1.5, 1.75, 2.0] {
            let mut app = TestAppContext::single();
            let (_harness, cx) = installer_view(&mut app, "Continue");
            cx.simulate_scale_factor_change(scale);
            draw(cx);
            let bounds = cx.debug_bounds("installer").unwrap();
            assert!(
                (bounds.size.width.as_f32() - 30.0).abs() <= 1.0,
                "scale {scale}: width {:?}",
                bounds.size.width
            );
            assert!(
                (bounds.size.width.as_f32() - bounds.size.height.as_f32()).abs() < 0.01,
                "scale {scale}: {bounds:?}"
            );
            // The painted bezel is the quad closest to the 30 px square. Its
            // device-space edges must stay equal after snapping.
            let target = (30.0 * scale).powi(2);
            let (width, height) = cx.update(|window, _| {
                let mut best: Option<(f32, f32, f32)> = None;
                for quad in window.painted_quads() {
                    let w = quad.bounds.size.width.0;
                    let h = quad.bounds.size.height.0;
                    let diff = (w * h - target).abs();
                    if best.is_none_or(|(d, _, _)| diff < d) {
                        best = Some((diff, w, h));
                    }
                }
                best.map(|(_, w, h)| (w, h)).unwrap()
            });
            assert!(
                (width - 30.0 * scale).abs() <= 1.0,
                "scale {scale}: bezel width {width}"
            );
            assert!(
                (width - height).abs() <= 1.0,
                "scale {scale}: bezel {width} x {height} is not square"
            );
        }
    }

    #[test]
    fn installer_continue_width_ignores_label_length() {
        let mut widths = Vec::new();
        for label in ["", "Continue", "Continue with a dramatically longer label"] {
            let mut app = TestAppContext::single();
            let (_harness, cx) = installer_view(&mut app, label);
            let bounds = cx.debug_bounds("installer").unwrap();
            widths.push(bounds.size.width.as_f32());
        }
        for width in &widths {
            assert!((width - 30.0).abs() < 0.01, "{widths:?}");
        }
    }

    #[test]
    fn installer_continue_activates_once_and_suppresses_cancelled_or_disabled_input() {
        let mut app = TestAppContext::single();
        let (harness, cx) = installer_view(&mut app, "Continue");
        let bounds = cx.debug_bounds("installer").unwrap();
        let position = bounds.center();
        let modifiers = Modifiers::default();

        cx.simulate_click(position, modifiers);
        draw(cx);
        assert_eq!(cx.update(|_, cx| harness.read(cx).activations), 1);

        // Keyboard Space activates once while the button holds focus.
        cx.simulate_keystrokes("space");
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse("space").unwrap(),
        });
        draw(cx);
        assert_eq!(cx.update(|_, cx| harness.read(cx).activations), 2);

        // Releasing outside the circle cancels the pending activation.
        cx.simulate_mouse_down(position, MouseButton::Left, modifiers);
        draw(cx);
        let outside = gpui::point(position.x, bounds.bottom() + px(12.));
        cx.simulate_mouse_move(outside, MouseButton::Left, modifiers);
        draw(cx);
        cx.simulate_mouse_up(outside, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(cx.update(|_, cx| harness.read(cx).activations), 2);

        cx.update(|_, cx| {
            harness.update(cx, |h, cx| {
                h.disabled = true;
                cx.notify();
            });
        });
        draw(cx);
        cx.simulate_click(position, modifiers);
        draw(cx);
        assert_eq!(cx.update(|_, cx| harness.read(cx).activations), 2);
    }

    #[test]
    fn installer_continue_default_action_routes_once_from_a_dialog() {
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
                        Button::new("continue", "Continue")
                            .style(ButtonStyle::InstallerContinue)
                            .default_button(true)
                            .throb(false)
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
        cx.update(|window, _| window.activate_window());
        draw(cx);
        cx.simulate_keystrokes("enter");
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse("enter").unwrap(),
        });
        draw(cx);
        cx.update(|window, cx| window.focus_next(cx));
        draw(cx);
        cx.simulate_keystrokes("escape");
        draw(cx);
        cx.update(|_, cx| {
            assert_eq!(dialog.read(cx).accepted, 1, "one activation per Return");
            assert_eq!(dialog.read(cx).cancelled, 1);
        });
    }

    #[test]
    fn installer_continue_focus_ring_is_circular() {
        let mut app = TestAppContext::single();
        let (_harness, cx) = installer_view(&mut app, "Continue");
        // Keyboard input enables the ring; shift-tab returns focus to the
        // default button after the first tab leaves it.
        cx.simulate_keystrokes("tab");
        cx.simulate_keystrokes("shift-tab");
        draw(cx);
        let rings = cx.update(|window, _| {
            window
                .painted_quads()
                .into_iter()
                .filter(|quad| quad.border_color.a > 0.0)
                .map(|quad| (quad.bounds.size.width.0, quad.corner_radii.top_left.0))
                .collect::<Vec<_>>()
        });
        assert!(!rings.is_empty(), "keyboard focus paints a ring");
        for (width, radius) in rings {
            assert!(
                (radius * 2.0 - width).abs() <= 0.5,
                "ring {width} px wide with radius {radius} is not circular"
            );
        }
    }

    struct InstallerMotionHarness {
        focus: FocusHandle,
        other_focus: FocusHandle,
        activations: usize,
        disabled: bool,
    }

    impl Render for InstallerMotionHarness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(
                    Button::new("installer", "Continue")
                        .style(ButtonStyle::InstallerContinue)
                        .default_button(true)
                        .disabled(self.disabled)
                        .on_click(cx.listener(|h, _, _, cx| {
                            h.activations += 1;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .track_focus(&self.other_focus)
                        .size(px(24.))
                        .debug_selector(|| "other-control".to_string()),
                )
                .track_focus(&self.focus)
        }
    }

    #[test]
    fn installer_continue_breathing_pauses_for_press_and_resumes_after_release() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(
                crate::ThemePreference::explicit(crate::ThemeId::AQUA_LIGHT),
                cx,
            )
        });
        let (harness, cx) = app.add_window_view(|_, cx| InstallerMotionHarness {
            focus: cx.focus_handle(),
            other_focus: cx.focus_handle(),
            activations: 0,
            disabled: false,
        });
        cx.update(|window, _| window.activate_window());
        draw(cx);
        let bounds = cx.debug_bounds("installer").unwrap();
        // The lower band is painted full-width over the inset bezel; it is the
        // first quad matching that rectangle and carries the throb gradient.
        let fill = |cx: &mut VisualTestContext| {
            cx.update(|window, _| {
                let inner = paint::inset(bounds, crate::convert::hairline(window))
                    .scale(window.scale_factor());
                window
                    .painted_quads()
                    .iter()
                    .find(|quad| quad.bounds == inner)
                    .unwrap()
                    .background
            })
        };
        let advance = |cx: &mut VisualTestContext, by: Duration| {
            cx.update(|window, cx| {
                cx.background_executor().advance_clock(by);
                window.simulate_next_frame(cx);
            });
            draw(cx);
        };
        let quarter = Duration::from_millis(275);
        let position = bounds.center();
        let modifiers = Modifiers::default();

        // The idle default breathes: quarter-cycle samples are not all equal.
        let mut idle = vec![fill(cx)];
        for _ in 0..3 {
            advance(cx, quarter);
            idle.push(fill(cx));
        }
        assert!(
            idle.windows(2).any(|pair| pair[0] != pair[1]),
            "the rendered bezel must change over the clock: {idle:?}"
        );

        // Holding a press settles on the pressed material and stops the throb.
        cx.simulate_mouse_down(position, MouseButton::Left, modifiers);
        draw(cx);
        advance(cx, PRESS_DURATION * 4);
        let held = fill(cx);
        advance(cx, quarter);
        assert_eq!(fill(cx), held, "breathing must pause while pressed");
        advance(cx, quarter);
        assert_eq!(fill(cx), held, "breathing must stay paused while held");

        // Releasing activates once and resumes the breathing after the release
        // transition completes.
        cx.simulate_mouse_up(position, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(cx.update(|_, cx| harness.read(cx).activations), 1);
        advance(cx, RELEASE_DURATION);
        let mut resumed = vec![fill(cx)];
        for _ in 0..3 {
            advance(cx, quarter);
            resumed.push(fill(cx));
        }
        assert!(
            resumed.windows(2).any(|pair| pair[0] != pair[1]),
            "breathing must resume after release: {resumed:?}"
        );

        // Moving focus to another control leaves the installer breathing.
        cx.update(|window, cx| {
            let other = harness.read(cx).other_focus.clone();
            window.focus(&other, cx);
        });
        draw(cx);
        cx.update(|window, cx| {
            assert!(
                harness.read(cx).other_focus.is_focused(window),
                "focus moved off the installer"
            );
        });
        let mut unfocused = vec![fill(cx)];
        for _ in 0..3 {
            advance(cx, quarter);
            unfocused.push(fill(cx));
        }
        assert!(
            unfocused.windows(2).any(|pair| pair[0] != pair[1]),
            "breathing must continue when another control has focus: {unfocused:?}"
        );

        // Disabling during a held press cancels activation and stops the throb.
        cx.simulate_mouse_down(position, MouseButton::Left, modifiers);
        draw(cx);
        advance(cx, PRESS_DURATION);
        let pressed = fill(cx);
        cx.update(|_, cx| {
            harness.update(cx, |h, cx| {
                h.disabled = true;
                cx.notify();
            });
        });
        draw(cx);
        let disabled = fill(cx);
        assert_ne!(
            disabled, pressed,
            "disabling must leave the pressed material"
        );
        advance(cx, quarter);
        assert_eq!(fill(cx), disabled, "the disabled control must not breathe");
        cx.simulate_mouse_up(position, MouseButton::Left, modifiers);
        draw(cx);
        assert_eq!(
            cx.update(|_, cx| harness.read(cx).activations),
            1,
            "disabling mid-press must cancel activation"
        );
    }

    #[test]
    fn existing_push_round_and_help_styles_do_not_join_the_throb() {
        assert!(ButtonStyle::Push.breathes_when_default());
        assert!(ButtonStyle::InstallerContinue.breathes_when_default());
        for style in [ButtonStyle::Round, ButtonStyle::Help, ButtonStyle::Textured] {
            assert!(!style.breathes_when_default(), "{style:?}");
        }
        // The push default material is untouched by the new token.
        let theme = silvestris_core::aqua::aqua_light();
        assert_eq!(
            throb_bevel(theme.controls.push_default, 0.0),
            theme.controls.push_default
        );
    }
}
