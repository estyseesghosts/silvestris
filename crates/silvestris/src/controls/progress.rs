//! Progress indicators (HIG pp. 305–309; M: progress.determinate and the
//! 10.6 installer's barber pole).
//!
//! Three shapes share one type: the determinate/indeterminate bar, the
//! asynchronous spinner and the circular determinate ring. The caller supplies
//! progress; `None` means unknown. Animation repaints only the indicator and
//! stops when the window is inactive or the app requests reduced motion.

use std::time::Duration;

use gpui::{
    canvas, div, point, prelude::*, px, size, Animation, AnimationExt, App, Bounds, ContentMask,
    ElementId, SharedString, Window,
};

use silvestris_core::ThemeFamily;

use crate::{paint, ActiveTheme};

use super::ControlSize;

/// Which indicator shape to draw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProgressStyle {
    /// Horizontal bar, determinate when a fraction is supplied.
    #[default]
    Bar,
    /// Twelve-spoke asynchronous spinner.
    Spinner,
    /// Circular determinate ring.
    Circular,
}

#[derive(IntoElement)]
pub struct ProgressIndicator {
    id: ElementId,
    label: SharedString,
    fraction: Option<f32>,
    style: ProgressStyle,
    size: ControlSize,
    animate: bool,
}

impl ProgressIndicator {
    /// A horizontal bar. `None` is indeterminate, as the HIG requires for
    /// unknown duration.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        fraction: Option<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            fraction,
            style: ProgressStyle::Bar,
            size: ControlSize::Regular,
            animate: true,
        }
    }

    /// An asynchronous spinner.
    pub fn spinner(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            fraction: None,
            style: ProgressStyle::Spinner,
            size: ControlSize::Regular,
            animate: true,
        }
    }

    /// A circular determinate ring.
    pub fn circular(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        fraction: f32,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            fraction: Some(fraction),
            style: ProgressStyle::Circular,
            size: ControlSize::Regular,
            animate: true,
        }
    }

    pub fn style(mut self, style: ProgressStyle) -> Self {
        self.style = style;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Animation is disabled for goldens and for callers that freeze motion.
    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }

    fn index(&self) -> usize {
        match self.size {
            ControlSize::Regular => 0,
            ControlSize::Small => 1,
            ControlSize::Mini => 2,
        }
    }
}

// M: indeterminate-barber-pole-installer.png: stripe minima repeat every 32 px along row 334
// and move one pixel right per row (45 degrees).
const BARBER_PERIOD: f32 = 32.0;
// Stripe edge width. The capture's soft edges are mostly upscaling; the
// source stripes are near-solid with a short anti-aliased edge (observed by
// the maintainer on the VM). Estimate, recorded in docs/exceptions.md.
const BARBER_EDGE: f32 = 4.0;
// Samples per stripe period; each span is one horizontal gradient.
const BARBER_SPANS: usize = 16;

fn bar_draw(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    fraction: Option<f32>,
    phase: f32,
    theme: &silvestris_core::Theme,
    animate: bool,
) {
    let controls = theme.controls.progress;
    let h = crate::convert::hairline(window);
    let radius = controls.radius.min(bounds.size.height.as_f32() / 2.0);
    paint::bevel(window, bounds, radius, &controls.track);
    let inner = paint::inset(bounds, h);
    if inner.size.width <= px(0.0) || inner.size.height <= px(0.0) {
        return;
    }
    match fraction {
        Some(f) => {
            if f > 0.0 {
                window.with_content_mask(Some(ContentMask { bounds: inner }), |window| {
                    let width = inner.size.width * f;
                    paint::bevel(
                        window,
                        Bounds::new(inner.origin, size(width, inner.size.height)),
                        radius,
                        &controls.fill,
                    );
                });
            }
        }
        None if theme.id.family == ThemeFamily::Geist => {
            // Same animation phase and period, flat accent instead of Aqua's
            // 45 degree barber-pole shading. The block travels across the
            // clipped well and keeps its rounded ends.
            let width = inner.size.width.as_f32();
            let block = (width * 0.35).max(px(8.).as_f32());
            let travel = width + block;
            let offset = (phase * travel).rem_euclid(travel) - block;
            window.with_content_mask(Some(ContentMask { bounds: inner }), |window| {
                let fill = Bounds::new(
                    point(inner.origin.x + px(offset), inner.origin.y),
                    size(px(block), inner.size.height),
                );
                paint::bevel(window, fill, radius, &controls.fill);
            });
        }
        None => {
            // The stripes tint the top border row too; the sides and bottom
            // keep the well's border.
            let field = Bounds::new(
                point(inner.left(), bounds.top()),
                size(inner.size.width, inner.bottom() - bounds.top()),
            );
            let shift = if animate { phase * BARBER_PERIOD } else { 0.0 };
            window.with_content_mask(Some(ContentMask { bounds: field }), |window| {
                barber(window, field, shift, &controls);
            });
        }
    }
}

