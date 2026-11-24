//! Disclosure triangle and disclosure button (HIG pp. 329-331;
//! M: disclosure.collapsed).
//!
//! Consumer owns expansion and label; activation requests expansion only.
//! Enter/Space use GPUI's keyboard click path; Left/Right collapse/expand.
//! The 13 px hit box is kept distinct from the measured 4.5 x 6 px glyph.
//! Rotation animates unless the app requests reduced motion.

use std::{cell::Cell, rc::Rc, time::Duration};

use crate::{convert::hsla, paint, ActiveTheme};
use gpui::{
    canvas, div, point, prelude::*, px, Animation, AnimationExt, App, ElementId, MouseButton,
    PathBuilder, SharedString, Window,
};

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// Whether the control is the bare triangle or the rounded-square disclosure
/// button (HIG p. 331).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DisclosureStyle {
    #[default]
    Triangle,
    Button,
}

#[derive(IntoElement)]
pub struct Disclosure {
    id: ElementId,
    label: SharedString,
    expanded: bool,
    disabled: bool,
    selected: bool,
    style: DisclosureStyle,
    on_toggle: Option<ToggleHandler>,
}

impl Disclosure {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            expanded: false,
            disabled: false,
            selected: false,
            style: DisclosureStyle::Triangle,
            on_toggle: None,
        }
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// On a selected source-list row the triangle is white (M: sidebar
    /// selection); elsewhere it is secondary text grey.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn style(mut self, style: DisclosureStyle) -> Self {
        self.style = style;
        self
    }

    /// Requests the new expansion state. Does not select a containing row.
    pub fn on_toggle(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

fn triangle_points(
    bounds: gpui::Bounds<gpui::Pixels>,
    angle: f32,
    width: f32,
    height: f32,
) -> Vec<gpui::Point<gpui::Pixels>> {
    // Collapsed points right; expanded points down. Linear interpolation turns
    // the measured right-pointing silhouette into the measured down one.
    let cx = bounds.left() + bounds.size.width / 2.0;
    let cy = bounds.top() + bounds.size.height / 2.0;
    let w = px(width);
    let h = px(height);
    let start = [
        point(cx - w * 0.36, cy - h / 2.0),
        point(cx - w * 0.36, cy + h / 2.0),
        point(cx + w * 0.64, cy),
    ];
    let end = [
        point(cx - h / 2.0, cy - w * 0.36),
        point(cx + h / 2.0, cy - w * 0.36),
        point(cx, cy + w * 0.64),
    ];
    start
        .iter()
        .zip(end.iter())
        .map(|(a, b)| point(a.x + (b.x - a.x) * angle, a.y + (b.y - a.y) * angle))
        .collect()
}

impl RenderOnce for Disclosure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let pressed = window
            .use_keyed_state(
                ElementId::Name(format!("{}-press", self.id).into()),
                cx,
                |_, _| Rc::new(Cell::new(false)),
            )
            .read(cx)
            .clone();
        let ring_focus = focus.clone();
        let mouse_focus = focus.clone();
        let expanded = self.expanded;
        let enabled = !self.disabled;
        let style = self.style;
        let color = if !enabled {
            theme.colors.text_disabled
        } else if self.selected {
            theme.lists.source_active.text
        } else {
            theme.colors.text_secondary
        };
        let pressed_text = theme.colors.text;
        let ring = theme.colors.focus_ring;
        let profile = theme.metrics.focus_ring;
        let click = self.on_toggle.clone();
        let arrow = self.on_toggle;
        let press_down = pressed.clone();
        let press_up = pressed.clone();
        let press_paint = pressed.clone();
        let down = styles_for(style, &theme);
        let down_for_paint = down;
        let angle = Rc::new(Cell::new(if expanded { 1.0 } else { 0.0 }));
        let paint_angle = angle.clone();
        let width = if style == DisclosureStyle::Button {
            theme.metrics.disclosure_width * 0.8
        } else {
            theme.metrics.disclosure_width
        };
        let height = if style == DisclosureStyle::Button {
            theme.metrics.disclosure_height * 0.8
        } else {
            theme.metrics.disclosure_height
        };
        let animated = enabled && !cx.reduce_motion();
        let glyph = canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let is_pressed = press_paint.get();
                let angle = paint_angle.get();
                if style == DisclosureStyle::Button {
                    let bevel = if !enabled {
                        down_for_paint.disabled
                    } else if is_pressed {
                        down_for_paint.pressed
                    } else {
                        down_for_paint.normal
                    };
                    paint::bevel(window, bounds, 4.0, &bevel);
                }
                let points = triangle_points(bounds, angle, width, height);
                let mut path = PathBuilder::fill();
                path.move_to(points[0]);
                path.line_to(points[1]);
                path.line_to(points[2]);
                path.close();
                if let Ok(path) = path.build() {
                    window.paint_path(
                        path,
                        hsla(if is_pressed && enabled {
                            color.mix(pressed_text, 0.35)
                        } else {
                            color
                        }),
                    );
                }
                if enabled && ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                    paint::focus_ring(window, bounds, 2.0, ring, profile);
                }
            },
        )
        .size_full();
        let element = div()
            .id(self.id.clone())
            .debug_selector({
                let id = self.id.clone();
                move || id.to_string()
            })
            .role(gpui::Role::Button)
            .aria_label(self.label)
            .aria_expanded(expanded)
            .relative()
            .flex_none()
            .size(px(theme.metrics.disclosure_size))
            .when(enabled, |el| {
                el.track_focus(&focus)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.focus(&mouse_focus, cx);
                        press_down.set(true);
                        window.refresh();
                        cx.stop_propagation();
                    })
                    .on_mouse_up(MouseButton::Left, move |_, window, _| {
                        if press_up.replace(false) {
                            window.refresh();
                        }
                    })
            })
            .when_some(click.filter(|_| enabled), |el, handler| {
                el.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler(&!expanded, window, cx);
                })
            })
            .when_some(arrow.filter(|_| enabled), |el, handler| {
                el.on_key_down(move |event, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let next = match event.keystroke.key.as_str() {
                        "left" => false,
                        "right" => true,
                        _ => return,
                    };
                    cx.stop_propagation();
                    if next != expanded {
                        handler(&next, window, cx);
                    }
                })
            })
            .child(if animated {
                let target = if expanded { 1.0 } else { 0.0 };
                let start = angle.get();
                let paint_angle = angle.clone();
                glyph
                    .with_animation(
                        "disclosure-rotation",
                        Animation::new(Duration::from_millis(120)),
                        move |el, t| {
                            paint_angle.set(start + (target - start) * t);
                            el
                        },
                    )
                    .into_any_element()
            } else {
                angle.set(if expanded { 1.0 } else { 0.0 });
                glyph.into_any_element()
            });
        element
    }
}

fn styles_for(
    style: DisclosureStyle,
    theme: &silvestris_core::Theme,
) -> silvestris_core::tokens::BevelStates {
    match style {
        DisclosureStyle::Triangle => theme.controls.push,
        DisclosureStyle::Button => theme.controls.push,
    }
}
