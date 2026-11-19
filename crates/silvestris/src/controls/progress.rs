//! Progress indicators (HIG pp. 305–309).
//! The caller supplies progress and decides whether unknown progress animates.

use std::time::Duration;

use gpui::{
    Animation, AnimationExt, App, Bounds, ContentMask, ElementId, SharedString, Window, canvas,
    div, point, prelude::*, px, size,
};

use crate::{ActiveTheme, paint};

#[derive(IntoElement)]
pub struct ProgressIndicator {
    id: ElementId,
    label: SharedString,
    fraction: Option<f32>,
    animate: bool,
}

impl ProgressIndicator {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        fraction: Option<f32>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            fraction,
            animate: true,
        }
    }

    pub fn animate(mut self, animate: bool) -> Self {
        self.animate = animate;
        self
    }
}

impl RenderOnce for ProgressIndicator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let controls = cx.theme().controls;
        let fraction = self
            .fraction
            .filter(|f| f.is_finite())
            .map(|f| f.clamp(0.0, 1.0));
        let draw = move |phase: f32| {
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let radius = bounds.size.height.as_f32() / 2.0;
                    paint::bevel(window, bounds, radius, &controls.progress_track);
                    let (start, end) = match fraction {
                        Some(fraction) => (0.0, fraction),
                        None => (phase * 1.3 - 0.3, phase * 1.3),
                    };
                    let start = start.max(0.0);
                    let end = end.min(1.0);
                    if start < end {
                        let inner = paint::inset(bounds, crate::convert::hairline(window));
                        let fill = Bounds::new(
                            point(inner.origin.x + inner.size.width * start, inner.origin.y),
                            size(inner.size.width * (end - start), inner.size.height),
                        );
                        window.with_content_mask(Some(ContentMask { bounds: inner }), |window| {
                            paint::bevel(window, fill, radius, &controls.progress_fill);
                        });
                    }
                },
            )
            .size_full()
        };
        let animated =
            fraction.is_none() && self.animate && window.is_window_active() && !cx.reduce_motion();

        div()
            .id(self.id.clone())
            .role(gpui::Role::ProgressIndicator)
            .aria_label(self.label)
            .aria_value(
                fraction.map_or_else(|| "In progress".into(), |f| format!("{:.0}%", f * 100.0)),
            )
            .w_full()
            .h(px(12.0))
            .when(animated, |el| {
                el.child(draw(0.0).with_animation(
                    "progress-motion",
                    Animation::new(Duration::from_millis(1450)).repeat(),
                    move |_, phase| draw(phase),
                ))
            })
            .when(!animated, |el| el.child(draw(0.5)))
    }
}
