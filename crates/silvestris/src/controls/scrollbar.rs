//! Scroll bars (HIG pp. 225-227) for any [`ScrollSource`].
//!
//! One painter and one pointer model serve both axes and every source
//! (`ScrollHandle`, `UniformListScrollHandle`, `ListState`). Most views should
//! use [`ScrollView`](super::ScrollView), which lays the bars and the corner
//! out; a bare `Scrollbar` is for custom layouts.
//!
//! - Thumb length is proportional to the visible fraction (HIG p. 226).
//! - Dragging the thumb scrolls live.
//! - A track click pages by a windowful, the viewport minus one line, or
//!   jumps to the clicked spot, per [`TrackClick`] (HIG p. 226, Appearance
//!   preferences). Holding pages until the thumb reaches the pointer.
//! - Arrow clicks move one line; holding repeats.
//! - Arrows sit together at the far end (the 10.6 default) or one at each
//!   end, per [`ScrollArrows`].
//! - The thumb is graphite in inactive windows (theme `thumb_inactive`).
//! - With nothing to scroll the bar either collapses
//!   ([`autohide`](Scrollbar::autohide)) or shows an empty track (HIG p. 226).
//!
//! Geometry for a bar of thickness `w` (M: scroller.arrows,
//! scroller.thumb.row, scroller.horizontal, scroller.cap; Console and Finder
//! captures): each arrow box is `w + 1` long; together, the track ends
//! `2(w + 1) + 2` before the far end, with a divider `w + 2` before it;
//! the 12 rows before the arrows darken towards `cap_shadow`, and the last
//! 6 rows of the bar darken slightly. Arrows-at-ends, small and mini sizes,
//! the line step, minimum thumb and repeat timing are estimates
//! (docs/exceptions.md).
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    canvas, div, point, prelude::*, px, App, Axis, Bounds, DispatchPhase, ElementId, Entity,
    Global, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, Point,
    Task, Window,
};
use silvestris_core::Rgba8;

use super::scroll_source::{self, ScrollSource};
use super::ControlSize;
use crate::convert::{across, hsla, rows};
use crate::{paint, ActiveTheme};

/// Estimate: one arrow click scrolls one 16 px line unless the view sets
/// its own unit (HIG p. 226: "your application determines what one unit
/// equals").
pub const DEFAULT_LINE: f32 = 16.0;
/// Estimate: shortest thumb that keeps both caps visible.
const MIN_THUMB: f32 = 18.0;
/// Estimates for press-and-hold repeat.
const REPEAT_DELAY: Duration = Duration::from_millis(350);
const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

/// M: scroller.cap: opacity of each of the 12 rows before the arrow boxes,
/// as a fraction of `cap_shadow` (Finder vertical and horizontal bars
/// averaged; the last row is the darkest).
const CAP_PROFILE: [f32; 12] = [
    0.017, 0.034, 0.045, 0.080, 0.119, 0.159, 0.222, 0.267, 0.341, 0.469, 0.727, 1.0,
];
/// M: scroller.boxes: black opacity across the divider between the boxes.
const DIVIDER_PROFILE: [f32; 15] = [
    0.10, 0.14, 0.17, 0.21, 0.24, 0.25, 0.25, 0.25, 0.25, 0.24, 0.21, 0.18, 0.15, 0.10, 0.07,
];
/// M: scroller.cap: black opacity of the bar's last 6 rows.
const END_PROFILE: [f32; 6] = [0.004, 0.009, 0.013, 0.019, 0.035, 0.132];

/// Where the scroll arrows sit (Appearance preferences).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollArrows {
    /// Both arrows at the bottom or right end: the 10.6 default, and what
    /// every corpus capture shows.
    #[default]
    Together,
    /// One arrow at each end.
    AtEnds,
}

/// What a click in the track does (HIG p. 226).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrackClick {
    /// Advance by a windowful: the default.
    #[default]
    JumpToPage,
    /// Move the thumb under the pointer, then follow it as a drag.
    JumpToSpot,
}

/// The user's scroll bar preferences, shared by every scroller in the app.
/// Install with [`ScrollerPreferences::set`]; until then the 10.6 defaults
/// apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScrollerPreferences {
    pub arrows: ScrollArrows,
    pub track_click: TrackClick,
}

