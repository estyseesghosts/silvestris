//! Vertical scroll bar (HIG pp. 225-226) for a GPUI scroll container.
//!
//! Attach the same `ScrollHandle` to the container (`track_scroll`) and to
//! the bar. The bar is shown only while the content is taller than its
//! container (HIG p. 225).
//!
//! - Thumb length is proportional to the visible fraction of the content.
//! - Drag the thumb to scroll live (HIG p. 226 "live scrolling").
//! - Click the track to page towards the pointer; holding repeats until the
//!   thumb reaches the pointer.
//! - Both arrows sit at the bottom, the 10.6 default; click scrolls a line,
//!   holding repeats.
//! - The thumb is graphite in inactive windows (theme `thumb_inactive`).
//!
//! Geometry (M: scroller.arrows, scroller.thumb.row; Console capture):
//! 15 px bar; the track ends 34 px above the bottom; up box, a short divider
//! 17 px above the bottom, down box; 7 x 6 px arrow glyphs centred 7.5 px
//! across. Line step, minimum thumb length and repeat timing are estimates.
use std::time::Duration;

use gpui::{
    canvas, div, point, prelude::*, px, App, Bounds, DispatchPhase, ElementId, MouseButton,
    Entity, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, ScrollHandle, Task, Window,
};

use crate::convert::{across, hsla};
use crate::{paint, ActiveTheme};

/// Height of the paired arrow boxes below the track (M: scroller.arrows).
const ARROWS: f32 = 34.0;
/// Estimate: one arrow click scrolls one 16 px line.
const LINE: f32 = 16.0;
/// Estimate: shortest thumb that keeps both caps visible.
const MIN_THUMB: f32 = 18.0;
/// Estimates for press-and-hold repeat.
const REPEAT_DELAY: Duration = Duration::from_millis(350);
const REPEAT_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Default)]
struct State {
    /// Pointer offset from the thumb top while dragging.
    grab: Option<Pixels>,
    repeat: Option<Task<()>>,
}

#[derive(Clone, Copy)]
struct Geometry {
    track: Bounds<Pixels>,
    thumb: Bounds<Pixels>,
    max: Pixels,
}

fn geometry(bar: Bounds<Pixels>, handle: &ScrollHandle) -> Option<Geometry> {
    let max = handle.max_offset().y;
    let viewport = handle.bounds().size.height;
    if max <= px(0.) || viewport <= px(0.) {
        return None;
    }
    let track = Bounds::new(bar.origin, gpui::size(bar.size.width, (bar.size.height - px(ARROWS)).max(px(0.))));
    let len = (track.size.height * (viewport / (viewport + max))).max(px(MIN_THUMB)).min(track.size.height);
    let pos = (-handle.offset().y / max).clamp(0.0, 1.0);
    let top = track.origin.y + (track.size.height - len) * pos;
    let thumb = Bounds::new(point(bar.origin.x + px(1.), top), gpui::size(bar.size.width - px(2.), len));
    Some(Geometry { track, thumb, max })
}

/// Scroll by `delta` (positive moves the content up, revealing what is below).
fn scroll_by(handle: &ScrollHandle, delta: Pixels) {
    let max = handle.max_offset().y;
    let current = -handle.offset().y;
    let y = (current + delta).clamp(px(0.), max.max(px(0.)));
    handle.set_offset(point(handle.offset().x, -y));
}

