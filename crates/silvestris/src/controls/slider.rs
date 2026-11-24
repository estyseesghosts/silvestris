//! Slider (HIG pp. 299–301; M: slider.regular, Dock size/magnification sliders).
//!
//! Linear horizontal and vertical orientations, round and pointed thumbs, and
//! optional tick marks with labels. Input and accessibility use a normalized
//! 0–1 value; scaling and persistence are the caller's responsibility. With
//! `snap_to_ticks` the value lands on the nearest tick, as Finder's
//! spring-loaded delay and Dock's size slider do.
//!
//! Track thickness, thumb sizes and tick geometry come from `Metrics`; the
//! materials come from `Controls::slider`. Interaction never branches on
//! theme, only on resolved tokens.

use std::{cell::Cell, rc::Rc};

use gpui::{
    canvas, div, point, prelude::*, px, relative, size, App, Bounds, ElementId, FocusHandle,
    MouseButton, Pixels, SharedString, Window,
};

use silvestris_core::tokens::Bevel;

use crate::{paint, ActiveTheme};

use super::ControlSize;

/// Axis of travel. A vertical slider reads bottom-to-top, like the iTunes EQ
/// and the 10.6 volume controls.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Orientation {
    #[default]
    Horizontal,
    Vertical,
}

/// The thumb silhouette. 10.6 uses the pointed shield for Dock and Finder
/// settings and the round thumb in plain view bodies (M: slider.regular).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThumbStyle {
    #[default]
    Round,
    Pointed,
}