impl Global for ScrollerPreferences {}

impl ScrollerPreferences {
    /// The preferences in effect.
    pub fn get(cx: &App) -> Self {
        cx.try_global::<Self>().copied().unwrap_or_default()
    }

    /// Replace the preferences and repaint every window.
    pub fn set(self, cx: &mut App) {
        cx.set_global(self);
        cx.refresh_windows();
    }
}

#[derive(Default)]
struct State {
    /// Pointer offset from the thumb's leading edge while dragging.
    grab: Option<Pixels>,
    repeat: Option<Task<()>>,
}

/// Positions along the bar, measured from its leading edge.
#[derive(Clone, Copy, Debug)]
struct Layout {
    track_start: Pixels,
    track_end: Pixels,
    /// Arrow boxes as `(start, end, direction)`; direction -1 scrolls back.
    boxes: [(Pixels, Pixels, f32); 2],
    /// Divider between adjacent boxes.
    divider: Option<Pixels>,
}

fn layout(length: Pixels, thickness: f32, arrows: ScrollArrows) -> Layout {
    let unit = px(thickness + 1.0);
    match arrows {
        ScrollArrows::Together => Layout {
            track_start: px(0.),
            track_end: (length - unit * 2.0 - px(2.)).max(px(0.)),
            boxes: [
                (length - unit * 2.0, length - unit - px(1.), -1.0),
                (length - unit, length, 1.0),
            ],
            divider: Some(length - unit - px(1.)),
        },
        ScrollArrows::AtEnds => Layout {
            track_start: unit + px(1.),
            track_end: (length - unit - px(2.)).max(unit + px(1.)),
            boxes: [(px(0.), unit, -1.0), (length - unit, length, 1.0)],
            divider: None,
        },
    }
}

#[derive(Clone, Copy, Debug)]
struct Geometry {
    track_start: Pixels,
    track_length: Pixels,
    /// Thumb span along the bar, from the bar's leading edge.
    thumb: (Pixels, Pixels),
    max: Pixels,
}

fn geometry(length: Pixels, l: &Layout, source: &dyn ScrollSource, axis: Axis) -> Option<Geometry> {
    let max = scroll_source::along(source.max_position(), axis);
    let viewport = scroll_source::extent(source.viewport(), axis);
    if max <= px(0.) || viewport <= px(0.) || length <= px(0.) {
        return None;
    }
    let track_length = l.track_end - l.track_start;
    if track_length < px(MIN_THUMB) {
        return None;
    }
    let thumb_length = (track_length * (viewport / (viewport + max)))
        .max(px(MIN_THUMB))
        .min(track_length);
    let fraction = (scroll_source::along(source.position(), axis) / max).clamp(0.0, 1.0);
    let start = l.track_start + (track_length - thumb_length) * fraction;
    Some(Geometry {
        track_start: l.track_start,
        track_length,
        thumb: (start, start + thumb_length),
        max,
    })
}

/// Repeat `step` after the hold delay until it returns false or the mouse
/// is released (dropping the task cancels it).
fn hold(
    state: &Entity<State>,
    window: &mut Window,
    cx: &mut App,
    step: impl Fn(&mut Window) -> bool + 'static,
) {
    let executor = cx.background_executor().clone();
    let task = window.spawn(cx, async move |cx| {
        executor.timer(REPEAT_DELAY).await;
        loop {
            let more = cx.update(|window, _| {
                let more = step(window);
                window.refresh();
                more
            });
            if !matches!(more, Ok(true)) {
                break;
            }
            executor.timer(REPEAT_INTERVAL).await;
        }
    });
    state.update(cx, |s, _| s.repeat = Some(task));
}

/// A scroll bar for one axis of a [`ScrollSource`].
#[derive(IntoElement)]
pub struct Scrollbar {
    id: ElementId,
    source: Rc<dyn ScrollSource>,
    axis: Axis,
    size: ControlSize,
    line: f32,
    autohide: bool,
}