/// Repeat `step` after the hold delay until it returns false or the mouse
/// is released (dropping the task cancels it).
fn hold(state: &Entity<State>, window: &mut Window, cx: &mut App, step: impl Fn(&mut Window) -> bool + 'static) {
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

#[derive(IntoElement)]
pub struct Scrollbar {
    id: ElementId,
    handle: ScrollHandle,
}

impl Scrollbar {
    pub fn new(id: impl Into<ElementId>, handle: &ScrollHandle) -> Self {
        Self { id: id.into(), handle: handle.clone() }
    }
}

impl RenderOnce for Scrollbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let width = theme.metrics.scroller_width;
        let style = theme.controls.scroller;
        let handle = self.handle;
        let shown = handle.max_offset().y > px(0.);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        div()
            .id(self.id)
            .debug_selector(|| "scrollbar".into())
            .flex_none()
            .h_full()
            .when(shown, |el| el.w(px(width)))
            .when(!shown, |el| el.w(px(0.)))
            .child(
                canvas(|_, _, _| (), move |bar, _, window, _| {
                    let Some(g) = geometry(bar, &handle) else { return };
                    // Track and arrow boxes share the track gradient (M: scroller.track.row).
                    window.paint_quad(gpui::fill(bar, across(style.track, width)));
                    let end = bar.bottom() - px(ARROWS);
                    paint::hline(window, bar.origin.x, end, bar.size.width, style.track_border);
                    paint::hline(window, bar.origin.x + px(3.), bar.bottom() - px(17.), px(9.), style.track_border);
                    let cx_ = bar.origin.x + px(7.5);
                    let tri = |window: &mut Window, apex: Pixels, base: Pixels| {
                        let mut path = PathBuilder::fill();
                        path.move_to(point(cx_, apex));
                        path.line_to(point(cx_ + px(3.5), base));
                        path.line_to(point(cx_ - px(3.5), base));
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, hsla(style.arrow));
                        }
                    };
                    tri(window, bar.bottom() - px(28.), bar.bottom() - px(22.));
                    tri(window, bar.bottom() - px(5.), bar.bottom() - px(11.));
                    let bevel = if window.is_window_active() { style.thumb } else { style.thumb_inactive };
                    paint::thumb(window, g.thumb, &bevel);

                    let down_state = state.clone();
                    let down_handle = handle.clone();
                    window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
                        if phase != DispatchPhase::Bubble || e.button != MouseButton::Left || !bar.contains(&e.position) {
                            return;
                        }
                        cx.stop_propagation();
                        let y = e.position.y;
                        let h = down_handle.clone();
                        if y >= end {
                            let dir = if y < bar.bottom() - px(17.) { -1.0 } else { 1.0 };
                            scroll_by(&h, px(LINE * dir));
                            hold(&down_state, window, cx, move |_| {
                                scroll_by(&h, px(LINE * dir));
                                true
                            });
                        } else if g.thumb.contains(&e.position) {
                            down_state.update(cx, |s, _| s.grab = Some(y - g.thumb.origin.y));
                        } else {
                            let dir = if y < g.thumb.origin.y { -1.0 } else { 1.0 };
                            let page = |h: &ScrollHandle| (h.bounds().size.height - px(LINE)).max(px(LINE));
                            scroll_by(&h, page(&h) * dir);
                            hold(&down_state, window, cx, move |_| {
                                // Stop once the thumb reaches the pointer.
                                let Some(now) = geometry(bar, &h) else { return false };
                                let reached = if dir < 0.0 { now.thumb.origin.y <= y } else { now.thumb.bottom() >= y };
                                if !reached {
                                    scroll_by(&h, page(&h) * dir);
                                }
                                !reached
                            });
                        }
                        window.refresh();
                    });
                    let move_state = state.clone();
                    let move_handle = handle.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                        if phase != DispatchPhase::Capture {
                            return;
                        }
                        let Some(grab) = move_state.read(cx).grab else { return };
                        let room = g.track.size.height - g.thumb.size.height;
                        if room <= px(0.) {
                            return;
                        }
                        let top = e.position.y - grab - g.track.origin.y;
                        let y = g.max * (top / room).clamp(0.0, 1.0);
                        move_handle.set_offset(point(move_handle.offset().x, -y));
                        window.refresh();
                    });
                    let up_state = state.clone();
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                        if phase == DispatchPhase::Capture {
                            up_state.update(cx, |s, _| {
                                s.grab = None;
                                s.repeat = None;
                            });
                        }
                    });
                })
                .size_full(),
            )
    }
}
