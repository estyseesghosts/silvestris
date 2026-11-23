//! Continuous horizontal slider (HIG pp. 299–301).
//! Input and accessibility use a normalized 0–1 value; scaling and persistence
//! are the caller's responsibility.

use std::{cell::Cell, rc::Rc};

use gpui::{
    canvas, div, point, prelude::*, px, size, App, Bounds, ElementId, FocusHandle, MouseButton,
    Pixels, SharedString, Window,
};

use crate::{paint, ActiveTheme};

const KNOB: f32 = 14.0;
type Change = Rc<dyn Fn(&f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    label: SharedString,
    value: f32,
    disabled: bool,
    on_change: Option<Change>,
}

struct SliderDrag(FocusHandle);

impl Slider {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, value: f32) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            value,
            disabled: false,
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

fn fraction(bounds: Bounds<Pixels>, x: Pixels) -> Option<f32> {
    let width = bounds.size.width.as_f32() - KNOB;
    (width > 0.0)
        .then(|| ((x - bounds.left()).as_f32() - KNOB / 2.0) / width)
        .map(|f| f.clamp(0.0, 1.0))
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focus: FocusHandle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let owner = focus.clone();
        let ring_focus = focus.clone();
        let mouse_focus = focus.clone();
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let click_bounds = bounds.clone();
        let drag_bounds = bounds.clone();
        let key_change = self.on_change.clone();
        let click_change = self.on_change.clone();
        let drag_change = self.on_change;
        let value = if self.value.is_finite() {
            self.value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let disabled = self.disabled;
        div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .role(gpui::Role::Slider)
            .aria_label(self.label)
            .aria_value(format!("{:.0}%", value * 100.0))
            .occlude()
            .w_full()
            .min_w(px(KNOB + 1.0))
            .h(px(20.0))
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if !disabled {
                    window.focus(&mouse_focus, cx);
                    if let (Some(change), Some(next)) = (
                        &click_change,
                        fraction(click_bounds.get(), event.position.x),
                    ) {
                        change(&next, window, cx);
                    }
                }
                cx.stop_propagation();
            })
            .when(!disabled, |el| {
                el.track_focus(&focus)
                    .on_key_down(move |event, window, cx| {
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        let next = match event.keystroke.key.as_str() {
                            "left" | "down" => value - 0.05,
                            "right" | "up" => value + 0.05,
                            "home" => 0.0,
                            "end" => 1.0,
                            _ => return,
                        };
                        cx.stop_propagation();
                        if let Some(change) = &key_change {
                            change(&next.clamp(0.0, 1.0), window, cx);
                        }
                    })
                    .on_drag(SliderDrag(owner.clone()), |_, _, _, cx| {
                        cx.new(|_| gpui::EmptyView)
                    })
                    .on_drag_move::<SliderDrag>(move |event, window, cx| {
                        if event.drag(cx).0 != owner
                            || event.event.pressed_button != Some(MouseButton::Left)
                        {
                            return;
                        }
                        if let (Some(change), Some(next)) = (
                            &drag_change,
                            fraction(drag_bounds.get(), event.event.position.x),
                        ) {
                            change(&next, window, cx);
                        }
                        cx.stop_propagation();
                    })
            })
            .child(
                canvas(
                    move |b, _, _| bounds.set(b),
                    move |b, _, window, _| {
                        let center = b.center();
                        let track = Bounds::new(
                            point(b.left() + px(KNOB / 2.0), center.y - px(2.0)),
                            size((b.size.width - px(KNOB)).max(px(0.0)), px(4.0)),
                        );
                        paint::bevel(window, track, 2.0, &theme.controls.slider_track);
                        let knob = Bounds::new(
                            point(
                                b.left() + (b.size.width - px(KNOB)) * value,
                                center.y - px(KNOB / 2.0),
                            ),
                            size(px(KNOB), px(KNOB)),
                        );
                        let bevel = if disabled {
                            theme.controls.push.disabled
                        } else {
                            theme.controls.slider_knob
                        };
                        paint::bevel(window, knob, KNOB / 2.0, &bevel);
                        if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                            paint::focus_ring(
                                window,
                                knob,
                                KNOB / 2.0,
                                theme.colors.focus_ring,
                                theme.metrics.focus_ring,
                            );
                        }
                    },
                )
                .size_full(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext};

    struct Harness(f32);

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            Slider::new("slider", "Scale", self.0).on_change(cx.listener(|view, next, _, cx| {
                view.0 = *next;
                cx.notify();
            }))
        }
    }

    #[test]
    fn pointer_and_keys_use_the_same_range() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (harness, cx): (_, &mut VisualTestContext) = app.add_window_view(|_, _| Harness(0.0));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let bounds = cx.debug_bounds("slider").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert!((harness.read(cx).0 - 0.5).abs() < 0.01);
        });
        cx.simulate_keystrokes("end right");
        cx.update(|_, cx| assert_eq!(harness.read(cx).0, 1.0));
        cx.simulate_keystrokes("home left");
        cx.update(|_, cx| assert_eq!(harness.read(cx).0, 0.0));
    }
}
