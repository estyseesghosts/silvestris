//! Push and textured buttons (HIG p. 265-269; handbook 12).
//!
//! - Height per control size: `Metrics::push_height` / `push_small_height` /
//!   `push_mini_height` (M: button.push.default, button.push.normal).
//! - Width = label + 2 × `Metrics::push_padding` (M: button.push.widths), or
//!   wider via `min_width` for equal-width groups (HIG PDF p. 359, "Similar
//!   controls have the same width").
//! - Label font: system, small system, mini system for regular, small, mini
//!   (HIG p. 269).
//! - Pressed draws the `pressed` bevel (blue in Aqua) while the pointer is down;
//!   Enter/Space activate a focused button (GPUI div.rs keyboard click).
//! - The focus ring shows only after keyboard input (handbook 12 focus manager).
//! - Disabled buttons are not focusable and have no click handler. GPUI at the
//!   pin has no `aria_disabled`, so assistive tech is not told the state; this
//!   is a recorded gap (docs/exceptions.md).

use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, ClickEvent, ElementId, FocusHandle, SharedString, Window,
};
use silvestris_core::tokens::{Bevel, Theme};

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
    disabled: bool,
    selected: bool,
    min_width: Option<f32>,
    icon: Option<Icon>,
    tooltip: Option<SharedString>,
    destructive: bool,
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
            disabled: false,
            selected: false,
            min_width: None,
            icon: None,
            tooltip: None,
            destructive: false,
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

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }
}

/// Group name for the pressed layer; GPUI resolves a group name to the
/// innermost enclosing element that declared it, so one name serves all.
const GROUP: &str = "silvestris-button";

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
            ButtonStyle::Gradient => (&theme.controls.gradient, 21.0, 0.0),
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
        let pressed = states.pressed;
        let focus: FocusHandle = window
            // Handles default to tab_stop(false) (gpui window.rs FocusHandle).
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let ring_focus = focus.clone();
        let pointer_focus = focus.clone();
        let ring_color = theme.colors.focus_ring;
        let ring_profile = m.focus_ring;
        let label_size = self.size.label_size(&theme);
        let icon_only = self.icon.is_some();
        let padding = if icon_only { 0.0 } else { m.push_padding[self.size.index()] };
        let resting_ink = if self.destructive && !self.disabled {
            theme.colors.error
        } else {
            base.text
        };
        let icon = self.icon.as_ref().map(|draw| draw(crate::convert::rgba(resting_ink)));
        let pressed_icon = self.icon.as_ref().filter(|_| !self.disabled)
            .map(|draw| draw(crate::convert::rgba(pressed.text)));
        let label_dy = if self.style == ButtonStyle::Push {
            m.push_label_dy[self.size.index()]
        } else {
            0.0
        };
        let enabled = !self.disabled;

        div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .group(GROUP)
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
            .when(icon_only, |el| el.w(px(height)))
            .px(px(padding))
            .when_some(self.min_width, |el, w| el.min_w(px(w)))
            .when_some(self.tooltip, super::tooltip::attach)
            .font_family(theme.type_.ui)
            .text_size(px(label_size))
            .when(enabled, |el| el.track_focus(&focus))
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                if enabled {
                    window.focus(&pointer_focus, cx);
                }
                cx.stop_propagation();
            })
            .when_some(self.on_click.filter(|_| enabled), |el, f| {
                el.on_click(move |event, window, cx| f(event, window, cx))
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint::bevel(window, bounds, radius, &base);
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when(enabled, |el| {
                el.child(
                    div()
                        .id("pressed")
                        .absolute()
                        .inset_0()
                        .opacity(0.)
                        .group_active(GROUP, |s| s.opacity(1.))
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    paint::bevel(window, bounds, radius, &pressed);
                                },
                            )
                            .size_full(),
                        ),
                )
            })
            .when_some(icon, |el, icon| {
                el.child(
                    div().id("icon").relative()
                        .when(enabled, |el| el.group_active(GROUP, |s| s.opacity(0.0)))
                        .child(icon),
                )
                .when_some(pressed_icon, |el, icon| {
                    el.child(
                        div().id("pressed-icon").absolute().inset_0()
                            .flex().items_center().justify_center()
                            .opacity(0.0)
                            .group_active(GROUP, |s| s.opacity(1.0))
                            .child(icon),
                    )
                })
            })
            .when(!icon_only, |el| {
                el.child(
                    div()
                        .id("label")
                        .relative()
                        .top(px(label_dy))
                        .text_color(hsla(resting_ink))
                        .when(enabled, |el| {
                            el.group_active(GROUP, |s| s.text_color(hsla(pressed.text)))
                        })
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
}