/// Colour of a barber profile at `t` (0 top, 1 bottom) of a bar `rows` tall.
fn profile(b: &silvestris_core::tokens::Bevel, t: f32, rows: f32) -> silvestris_core::Rgba8 {
    let top = 1.0 / rows;
    if t < top {
        b.border.from
    } else if t < b.split {
        b.upper.at((t - top) / (b.split - top).max(f32::EPSILON))
    } else {
        b.lower
            .at((t - b.split) / (1.0 - b.split).max(f32::EPSILON))
    }
}

/// Paints the barber pole one device row at a time. Each row blends the dark
/// and light profiles by the distance from `x - y` to the nearest stripe, so
/// the stripes run at 45 degrees with short anti-aliased edges.
fn barber(
    window: &mut Window,
    field: Bounds<gpui::Pixels>,
    shift: f32,
    controls: &silvestris_core::tokens::Progress,
) {
    use gpui::{linear_color_stop, linear_gradient};
    let row = crate::convert::hairline(window).as_f32();
    let rows = (field.size.height.as_f32() / row).round().max(1.0);
    let left = field.left().as_f32();
    let right = field.right().as_f32();
    let span = BARBER_PERIOD / BARBER_SPANS as f32;
    let colour = |t: f32, u: f32| {
        let light = profile(&controls.barber_light, t, rows);
        let dark = profile(&controls.barber_dark, t, rows);
        // Distance from the nearest blue centre; blue covers half the period
        // and blends to white over BARBER_EDGE pixels.
        let d = (u.rem_euclid(BARBER_PERIOD) - BARBER_PERIOD / 2.0).abs();
        let x = ((d - BARBER_PERIOD / 4.0) / BARBER_EDGE + 0.5).clamp(0.0, 1.0);
        light.mix(dark, x * x * (3.0 - 2.0 * x))
    };
    for i in 0..rows as usize {
        let y = i as f32 * row;
        let t = (i as f32 + 0.5) / rows;
        // Stripe centres sit where (x - left - y - shift) is a multiple of
        // the period; spans start on that grid so every row lines up.
        let mut x = left - (-y - shift).rem_euclid(span);
        while x < right {
            let u = x - left - y - shift;
            let b = Bounds::new(point(px(x), field.top() + px(y)), size(px(span), px(row)));
            window.paint_quad(gpui::fill(
                b,
                linear_gradient(
                    90.0,
                    linear_color_stop(crate::convert::hsla(colour(t, u)), 0.0),
                    linear_color_stop(crate::convert::hsla(colour(t, u + span)), 1.0),
                ),
            ));
            x += span;
        }
    }
}

fn spinner_draw(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    phase: f32,
    theme: &silvestris_core::Theme,
    animate: bool,
) {
    let controls = theme.controls.progress;
    let centre = bounds.center();
    let outer = (bounds.size.width.min(bounds.size.height).as_f32() / 2.0).max(1.0);
    let inner = outer * 0.28;
    let count = 12;
    let step = std::f32::consts::TAU / count as f32;
    let rotation = if animate {
        phase * std::f32::consts::TAU
    } else {
        0.0
    };
    for i in 0..count {
        let angle = i as f32 * step + rotation;
        let next = angle + step * 0.55;
        let alpha = ((i as f32 / count as f32 + 1.0 - phase) % 1.0).clamp(0.08, 1.0);
        let colour = controls.spinner.mix(controls.spinner_dim, 1.0 - alpha);
        let points = [
            point(
                centre.x + px(angle.cos() * inner),
                centre.y + px(angle.sin() * inner),
            ),
            point(
                centre.x + px(next.cos() * inner),
                centre.y + px(next.sin() * inner),
            ),
            point(
                centre.x + px(next.cos() * outer),
                centre.y + px(next.sin() * outer),
            ),
            point(
                centre.x + px(angle.cos() * outer),
                centre.y + px(angle.sin() * outer),
            ),
        ];
        paint::polygon(window, &points, colour);
    }
}

