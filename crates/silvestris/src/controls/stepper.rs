//! Stepper ("little arrows", HIG p. 301): a paired up/down control placed to
//! the right of a text field that holds a number. The consumer owns the
//! value, step and bounds; the stepper only reports a direction.
//!
//! - Click the upper half: +1; the lower half: -1.
//! - Focused: Up/Down step. Disabled halves (at a bound) ignore input.
//! - 13 x 22 px regular, 11 x 19 small (estimates; no capture in the corpus).
use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, App, Bounds, ElementId, FocusHandle, KeyDownEvent,
    MouseButton, SharedString, Window,
};

use crate::controls::ControlSize;
use crate::convert::hsla;
use crate::{paint, ActiveTheme};

type StepFn = Rc<dyn Fn(&i32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Stepper {
    id: ElementId,
    label: SharedString,
    can_increase: bool,
    can_decrease: bool,
    size: ControlSize,
    on_step: Option<StepFn>,
}

impl Stepper {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            can_increase: true,
            can_decrease: true,
            size: ControlSize::Small,
            on_step: None,
        }
    }
    pub fn limits(mut self, can_decrease: bool, can_increase: bool) -> Self {
        self.can_decrease = can_decrease;
        self.can_increase = can_increase;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
    /// +1 for up, -1 for down.
    pub fn on_step(mut self, f: impl Fn(&i32, &mut Window, &mut App) + 'static) -> Self {
        self.on_step = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Stepper {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let (w, h) = match self.size {
            ControlSize::Regular => (13.0, 22.0),
            ControlSize::Small | ControlSize::Mini => (11.0, 19.0),
        };
        let enabled = self.can_increase || self.can_decrease;
        let focus: FocusHandle = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle().tab_stop(true))
            .read(cx)
            .clone();
        let states = theme.controls.push;
        let up = if self.can_increase { states.normal } else { states.disabled };
        let down = if self.can_decrease { states.normal } else { states.disabled };
        let ring_focus = focus.clone();
        let ring = theme.colors.focus_ring;
        let profile = theme.metrics.focus_ring;
        let half = |id: &'static str, dir: i32, ok: bool, glyph: &'static str, bevel: silvestris_core::tokens::Bevel| {
            let f = self.on_step.clone();
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .relative()
                .w(px(w))
                .h(px(h / 2.0))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(6.0))
                .text_color(hsla(bevel.text))
                .child(
                    canvas(|_, _, _| (), move |b, _, window, _| paint::bevel(window, b, 2.0, &bevel))
                        .absolute()
                        .inset_0(),
                )
                .child(div().relative().child(glyph))
                .when(ok, |el| {
                    el.on_click(move |_, window, cx| {
                        if let Some(f) = &f {
                            f(&dir, window, cx);
                        }
                    })
                })
        };
        let key_step = self.on_step.clone();
        let (inc, dec) = (self.can_increase, self.can_decrease);
        let mouse_focus = focus.clone();
        div()
            .debug_selector(|| self.id.to_string())
            .id(self.id.clone())
            .role(gpui::Role::SpinButton)
            .aria_label(self.label.clone())
            .occlude()
            .relative()
            .flex_none()
            .flex()
            .flex_col()
            .when(enabled, |el| {
                el.track_focus(&focus)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.focus(&mouse_focus, cx);
                        cx.stop_propagation();
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let dir = match event.keystroke.key.as_str() {
                            "up" if inc => 1,
                            "down" if dec => -1,
                            _ => return,
                        };
                        cx.stop_propagation();
                        if let Some(f) = &key_step {
                            f(&dir, window, cx);
                        }
                    })
            })
            .child(half("stepper-up", 1, self.can_increase, "\u{25B2}", up))
            .child(half("stepper-down", -1, self.can_decrease, "\u{25BC}", down))
            .child(
                canvas(|_, _, _| (), move |b: Bounds<gpui::Pixels>, _, window, _| {
                    if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                        paint::focus_ring(window, b, 2.0, ring, profile);
                    }
                })
                .absolute()
                .inset_0(),
            )
    }
}
