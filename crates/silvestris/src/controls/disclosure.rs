//! Disclosure triangle and disclosure button (HIG pp. 329-332;
//! M: disclosure.collapsed).
//!
//! Consumer owns expansion and label; activation requests expansion only.
//! Enter/Space use GPUI's keyboard click path; Left/Right collapse/expand.
//! The 13 px hit box is kept distinct from the measured 4.5 x 6 px glyph.
//! Rotation animates unless the app requests reduced motion.
//!
//! The two styles read the same state differently. A disclosure *triangle*
//! follows the hierarchy: right when collapsed, down when expanded. A
//! disclosure *button* is a pop-down control beside a list-based control:
//! it points down when collapsed and up when expanded (HIG pp. 331-332,
//! "Closed by default (pointing down); clicking expands the window and
//! points it up"). Both animate between their own two silhouettes.

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

/// The two silhouettes a disclosure rotates between, in quarter turns from the
/// measured right-pointing triangle: `0.0` points right, `1.0` points down,
/// `-1.0` points up.
///
/// A triangle is the hierarchy control: it is drawn right-pointing when
/// collapsed and turns a quarter turn to point down when expanded. A button is
/// the dialog pop-down control beside the field it affects (HIG pp. 331-332):
/// it is drawn pointing down when collapsed and turns half a turn to point up,
/// passing through the right-pointing state so one silhouette turns rather than
/// two shapes swapping.
const fn rotation(style: DisclosureStyle, expanded: bool) -> f32 {
    match (style, expanded) {
        (DisclosureStyle::Triangle, false) => 0.0,
        (DisclosureStyle::Triangle, true) => 1.0,
        (DisclosureStyle::Button, false) => 1.0,
        (DisclosureStyle::Button, true) => -1.0,
    }
}

/// Hit box and drawn glyph per style, at 1x.
///
/// `axis` is measured along the direction the triangle points; `spread` is
/// measured across it. The hierarchy triangle keeps the measured 13 px hit box
/// and 4.5 x 6 px glyph (M: disclosure.collapsed). The Save-sheet disclosure
/// button is its own bezelled control: 10.6 draws a 21 x 20 px bezel with a
/// 7 wide x 5 tall solid triangle inside it, pointing down when collapsed and
/// up when expanded (`dialogs--save-sheet--collapsed.png`, `--expanded.png`).
#[derive(Clone, Copy)]
struct Glyph {
    hit_width: f32,
    hit_height: f32,
    axis: f32,
    spread: f32,
    /// Apex distance from the centre along the pointing axis, as a fraction of
    /// `axis`. The measured hierarchy triangle sits forward of centre (its base
    /// is -0.36, its apex 0.64); the button glyph is symmetric.
    apex: f32,
}

impl Glyph {
    fn for_style(style: DisclosureStyle, theme: &silvestris_core::Theme) -> Self {
        match style {
            DisclosureStyle::Triangle => Self {
                hit_width: theme.metrics.disclosure_size,
                hit_height: theme.metrics.disclosure_size,
                axis: theme.metrics.disclosure_width,
                spread: theme.metrics.disclosure_height,
                apex: 0.64,
            },
            // M: panel.save.disclosure: 21 x 20 px bezel, 7 wide x 5 tall
            // glyph; the pointing axis is the 5 px one.
            DisclosureStyle::Button => Self {
                hit_width: 21.0,
                hit_height: 20.0,
                axis: 5.0,
                spread: 7.0,
                apex: 0.5,
            },
        }
    }
}

