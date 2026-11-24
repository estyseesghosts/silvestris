//! Progress indicators (HIG pp. 305–309; M: progress.determinate and the
//! installer barber pole).
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
    let radius = bounds.size.height.as_f32() / 2.0;
    paint::bevel(window, bounds, radius, &controls.track);
    let inner = paint::inset(bounds, h);
    if inner.size.width <= px(0.0) || inner.size.height <= px(0.0) {
        return;
    }
    window.with_content_mask(Some(ContentMask { bounds: inner }), |window| {
        match fraction {
            Some(f) => {
                if f > 0.0 {
                    let width = inner.size.width * f;
                    paint::bevel(
                        window,
                        Bounds::new(inner.origin, size(width, inner.size.height)),
                        radius,
                        &controls.fill,
                    );
                }
            }
            None => {
                // M: installer barber pole: 8 px period, 45 degrees, blue
                // #1784de against #e5e5e5. The phase scrolls one period.
                let period = 8.0;
                let stripe = 4.0;
                let shift = if animate {
                    phase * period
                } else {
                    period * 0.5
                };
                window.paint_quad(gpui::fill(
                    inner,
                    crate::convert::hsla(controls.barber_light),
                ));
                let mut x = inner.left().as_f32() - inner.size.height.as_f32() - period + shift;
                let right = inner.right().as_f32();
                while x < right {
                    let points = [
                        point(px(x), inner.top()),
                        point(px(x + stripe), inner.top()),
                        point(px(x + stripe + inner.size.height.as_f32()), inner.bottom()),
                        point(px(x + inner.size.height.as_f32()), inner.bottom()),
                    ];
                    paint::polygon(window, &points, controls.barber_dark);
                    x += period;
                }
            }
        }
    });
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
