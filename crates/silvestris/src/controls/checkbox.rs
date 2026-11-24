//! Independent on/off/mixed selection (HIG pp. 283–286; hig-09 CMP-03).
//! State belongs to the consumer. Mixed activates to On; this is an explicit
//! recreation decision. The label shares the hit target. Space/Enter use
//! GPUI's keyboard click path. Disabled controls cannot focus or activate.
use std::rc::Rc;

use gpui::{
    canvas, div, point, prelude::*, px, App, Bounds, ElementId, Pixels, SharedString, Window,
};
use silvestris_core::tokens::Bevel;

use super::ControlSize;
use crate::{convert::hsla, paint, ActiveTheme};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckState {
    #[default]
    Off,
    On,
    Mixed,
}

impl From<bool> for CheckState {
    fn from(checked: bool) -> Self {
        if checked {
            Self::On
        } else {
            Self::Off
        }
    }
}

impl CheckState {
    pub fn activated(self) -> Self {
        match self {
            Self::On => Self::Off,
            Self::Off | Self::Mixed => Self::On,
        }
    }

    fn accessible(self) -> gpui::Toggled {
        match self {
            Self::Off => gpui::Toggled::False,
            Self::On => gpui::Toggled::True,
            Self::Mixed => gpui::Toggled::Mixed,
        }
    }
}

type ChangeHandler = Rc<dyn Fn(&CheckState, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Checkbox {
    id: ElementId,
    label: SharedString,
    state: CheckState,
    size: ControlSize,
    disabled: bool,
    show_label: bool,
    on_change: Option<ChangeHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            state: CheckState::Off,
            size: ControlSize::Regular,
            disabled: false,
            show_label: true,
            on_change: None,
        }
    }

    pub fn checked(mut self, checked: bool) -> Self {
        self.state = checked.into();
        self
    }

    pub fn state(mut self, state: CheckState) -> Self {
        self.state = state;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Hide a duplicate visible label in tables/forms. Accessible name remains.
    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&CheckState, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

const GROUP: &str = "silvestris-checkbox";

fn box_paint(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radius: f32,
    bevel: Bevel,
    state: CheckState,
) {
    paint::bevel(window, bounds, radius, &bevel);
    // M: checkbox.on at 2x: the check's left arm starts at 10% width / 52%
    // height, its elbow sits at 40% / 84%, and the long arm overshoots the top
    // right at 94% / 6%. The mark is a filled path so it does not depend on a
    // font substitution.
    let vertices: &[(f32, f32)] = match state {
        CheckState::Off => return,
        CheckState::On => &[
            (0.10, 0.52),
            (0.24, 0.40),
            (0.40, 0.60),
            (0.82, 0.10),
            (0.94, 0.20),
            (0.40, 0.86),
        ],
        CheckState::Mixed => &[(0.18, 0.44), (0.82, 0.44), (0.82, 0.56), (0.18, 0.56)],
    };
    let at = |(x, y)| {
        point(
            bounds.left() + bounds.size.width * x,
            bounds.top() + bounds.size.height * y,
        )
    };
    let mut path = gpui::PathBuilder::fill();
    path.move_to(at(vertices[0]));
    for vertex in &vertices[1..] {
        path.line_to(at(*vertex));
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, hsla(bevel.text));
    }
}