fn triangle_points(
    bounds: gpui::Bounds<gpui::Pixels>,
    angle: f32,
    axis: f32,
    spread: f32,
    apex: f32,
) -> Vec<gpui::Point<gpui::Pixels>> {
    // `angle` is a rotation in quarter turns: 0 points right, 1 points down,
    // -1 points up. Rotating the measured vertices keeps a proper triangle at
    // every intermediate angle, so an up glyph is the same glyph turned, not a
    // mirrored one.
    let cx = bounds.left() + bounds.size.width / 2.0;
    let cy = bounds.top() + bounds.size.height / 2.0;
    let (sin, cos) = (angle * std::f32::consts::FRAC_PI_2).sin_cos();
    let forward = axis * apex;
    let back = -axis * (1.0 - apex);
    let half_spread = spread / 2.0;
    let at = |along: f32, across: f32| {
        point(
            cx + px(cos * along - sin * across),
            cy + px(sin * along + cos * across),
        )
    };
    vec![
        at(forward, 0.0),
        at(back, half_spread),
        at(back, -half_spread),
    ]
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
        let target = rotation(style, expanded);
        let angle = Rc::new(Cell::new(target));
        let paint_angle = angle.clone();
        let glyph_metrics = Glyph::for_style(style, &theme);
        let axis = glyph_metrics.axis;
        let spread = glyph_metrics.spread;
        let hit_width = glyph_metrics.hit_width;
        let hit_height = glyph_metrics.hit_height;
        let apex = glyph_metrics.apex;
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
                let points = triangle_points(bounds, angle, axis, spread, apex);
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
            .w(px(hit_width))
            .h(px(hit_height))
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
                angle.set(target);
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

#[cfg(test)]
mod tests {
    use super::*;
    use silvestris_core::{ThemeId, ThemePreference};

    fn aqua() -> silvestris_core::Theme {
        let app = gpui::TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx);
        });
        app.update(|cx| cx.theme().as_ref().clone())
    }

    #[test]
    fn a_triangle_points_right_when_collapsed_and_down_when_expanded() {
        assert_eq!(rotation(DisclosureStyle::Triangle, false), 0.0);
        assert_eq!(rotation(DisclosureStyle::Triangle, true), 1.0);
    }

    #[test]
    fn a_button_points_down_when_collapsed_and_up_when_expanded() {
        assert_eq!(rotation(DisclosureStyle::Button, false), 1.0);
        assert_eq!(rotation(DisclosureStyle::Button, true), -1.0);
    }

    #[test]
    fn the_two_styles_never_share_a_collapsed_silhouette() {
        assert_ne!(
            rotation(DisclosureStyle::Triangle, false),
            rotation(DisclosureStyle::Button, false),
            "a global rotation would make source-list triangles point down"
        );
    }

    #[test]
    fn the_button_glyph_is_the_measured_7_by_5_triangle_in_a_21_by_20_bezel() {
        let theme = aqua();
        let button = Glyph::for_style(DisclosureStyle::Button, &theme);
        assert_eq!((button.hit_width, button.hit_height), (21.0, 20.0));
        assert_eq!((button.spread, button.axis), (7.0, 5.0));

        let triangle = Glyph::for_style(DisclosureStyle::Triangle, &theme);
        assert_eq!(triangle.hit_width, theme.metrics.disclosure_size);
        assert_eq!(triangle.hit_height, theme.metrics.disclosure_size);
        assert_eq!(triangle.axis, theme.metrics.disclosure_width);
        assert_eq!(triangle.spread, theme.metrics.disclosure_height);
    }

    #[test]
    fn the_triangle_keeps_its_silhouette_at_every_quarter_turn() {
        let bounds = gpui::Bounds::new(gpui::point(px(0.), px(0.)), gpui::size(px(13.), px(13.)));
        for angle in [0.0, 1.0, -1.0, 2.0] {
            let points = triangle_points(bounds, angle, 4.5, 6.0, 0.64);
            assert_eq!(points.len(), 3, "a quarter turn is still a triangle");
        }
        // Right-pointing: the apex is the rightmost vertex.
        let right = triangle_points(bounds, 0.0, 4.5, 6.0, 0.64);
        let apex = right
            .iter()
            .max_by(|a, b| a.x.partial_cmp(&b.x).unwrap())
            .unwrap();
        assert!(apex.x > bounds.center().x);
        // Down-pointing: the apex is the lowest vertex.
        let down = triangle_points(bounds, 1.0, 4.5, 6.0, 0.64);
        let apex = down
            .iter()
            .max_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap();
        assert!(apex.y > bounds.center().y);
        // Up-pointing: the apex is the highest vertex.
        let up = triangle_points(bounds, -1.0, 4.5, 6.0, 0.64);
        let apex = up
            .iter()
            .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
            .unwrap();
        assert!(apex.y < bounds.center().y);
    }
}