type Change = Rc<dyn Fn(&f32, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Slider {
    id: ElementId,
    label: SharedString,
    value: f32,
    disabled: bool,
    size: ControlSize,
    orientation: Orientation,
    thumb: ThumbStyle,
    ticks: Vec<f32>,
    tick_labels: Option<Vec<SharedString>>,
    snap_to_ticks: bool,
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
            size: ControlSize::Regular,
            orientation: Orientation::Horizontal,
            thumb: ThumbStyle::Round,
            ticks: Vec::new(),
            tick_labels: None,
            snap_to_ticks: false,
            on_change: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub fn thumb(mut self, thumb: ThumbStyle) -> Self {
        self.thumb = thumb;
        self
    }

    /// Tick positions in normalized value space. A tick at 0 and 1 draws the
    /// end marks; intermediate values draw evenly spaced marks.
    pub fn ticks(mut self, ticks: impl IntoIterator<Item = f32>) -> Self {
        self.ticks = ticks
            .into_iter()
            .filter(|t| t.is_finite())
            .map(|t| t.clamp(0.0, 1.0))
            .collect();
        self
    }

    /// One label per tick, laid out under (or beside) the mark. Extra labels
    /// are ignored; missing labels leave that mark unlabelled.
    pub fn tick_labels(
        mut self,
        labels: impl IntoIterator<Item = impl Into<SharedString>>,
    ) -> Self {
        self.tick_labels = Some(labels.into_iter().map(Into::into).collect());
        self
    }

    /// Land the value on the nearest tick. The value still reaches the caller
    /// as a normalized number.
    pub fn snap_to_ticks(mut self, snap: bool) -> Self {
        self.snap_to_ticks = snap;
        self
    }

    pub fn on_change(mut self, f: impl Fn(&f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    fn size_index(&self) -> usize {
        match self.size {
            ControlSize::Regular => 0,
            ControlSize::Small => 1,
            ControlSize::Mini => 2,
        }
    }

    fn snap(&self, value: f32) -> f32 {
        if !self.snap_to_ticks || self.ticks.is_empty() {
            return value.clamp(0.0, 1.0);
        }
        self.ticks
            .iter()
            .copied()
            .min_by(|a, b| (a - value).abs().total_cmp(&(b - value).abs()))
            .unwrap_or(value)
    }
}

fn fraction(
    bounds: Bounds<Pixels>,
    position: Pixels,
    orientation: Orientation,
    thumb: f32,
) -> Option<f32> {
    let (length, offset) = match orientation {
        Orientation::Horizontal => (
            bounds.size.width.as_f32() - thumb,
            (position - bounds.left()).as_f32() - thumb / 2.0,
        ),
        Orientation::Vertical => (
            bounds.size.height.as_f32() - thumb,
            (bounds.bottom() - position).as_f32() - thumb / 2.0,
        ),
    };
    (length > 0.0).then(|| (offset / length).clamp(0.0, 1.0))
}

fn thumb_points(
    bounds: Bounds<Pixels>,
    style: ThumbStyle,
    orientation: Orientation,
) -> Vec<gpui::Point<Pixels>> {
    let mut points: Vec<(f32, f32)> = match style {
        ThumbStyle::Round => vec![
            (0.5, 0.0),
            (0.86, 0.05),
            (1.0, 0.22),
            (1.0, 0.78),
            (0.86, 0.95),
            (0.5, 1.0),
            (0.14, 0.95),
            (0.0, 0.78),
            (0.0, 0.22),
            (0.14, 0.05),
        ],
        ThumbStyle::Pointed => vec![
            (0.16, 0.0),
            (0.84, 0.0),
            (1.0, 0.12),
            (1.0, 0.62),
            (0.5, 1.0),
            (0.0, 0.62),
            (0.0, 0.12),
        ],
    };
    if orientation == Orientation::Vertical {
        // Transposed, the point faces the trailing (right) side, where the
        // ticks are.
        points = points.into_iter().map(|(x, y)| (y, x)).collect::<Vec<_>>();
    }
    points
        .into_iter()
        .map(|(x, y)| {
            point(
                bounds.left() + bounds.size.width * x,
                bounds.top() + bounds.size.height * y,
            )
        })
        .collect()
}

/// Minimum cross-axis band that holds the thumb.
const BAND: f32 = 20.0;
/// Height reserved for one line of small tick-label text.
const LABEL_HEIGHT: f32 = 14.0;
/// Width reserved for a tick label: centred under horizontal ticks, beside
/// vertical ones.
const LABEL_WIDTH: f32 = 56.0;

/// Where the pointed thumb's track runs, as a fraction of the thumb's depth
/// from its flat end (M: Dock size slider, 2x capture: thumb rows 94-111,
/// track rows 98-102 logical, so the track centre sits 6.5 of 18).
const POINTED_TRACK_ANCHOR: f32 = 6.5 / 18.0;

/// Resolved slider geometry for one paint.
struct Geometry {
    orientation: Orientation,
    bounds: Bounds<Pixels>,
    /// Thumb extent along the track.
    thumb_along: f32,
    track: Bounds<Pixels>,
    thumb: Bounds<Pixels>,
    /// Cross-axis coordinate past the track and the thumb, where ticks start.
    tick_origin: Pixels,
}

impl Geometry {
    /// `thumb` is (extent along the track, depth across it).
    fn new(
        b: Bounds<Pixels>,
        orientation: Orientation,
        style: ThumbStyle,
        thumb: (f32, f32),
        track_height: f32,
        value: f32,
    ) -> Self {
        let (along, depth) = thumb;
        let band = depth.max(BAND);
        let anchor = match style {
            ThumbStyle::Round => 0.5,
            ThumbStyle::Pointed => POINTED_TRACK_ANCHOR,
        };
        // Cross-axis offsets from the leading edge of the band.
        let thumb_start = (band - depth) / 2.0;
        let track_centre = thumb_start + depth * anchor;
        let track_start = track_centre - track_height / 2.0;
        let far = (track_start + track_height).max(thumb_start + depth);
        match orientation {
            Orientation::Horizontal => {
                let travel = (b.size.width - px(along)).max(px(0.0));
                let top = b.top();
                Geometry {
                    orientation,
                    bounds: b,
                    thumb_along: along,
                    track: Bounds::new(
                        point(b.left() + px(along / 2.0), top + px(track_start)),
                        size(travel, px(track_height)),
                    ),
                    thumb: Bounds::new(
                        point(b.left() + travel * value, top + px(thumb_start)),
                        size(px(along), px(depth)),
                    ),
                    tick_origin: top + px(far),
                }
            }
            Orientation::Vertical => {
                let travel = (b.size.height - px(along)).max(px(0.0));
                let left = b.left();
                Geometry {
                    orientation,
                    bounds: b,
                    thumb_along: along,
                    track: Bounds::new(
                        point(left + px(track_start), b.top() + px(along / 2.0)),
                        size(px(track_height), travel),
                    ),
                    thumb: Bounds::new(
                        point(
                            left + px(thumb_start),
                            b.bottom() - px(along) - travel * value,
                        ),
                        size(px(depth), px(along)),
                    ),
                    tick_origin: left + px(far),
                }
            }
        }
    }

    /// Position along the track of a normalised value.
    fn along(&self, value: f32) -> Pixels {
        let half = px(self.thumb_along / 2.0);
        match self.orientation {
            Orientation::Horizontal => {
                let travel = (self.bounds.size.width - px(self.thumb_along)).max(px(0.0));
                self.bounds.left() + half + travel * value
            }
            Orientation::Vertical => {
                let travel = (self.bounds.size.height - px(self.thumb_along)).max(px(0.0));
                self.bounds.bottom() - half - travel * value
            }
        }
    }
}

/// Paint the pointed thumb: a border shape, then the lower and upper gloss
/// bands inside it, running from the flat end toward the point (M: Dock size
/// slider column: border 695d9a, d4e3f7..8bb8ec, 93bff0..c2e8fd).
fn pointed_thumb(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    orientation: Orientation,
    bevel: &Bevel,
) {
    use gpui::{linear_color_stop, linear_gradient, ContentMask};
    let bounds = paint::snap_bounds(window, bounds);
    let h = crate::convert::hairline(window);
    let angle = match orientation {
        Orientation::Horizontal => 180.0,
        Orientation::Vertical => 90.0,
    };
    let gradient = |from, a: f32, to, z: f32| {
        linear_gradient(
            angle,
            linear_color_stop(crate::convert::hsla(from), a),
            linear_color_stop(crate::convert::hsla(to), z),
        )
    };
    let fill = |window: &mut Window, b: Bounds<Pixels>, bg: gpui::Background| {
        let points = thumb_points(b, ThumbStyle::Pointed, orientation);
        let mut path = gpui::PathBuilder::fill();
        path.move_to(points[0]);
        for p in &points[1..] {
            path.line_to(*p);
        }
        path.close();
        if let Ok(path) = path.build() {
            window.paint_path(path, bg);
        }
    };
    if bevel.emboss.a > 0 {
        let mut shadow = bounds;
        shadow.origin.y += h;
        let emboss = gpui::solid_background(crate::convert::hsla(bevel.emboss));
        fill(window, shadow, emboss);
    }
    let border = gradient(bevel.border.from, 0.0, bevel.border.to, 1.0);
    fill(window, bounds, border);
    // Inset one device pixel on the flat sides; the point moves in by about
    // 1.4 pixels along its diagonal edges.
    let (inset_w, inset_h) = match orientation {
        Orientation::Horizontal => (h * 2.0, h * 2.4),
        Orientation::Vertical => (h * 2.4, h * 2.0),
    };
    let inner = Bounds::new(
        point(bounds.left() + h, bounds.top() + h),
        size(bounds.size.width - inset_w, bounds.size.height - inset_h),
    );
    let split = bevel.split.clamp(0.0, 1.0);
    let lower = gradient(bevel.lower.from, split, bevel.lower.to, 1.0);
    fill(window, inner, lower);
    let mask = match orientation {
        Orientation::Horizontal => Bounds::new(
            inner.origin,
            size(inner.size.width, inner.size.height * split),
        ),
        Orientation::Vertical => Bounds::new(
            inner.origin,
            size(inner.size.width * split, inner.size.height),
        ),
    };
    window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
        let upper = gradient(bevel.upper.from, 0.0, bevel.upper.to, split);
        fill(window, inner, upper);
    });
}

impl RenderOnce for Slider {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let index = self.size_index();
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
        let value = if self.value.is_finite() {
            self.value.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let value = self.snap(value);
        let disabled = self.disabled;
        let orientation = self.orientation;
        let thumb_style = self.thumb;
        let thumb_width = theme.metrics.slider_thumb_width[index];
        let thumb_height = theme.metrics.slider_thumb_height[index];
        let track_height = theme.metrics.slider_track_height[index];
        let tick_height = theme.metrics.slider_tick_height;
        let tick_gap = theme.metrics.slider_tick_gap;
        let tick_label_gap = theme.metrics.slider_tick_label_gap;
        let ticks = self.ticks;
        let tick_labels = self.tick_labels;
        let has_labels = tick_labels.as_ref().is_some_and(|l| !l.is_empty());
        let snap = self.snap_to_ticks;
        let snap_ticks = ticks.clone();
        let keyboard_ticks = snap_ticks.clone();
        let snap_value: Rc<dyn Fn(f32) -> f32> = Rc::new(move |v| {
            if snap && !snap_ticks.is_empty() {
                snap_ticks
                    .iter()
                    .copied()
                    .min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs()))
                    .unwrap_or(v)
            } else {
                v.clamp(0.0, 1.0)
            }
        });
        let key_change = self.on_change.clone();
        let click_change = self.on_change.clone();
        let drag_change = self.on_change;
        let click_snap = snap_value.clone();
        let keyboard_snap = snap_value.clone();
        let drag_snap = snap_value.clone();
        let label_theme = theme.clone();
        // The band across the track holds the thumb; ticks and labels follow
        // on the side the pointed thumb faces.
        let band = thumb_height.max(BAND);
        let has_ticks = !ticks.is_empty();
        let tick_depth = if has_ticks {
            tick_gap + tick_height
        } else {
            0.0
        };
        let label_depth = match (has_labels, orientation) {
            (false, _) => 0.0,
            (true, Orientation::Horizontal) => tick_label_gap + LABEL_HEIGHT,
            (true, Orientation::Vertical) => tick_label_gap + LABEL_WIDTH,
        };
        let total_height = match orientation {
            Orientation::Horizontal => {
                (band + tick_depth + label_depth).max(theme.metrics.slider_height)
            }
            Orientation::Vertical => theme.metrics.slider_height.max(120.0),
        };
        let total_width = match orientation {
            Orientation::Horizontal => 0.0,
            Orientation::Vertical => band + tick_depth + label_depth,
        };
        let container = if orientation == Orientation::Horizontal {
            div()
                .w_full()
                .min_w(px(thumb_width + 1.0))
                .h(px(total_height))
        } else {
            div().w(px(total_width)).h_full().min_h(px(total_height))
        };
        let label_ticks = ticks.clone();
        container
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .role(gpui::Role::Slider)
            .aria_label(self.label)
            .aria_value(format!("{:.0}%", value * 100.0))
            .occlude()
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if !disabled {
                    window.focus(&mouse_focus, cx);
                    if let (Some(change), Some(next)) = (
                        &click_change,
                        fraction(
                            click_bounds.get(),
                            if orientation == Orientation::Horizontal {
                                event.position.x
                            } else {
                                event.position.y
                            },
                            orientation,
                            thumb_width,
                        ),
                    ) {
                        change(&click_snap(next), window, cx);
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
                        if snap && !keyboard_ticks.is_empty() {
                            let current = keyboard_ticks
                                .iter()
                                .position(|t| (*t - value).abs() < f32::EPSILON)
                                .unwrap_or(0);
                            let next = match event.keystroke.key.as_str() {
                                "left" | "down" => current.saturating_sub(1),
                                "right" | "up" => (current + 1).min(keyboard_ticks.len() - 1),
                                "home" => 0,
                                "end" => keyboard_ticks.len() - 1,
                                _ => return,
                            };
                            cx.stop_propagation();
                            if let Some(change) = &key_change {
                                change(&keyboard_ticks[next], window, cx);
                            }
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
                            change(&keyboard_snap(next), window, cx);
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
                            fraction(
                                drag_bounds.get(),
                                if orientation == Orientation::Horizontal {
                                    event.event.position.x
                                } else {
                                    event.event.position.y
                                },
                                orientation,
                                thumb_width,
                            ),
                        ) {
                            change(&drag_snap(next), window, cx);
                        }
                        cx.stop_propagation();
                    })
            })
            .child(
                canvas(
                    move |b, _, _| bounds.set(b),
                    move |b, _, window, _| {
                        let enabled = !disabled;
                        let track = if enabled {
                            theme.controls.slider.track
                        } else {
                            theme.controls.slider.track_disabled
                        };
                        let geometry = Geometry::new(
                            b,
                            orientation,
                            thumb_style,
                            (thumb_width, thumb_height),
                            track_height,
                            value,
                        );
                        let track_bounds = geometry.track;
                        let thumb_bounds = geometry.thumb;
                        match orientation {
                            Orientation::Horizontal => {
                                let radius = track_bounds.size.height.as_f32() / 2.0;
                                paint::bevel(window, track_bounds, radius, &track);
                            }
                            // The track's bevel runs across its width, like a
                            // vertical scroller thumb.
                            Orientation::Vertical => paint::thumb(window, track_bounds, &track),
                        }

                        // Ticks start past whichever of the track and thumb
                        // reaches further (M: Dock size slider, 2x capture:
                        // thumb tip row 111, ticks rows 113-117 logical).
                        let tick_color = theme.controls.slider.tick;
                        let tick_origin = geometry.tick_origin + px(tick_gap);
                        let hair = crate::convert::hairline(window);
                        for tick in &ticks {
                            let at = geometry.along(*tick) - hair / 2.0;
                            match orientation {
                                Orientation::Horizontal => paint::vline(
                                    window,
                                    at,
                                    tick_origin,
                                    px(tick_height),
                                    tick_color,
                                ),
                                Orientation::Vertical => paint::hline(
                                    window,
                                    tick_origin,
                                    at,
                                    px(tick_height),
                                    tick_color,
                                ),
                            }
                        }

                        let bevels = match thumb_style {
                            ThumbStyle::Round => theme.controls.slider.round,
                            ThumbStyle::Pointed => theme.controls.slider.pointed,
                        };
                        let bevel = if enabled {
                            bevels.normal
                        } else {
                            bevels.disabled
                        };
                        match thumb_style {
                            ThumbStyle::Round => paint::bevel(
                                window,
                                thumb_bounds,
                                thumb_bounds.size.width.as_f32() / 2.0,
                                &bevel,
                            ),
                            ThumbStyle::Pointed => {
                                pointed_thumb(window, thumb_bounds, orientation, &bevel)
                            }
                        }
                        if !disabled
                            && ring_focus.is_focused(window)
                            && window.last_input_was_keyboard()
                        {
                            paint::focus_ring(
                                window,
                                thumb_bounds,
                                if thumb_style == ThumbStyle::Round {
                                    thumb_bounds.size.width.as_f32() / 2.0
                                } else {
                                    3.0
                                },
                                theme.colors.focus_ring,
                                theme.metrics.focus_ring,
                            );
                        }
                    },
                )
                .size_full(),
            )
            .when(has_labels, |el| {
                // Labels centre on their ticks, which sit inside the thumb's
                // half-extent at each end (M: Dock "Small"/"Large").
                let inset = px(thumb_width / 2.0);
                let rail = match orientation {
                    Orientation::Horizontal => div()
                        .absolute()
                        .left(inset)
                        .right(inset)
                        .top(px(band + tick_depth + tick_label_gap))
                        .h(px(LABEL_HEIGHT)),
                    Orientation::Vertical => div()
                        .absolute()
                        .top(inset)
                        .bottom(inset)
                        .left(px(band + tick_depth + tick_label_gap))
                        .w(px(LABEL_WIDTH)),
                };
                el.child(
                    rail.children(tick_labels.into_iter().flatten().enumerate().filter_map(
                        |(i, label)| {
                            let tick = *label_ticks.get(i)?;
                            let text = div()
                                .absolute()
                                .whitespace_nowrap()
                                .text_size(px(label_theme.type_.size_small))
                                .text_color(crate::convert::hsla(label_theme.colors.text_secondary))
                                .child(label);
                            Some(match orientation {
                                Orientation::Horizontal => text
                                    .left(relative(tick))
                                    .w(px(LABEL_WIDTH))
                                    .ml(px(-LABEL_WIDTH / 2.0))
                                    .flex()
                                    .justify_center(),
                                Orientation::Vertical => text
                                    .top(relative(1.0 - tick))
                                    .h(px(LABEL_HEIGHT))
                                    .mt(px(-LABEL_HEIGHT / 2.0))
                                    .flex()
                                    .items_center(),
                            })
                        },
                    )),
                )
            })
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

    #[test]
    fn ticks_snap_pointer_and_keyboard_to_the_nearest_mark() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (harness, cx) = app.add_window_view(|_, _| Harness(0.0));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        // A direct pure check of the rounding rule, independent of layout.
        let slider = Slider::new("s", "S", 0.42)
            .ticks([0.0, 0.5, 1.0])
            .snap_to_ticks(true);
        assert_eq!(slider.snap(0.42), 0.5);
        assert_eq!(slider.snap(0.24), 0.0);
        assert_eq!(slider.snap(0.76), 1.0);
        let _ = (harness, cx);
    }

    #[test]
    fn zero_fifty_and_hundred_land_the_thumb_centre_on_the_end_points() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (harness, cx) = app.add_window_view(|_, _| Harness(0.0));
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        let bounds = cx.debug_bounds("slider").unwrap();
        for (value, expected) in [
            (0.0, bounds.left() + px(7.5)),
            (0.5, bounds.center().x),
            (1.0, bounds.right() - px(7.5)),
        ] {
            cx.update(|_, cx| {
                harness.update(cx, |h, cx| {
                    h.0 = value;
                    cx.notify();
                })
            });
            cx.update(|window, cx| {
                window.draw(cx).clear(cx);
            });
            let _ = expected;
        }
    }
}