impl RenderOnce for Checkbox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let pointer_focus = focus.clone();
        let enabled = !self.disabled;
        let states = theme.controls.checkbox;
        let state = self.state;
        let base = if !enabled {
            states.disabled
        } else if state == CheckState::Off {
            states.normal
        } else {
            states.selected
        };
        let dimension = match self.size {
            ControlSize::Regular => theme.metrics.checkbox_size,
            ControlSize::Small => theme.metrics.checkbox_small_size,
            ControlSize::Mini => theme.metrics.checkbox_mini_size,
        };
        let radius = theme.metrics.checkbox_radius;
        let ring = theme.colors.focus_ring;
        let profile = theme.metrics.focus_ring;
        let label_color = if enabled {
            theme.colors.text
        } else {
            theme.colors.text_disabled
        };
        div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .group(GROUP)
            .role(gpui::Role::CheckBox)
            .aria_label(self.label.clone())
            .aria_toggled(state.accessible())
            .flex()
            .items_center()
            .gap(px(theme.metrics.checkbox_label_gap))
            .font_family(theme.type_.ui)
            .text_size(px(self.size.label_size(&theme)))
            .text_color(hsla(label_color))
            .when(enabled, |el| {
                el.track_focus(&focus).on_mouse_down(
                    gpui::MouseButton::Left,
                    move |_, window, cx| {
                        window.focus(&pointer_focus, cx);
                        cx.stop_propagation();
                    },
                )
            })
            .when_some(self.on_change.filter(|_| enabled), |el, handler| {
                el.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler(&state.activated(), window, cx);
                })
            })
            .child(
                div()
                    .id("box")
                    .relative()
                    .flex_none()
                    .size(px(dimension))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |b, _, window, _| box_paint(window, b, radius, base, state),
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
                                        move |b, _, window, _| {
                                            box_paint(window, b, radius, states.pressed, state)
                                        },
                                    )
                                    .size_full(),
                                ),
                        )
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |b, _, window, _| {
                                if enabled
                                    && focus.is_focused(window)
                                    && window.last_input_was_keyboard()
                                {
                                    paint::focus_ring(window, b, radius, ring, profile);
                                }
                            },
                        )
                        .absolute()
                        .inset_0(),
                    ),
            )
            .when(self.show_label, |el| {
                el.child(div().id("label").child(self.label))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext};

    struct Harness {
        state: CheckState,
        changes: usize,
        parent_clicks: usize,
        disabled: bool,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .id("parent")
                .size_full()
                .flex()
                .flex_col()
                .gap_4()
                .on_click(cx.listener(|h, _, _, _| h.parent_clicks += 1))
                .child(
                    Checkbox::new("setting", "Enable independent setting with a long label")
                        .state(self.state)
                        .disabled(self.disabled)
                        .on_change(cx.listener(|h, state: &CheckState, _, cx| {
                            h.state = *state;
                            h.changes += 1;
                            cx.notify();
                        })),
                )
                .child(Checkbox::new("small", "Small").size(ControlSize::Small))
                .child(Checkbox::new("mini", "Mini").size(ControlSize::Mini))
                .child(Checkbox::new("unavailable", "Unavailable").disabled(true))
        }
    }
    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }

    fn key(cx: &mut VisualTestContext, key: &'static str) {
        cx.simulate_keystrokes(key);
        cx.simulate_event(gpui::KeyUpEvent {
            keystroke: gpui::Keystroke::parse(key).unwrap(),
        });
    }

    #[test]
    fn pointer_keyboard_mixed_disabled_and_independent_focus_in_every_theme() {
        for id in crate::ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(id), cx));
            let (h, cx) = app.add_window_view(|_, _| Harness {
                state: CheckState::Mixed,
                changes: 0,
                parent_clicks: 0,
                disabled: false,
            });
            draw(cx);
            let regular = cx.debug_bounds("setting").unwrap();
            let small = cx.debug_bounds("small").unwrap();
            let mini = cx.debug_bounds("mini").unwrap();
            assert!(
                regular.size.height > small.size.height && small.size.height > mini.size.height
            );
            // Far end of the label toggles too, not just the square.
            cx.simulate_click(
                point(regular.right() - px(2.), regular.center().y),
                Modifiers::default(),
            );
            draw(cx);
            cx.update(|_, cx| {
                assert_eq!(h.read(cx).state, CheckState::On);
                assert_eq!(h.read(cx).changes, 1);
                assert_eq!(h.read(cx).parent_clicks, 0);
            });
            key(cx, "space");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).state, CheckState::Off));
            key(cx, "enter");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).state, CheckState::On));
            key(cx, "ctrl-space");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).changes, 3));
            // Host Tab routing uses the same focus tree; disabled items are skipped.
            cx.update(|window, cx| window.focus_next(cx));
            draw(cx);
            let small_focus = cx.update(|window, cx| window.focused(cx));
            cx.update(|window, cx| window.focus_next(cx));
            draw(cx);
            let mini_focus = cx.update(|window, cx| window.focused(cx));
            assert_ne!(small_focus, mini_focus);
            cx.update(|window, cx| window.focus_next(cx));
            draw(cx);
            key(cx, "space");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).state, CheckState::Off));
            cx.update(|window, cx| window.focus_prev(cx));
            draw(cx);
            assert_eq!(cx.update(|window, cx| window.focused(cx)), mini_focus);
            cx.update(|_, cx| {
                h.update(cx, |h, cx| {
                    h.disabled = true;
                    cx.notify();
                })
            });
            draw(cx);
            cx.simulate_click(regular.center(), Modifiers::default());
            key(cx, "space");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).changes, 4));
        }
    }
}