impl Scrollbar {
    /// A vertical bar. The bar collapses while there is nothing to scroll.
    pub fn new<S: ScrollSource + Clone>(id: impl Into<ElementId>, source: &S) -> Self {
        Self::from_rc(id.into(), Rc::new(source.clone()), Axis::Vertical)
    }

    /// A horizontal bar.
    pub fn horizontal<S: ScrollSource + Clone>(id: impl Into<ElementId>, source: &S) -> Self {
        Self::from_rc(id.into(), Rc::new(source.clone()), Axis::Horizontal)
    }

    pub(crate) fn from_rc(id: ElementId, source: Rc<dyn ScrollSource>, axis: Axis) -> Self {
        Self {
            id,
            source,
            axis,
            size: ControlSize::Regular,
            line: DEFAULT_LINE,
            autohide: true,
        }
    }

    /// Small and mini bars are for panels whose other controls are small or
    /// mini too (HIG p. 227).
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Distance one arrow click scrolls, and the overlap kept when paging.
    pub fn line(mut self, line: f32) -> Self {
        self.line = line;
        self
    }

    /// `false` keeps the empty track visible while everything fits
    /// (HIG p. 226); the default collapses the bar.
    pub fn autohide(mut self, autohide: bool) -> Self {
        self.autohide = autohide;
        self
    }
}

/// Thickness of a bar of `size` in the active theme.
pub fn thickness(size: ControlSize, cx: &App) -> f32 {
    let widths = cx.theme().metrics.scroller_width;
    match size {
        ControlSize::Regular => widths[0],
        ControlSize::Small => widths[1],
        ControlSize::Mini => widths[2],
    }
}

/// Whether `source` can scroll along `axis`.
pub fn scrollable(source: &dyn ScrollSource, axis: Axis) -> bool {
    scroll_source::along(source.max_position(), axis) > px(0.)
}

impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let width = thickness(self.size, cx);
        let style = theme.controls.scroller;
        let prefs = ScrollerPreferences::get(cx);
        let axis = self.axis;
        let line = px(self.line);
        let source = self.source;
        let shown = !self.autohide || scrollable(source.as_ref(), axis);
        let thickness_px = if shown { px(width) } else { px(0.) };
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        let debug = match axis {
            Axis::Vertical => "scrollbar",
            Axis::Horizontal => "scrollbar-horizontal",
        };
        div()
            .id(self.id)
            .debug_selector(move || debug.into())
            .flex_none()
            .map(|el| match axis {
                Axis::Vertical => el.h_full().w(thickness_px),
                Axis::Horizontal => el.w_full().h(thickness_px),
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bar, _, window, _| {
                        if bar.size.width <= px(0.) || bar.size.height <= px(0.) {
                            return;
                        }
                        let length = scroll_source::extent(bar.size, axis);
                        let l = layout(length, width, prefs.arrows);
                        // Map (along, across) offsets into window coordinates.
                        let at = move |t: Pixels, a: Pixels| -> Point<Pixels> {
                            match axis {
                                Axis::Vertical => point(bar.origin.x + a, bar.origin.y + t),
                                Axis::Horizontal => point(bar.origin.x + t, bar.origin.y + a),
                            }
                        };
                        let span = move |t0: Pixels, t1: Pixels, a0: Pixels, a1: Pixels| {
                            Bounds::from_corners(at(t0, a0), at(t1, a1))
                        };
                        let along_of = move |p: Point<Pixels>| -> Pixels {
                            match axis {
                                Axis::Vertical => p.y - bar.origin.y,
                                Axis::Horizontal => p.x - bar.origin.x,
                            }
                        };
                        let full = px(width);

                        // Track, then arrow boxes, across the bar (M:
                        // scroller.track.row, scroller.boxes,
                        // scroller.horizontal: both axes share the profiles).
                        if style.track_rows.is_empty() {
                            let track = match axis {
                                Axis::Vertical => across(style.track, width),
                                Axis::Horizontal => rows(style.track, width),
                            };
                            window.paint_quad(gpui::fill(bar, track));
                        } else {
                            paint_rows(window, &span, (px(0.), length), width, style.track_rows);
                        }
                        if !style.arrow_rows.is_empty() {
                            for (start, end, _) in l.boxes {
                                paint_rows(window, &span, (start, end), width, style.arrow_rows);
                            }
                        }
                        paint_shading(window, &span, &l, length, full, style.cap_shadow);
                        if let Some(d) = l.divider {
                            if style.arrow_rows.is_empty() {
                                let inset = px((width * 0.2).round());
                                window.paint_quad(gpui::fill(
                                    span(d, d + px(1.), inset, full - inset),
                                    hsla(style.track_border),
                                ));
                            } else {
                                // M: scroller.boxes divider column: the boxes
                                // darkened by a quarter mid-bar, less at the edges.
                                let shades: Vec<Rgba8> = DIVIDER_PROFILE
                                    .iter()
                                    .map(|a| Rgba8::hex(0x000000).alpha((a * 255.0).round() as u8))
                                    .collect();
                                paint_rows(window, &span, (d, d + px(1.)), width, &shades);
                            }
                        }
                        if l.track_start > px(0.) {
                            let edge = l.track_start - px(1.);
                            window.paint_quad(gpui::fill(
                                span(edge, edge + px(1.), px(0.), full),
                                hsla(style.track_border),
                            ));
                        }
                        let scale = width / 15.0;
                        for (start, end, direction) in l.boxes {
                            // M: scroller.arrows: 7 x 6 px glyph, apex 5 px
                            // in from the box's outer end.
                            let (apex, base) = if direction < 0.0 {
                                (start + px(4.0 * scale), start + px(10.0 * scale))
                            } else {
                                (end - px(5.0 * scale), end - px(11.0 * scale))
                            };
                            let centre = full / 2.0;
                            let half = px(3.5 * scale);
                            let mut path = PathBuilder::fill();
                            path.move_to(at(apex, centre));
                            path.line_to(at(base, centre + half));
                            path.line_to(at(base, centre - half));
                            path.close();
                            if let Ok(path) = path.build() {
                                window.paint_path(path, hsla(style.arrow));
                            }
                        }

                        let current = geometry(length, &l, source.as_ref(), axis);
                        if let Some(g) = current {
                            let bevel = if window.is_window_active() {
                                style.thumb
                            } else {
                                style.thumb_inactive
                            };
                            let thumb = span(g.thumb.0, g.thumb.1, px(1.), full - px(1.));
                            match axis {
                                Axis::Vertical => paint::thumb(window, thumb, &bevel),
                                Axis::Horizontal => {
                                    let radius = thumb.size.height.as_f32() / 2.0;
                                    paint::bevel(window, thumb, radius, &bevel);
                                }
                            }
                        }

                        let down_state = state.clone();
                        let down_source = source.clone();
                        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                            if phase != DispatchPhase::Bubble
                                || e.button != MouseButton::Left
                                || !bar.contains(&e.position)
                            {
                                return;
                            }
                            cx.stop_propagation();
                            let t = along_of(e.position);
                            let src = down_source.clone();
                            if let Some(&(_, _, direction)) =
                                l.boxes.iter().find(|(s, e, _)| t >= *s && t < *e)
                            {
                                scroll_source::scroll_by(src.as_ref(), axis, line * direction);
                                hold(&down_state, window, cx, move |_| {
                                    scroll_source::scroll_by(src.as_ref(), axis, line * direction)
                                });
                                window.refresh();
                                return;
                            }
                            let Some(g) = current else {
                                return;
                            };
                            let thumb_length = g.thumb.1 - g.thumb.0;
                            if t >= g.thumb.0 && t < g.thumb.1 {
                                src.drag_started();
                                down_state.update(cx, |s, _| s.grab = Some(t - g.thumb.0));
                            } else if t >= l.track_start && t < l.track_end {
                                match prefs.track_click {
                                    TrackClick::JumpToSpot => {
                                        let room = g.track_length - thumb_length;
                                        let lead = t - thumb_length / 2.0 - g.track_start;
                                        if room > px(0.) {
                                            let fraction: f32 = lead / room;
                                            let target = g.max * fraction.clamp(0.0, 1.0);
                                            scroll_source::scroll_to(src.as_ref(), axis, target);
                                        }
                                        src.drag_started();
                                        down_state
                                            .update(cx, |s, _| s.grab = Some(thumb_length / 2.0));
                                    }
                                    TrackClick::JumpToPage => {
                                        let direction = if t < g.thumb.0 { -1.0 } else { 1.0 };
                                        let page = scroll_source::page(src.as_ref(), axis, line);
                                        scroll_source::scroll_by(
                                            src.as_ref(),
                                            axis,
                                            page * direction,
                                        );
                                        hold(&down_state, window, cx, move |_| {
                                            // Stop once the thumb reaches the pointer.
                                            let Some(now) =
                                                geometry(length, &l, src.as_ref(), axis)
                                            else {
                                                return false;
                                            };
                                            let reached = if direction < 0.0 {
                                                now.thumb.0 <= t
                                            } else {
                                                now.thumb.1 >= t
                                            };
                                            if reached {
                                                return false;
                                            }
                                            let page =
                                                scroll_source::page(src.as_ref(), axis, line);
                                            scroll_source::scroll_by(
                                                src.as_ref(),
                                                axis,
                                                page * direction,
                                            )
                                        });
                                    }
                                }
                            }
                            window.refresh();
                        });
                        let move_state = state.clone();
                        let move_source = source.clone();
                        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let Some(grab) = move_state.read(cx).grab else {
                                return;
                            };
                            // Re-read the range: content may have grown.
                            let Some(g) = geometry(length, &l, move_source.as_ref(), axis) else {
                                return;
                            };
                            let room = g.track_length - (g.thumb.1 - g.thumb.0);
                            if room <= px(0.) {
                                return;
                            }
                            let lead = along_of(e.position) - grab - g.track_start;
                            let fraction: f32 = lead / room;
                            let target = g.max * fraction.clamp(0.0, 1.0);
                            if scroll_source::scroll_to(move_source.as_ref(), axis, target) {
                                window.refresh();
                            }
                        });
                        let up_state = state.clone();
                        let up_source = source.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                            if phase != DispatchPhase::Capture {
                                return;
                            }
                            let dragging = up_state.update(cx, |s, _| {
                                s.repeat = None;
                                s.grab.take().is_some()
                            });
                            if dragging {
                                up_source.drag_ended();
                            }
                        });
                    },
                )
                .size_full(),
            )
    }
}