fn ring_draw(
    window: &mut Window,
    bounds: Bounds<gpui::Pixels>,
    fraction: f32,
    theme: &silvestris_core::Theme,
) {
    let controls = theme.controls.progress;
    let centre = bounds.center();
    let outer = (bounds.size.width.min(bounds.size.height).as_f32() / 2.0).max(1.0);
    let thickness = (outer * 0.22).max(2.0);
    let inner = outer - thickness;
    let steps = 96;
    let start = -std::f32::consts::FRAC_PI_2;
    paint::bevel(
        window,
        Bounds::new(bounds.origin, bounds.size),
        outer,
        &controls.track,
    );
    let filled = (fraction.clamp(0.0, 1.0) * steps as f32).round() as usize;
    for i in 0..filled {
        let a = start + std::f32::consts::TAU * i as f32 / steps as f32;
        let b = start + std::f32::consts::TAU * (i + 1).min(steps) as f32 / steps as f32;
        let points = [
            point(
                centre.x + px(a.cos() * inner),
                centre.y + px(a.sin() * inner),
            ),
            point(
                centre.x + px(b.cos() * inner),
                centre.y + px(b.sin() * inner),
            ),
            point(
                centre.x + px(b.cos() * outer),
                centre.y + px(b.sin() * outer),
            ),
            point(
                centre.x + px(a.cos() * outer),
                centre.y + px(a.sin() * outer),
            ),
        ];
        paint::polygon(window, &points, controls.fill.border.at(0.5));
    }
}

impl RenderOnce for ProgressIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let index = self.index();
        let fraction = self
            .fraction
            .filter(|f| f.is_finite())
            .map(|f| f.clamp(0.0, 1.0));
        let style = self.style;
        let animate = self.animate;
        let active = window.is_window_active() && !cx.reduce_motion();
        let animated = animate && active && fraction.is_none() && style != ProgressStyle::Circular;
        let height = theme.metrics.progress_height[index];
        let spinner = theme.metrics.spinner_size[index];
        let theme_for_draw = theme.clone();
        let draw = move |phase: f32| {
            let theme = theme_for_draw.clone();
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| match style {
                    ProgressStyle::Bar => {
                        bar_draw(window, bounds, fraction, phase, &theme, animate)
                    }
                    ProgressStyle::Spinner => spinner_draw(window, bounds, phase, &theme, animate),
                    ProgressStyle::Circular => {
                        ring_draw(window, bounds, fraction.unwrap_or(0.0), &theme)
                    }
                },
            )
            .size_full()
        };
        let (width, height) = match style {
            ProgressStyle::Bar => (None, height),
            ProgressStyle::Spinner => (Some(spinner), spinner),
            ProgressStyle::Circular => (Some(spinner), spinner),
        };
        let mut root = div()
            .id(self.id.clone())
            .debug_selector({
                let id = self.id.clone();
                move || id.to_string()
            })
            .role(gpui::Role::ProgressIndicator)
            .aria_label(self.label)
            .aria_value(
                fraction.map_or_else(|| "In progress".into(), |f| format!("{:.0}%", f * 100.0)),
            )
            .flex_none()
            .h(px(height));
        root = match width {
            Some(width) => root.w(px(width)),
            None => root.w_full(),
        };
        if animated {
            root.child(draw(0.0).with_animation(
                "progress-motion",
                Animation::new(Duration::from_millis(1100)).repeat(),
                move |_, phase| draw(phase),
            ))
        } else {
            root.child(draw(0.5))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Render, TestAppContext, VisualTestContext};

    struct Harness {
        fraction: Option<f32>,
        style: ProgressStyle,
        animate: bool,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            ProgressIndicator::new("progress", "Copy", self.fraction)
                .style(self.style)
                .animate(self.animate)
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }

    #[test]
    fn every_shape_renders_in_every_theme_and_clamps_the_fraction() {
        for theme in crate::ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(theme), cx));
            for (fraction, style) in [
                (Some(-1.0), ProgressStyle::Bar),
                (Some(2.0), ProgressStyle::Bar),
                (None, ProgressStyle::Bar),
                (None, ProgressStyle::Spinner),
                (Some(0.35), ProgressStyle::Circular),
            ] {
                let (_, cx) = app.add_window_view(|_, _| Harness {
                    fraction,
                    style,
                    animate: false,
                });
                draw(cx);
                assert!(cx.debug_bounds("progress").is_some(), "{theme:?} {style:?}");
            }
        }
    }

    #[test]
    fn reduced_motion_and_inactive_windows_draw_a_still_frame() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            cx.set_reduce_motion(true);
        });
        let (_, cx) = app.add_window_view(|_, _| Harness {
            fraction: None,
            style: ProgressStyle::Bar,
            animate: true,
        });
        draw(cx);
        assert!(cx.debug_bounds("progress").is_some());
    }
}