/// Paint `profile` across the bar over `along`, one logical row per entry,
/// resampled to the bar's thickness (nearest row) for small and mini bars.
fn paint_rows(
    window: &mut Window,
    span: &dyn Fn(Pixels, Pixels, Pixels, Pixels) -> Bounds<Pixels>,
    along: (Pixels, Pixels),
    thickness: f32,
    profile: &[Rgba8],
) {
    let count = thickness.round().max(1.0) as usize;
    for row in 0..count {
        let index = (row * profile.len() / count).min(profile.len() - 1);
        let across = px(row as f32);
        window.paint_quad(gpui::fill(
            span(along.0, along.1, across, across + px(1.)),
            hsla(profile[index]),
        ));
    }
}

/// Darken the rows before the arrow boxes and the bar's last rows
/// (M: scroller.cap).
fn paint_shading(
    window: &mut Window,
    span: &dyn Fn(Pixels, Pixels, Pixels, Pixels) -> Bounds<Pixels>,
    l: &Layout,
    length: Pixels,
    full: Pixels,
    cap: Rgba8,
) {
    if cap.a == 0 {
        return;
    }
    let shade = |window: &mut Window, t: Pixels, alpha: f32| {
        let color = cap.alpha((alpha * 255.0).round() as u8);
        window.paint_quad(gpui::fill(span(t, t + px(1.), px(0.), full), hsla(color)));
    };
    let cap_alpha = cap.a as f32 / 255.0;
    let rows = CAP_PROFILE.len() as f32;
    for (i, fraction) in CAP_PROFILE.iter().enumerate() {
        let t = l.track_end - px(rows - 2.0) + px(i as f32);
        if t >= l.track_start {
            shade(window, t, fraction * cap_alpha);
        }
    }
    let end_rows = END_PROFILE.len() as f32;
    for (i, alpha) in END_PROFILE.iter().enumerate() {
        shade(window, length - px(end_rows) + px(i as f32), *alpha);
    }
}
