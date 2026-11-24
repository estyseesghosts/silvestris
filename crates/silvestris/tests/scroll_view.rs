//! Scroll sources, scroll bars and the scroll view (#40): adapters, both
//! axes, the corner, arrow and track clicks, both Appearance preferences and
//! the frozen-height drag contract for variable-height lists.

use gpui::{
    div, list, point, prelude::*, px, uniform_list, Axis, Bounds, Context, ListAlignment,
    ListState, Modifiers, MouseButton, Pixels, Point, Render, ScrollHandle, TestAppContext,
    UniformListScrollHandle, VisualTestContext, Window,
};
use silvestris::controls::scroll_source::{self, ScrollSource};
use silvestris::controls::scroll_view::ScrollAxes;
use silvestris::controls::{scrollbar, ScrollView, ScrollerPreferences, TrackClick};
use silvestris::{ThemeId, ThemePreference};

const ROW: f32 = 20.0;

/// A 200 x 150 view of 40 rows that are 600 px wide, or of `rows` rows that
/// fit when `rows` is small.
struct Harness {
    scroll: ScrollHandle,
    rows: usize,
    width: f32,
    axes: ScrollAxes,
    autohide: bool,
    leave_corner: bool,
}

impl Harness {
    fn new(rows: usize, width: f32) -> Self {
        Self {
            scroll: ScrollHandle::new(),
            rows,
            width,
            axes: ScrollAxes::Both,
            autohide: true,
            leave_corner: false,
        }
    }
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows = (0..self.rows).map(|i| {
            div()
                .flex_none()
                .h(px(ROW))
                .w(px(self.width))
                .child(format!("Row {i}"))
        });
        div().size(px(200.)).child(
            ScrollView::new("view", &self.scroll)
                .axes(self.axes)
                .autohide(self.autohide)
                .leave_corner(self.leave_corner)
                .line(ROW)
                .child(
                    div()
                        .id("content")
                        .size_full()
                        .flex()
                        .flex_col()
                        .overflow_scroll()
                        .track_scroll(&self.scroll)
                        .children(rows),
                ),
        )
    }
}

fn install(cx: &mut TestAppContext) {
    cx.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
}

fn open(
    app: &mut TestAppContext,
    harness: Harness,
) -> (gpui::Entity<Harness>, &mut VisualTestContext) {
    install(app);
    let (view, cx) = app.add_window_view(|_, _| harness);
    cx.run_until_parked();
    // Scroll extents are known after the first paint.
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    (view, cx)
}

fn scroll_of(view: &gpui::Entity<Harness>, cx: &mut VisualTestContext) -> ScrollHandle {
    cx.read(|cx| view.read(cx).scroll.clone())
}

fn press(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
}

fn bar(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).expect(selector)
}

#[test]
fn both_bars_and_the_corner_appear_when_content_overflows_both_ways() {
    let mut app = TestAppContext::single();
    let (_, cx) = open(&mut app, Harness::new(40, 600.));
    let vertical = bar(cx, "scrollbar");
    let horizontal = bar(cx, "scrollbar-horizontal");
    let corner = bar(cx, "scroll-corner");
    // M: scroller.track.row, scroller.horizontal: 15 px bars.
    assert_eq!(vertical.size.width, px(15.));
    assert_eq!(horizontal.size.height, px(15.));
    assert_eq!(corner.size.width, px(15.));
    assert_eq!(corner.size.height, px(15.));
    assert_eq!(
        vertical.bottom(),
        corner.top(),
        "vertical bar stops at the corner"
    );
    assert_eq!(
        horizontal.right(),
        corner.left(),
        "horizontal bar stops at the corner"
    );
    assert_eq!(vertical.right(), px(200.));
    assert_eq!(horizontal.bottom(), px(200.));
}

#[test]
fn autohidden_bars_collapse_and_empty_tracks_stay_when_asked() {
    let mut app = TestAppContext::single();
    let (view, cx) = open(&mut app, Harness::new(3, 100.));
    assert!(cx.debug_bounds("scroll-corner").is_none());
    let vertical = cx.debug_bounds("scrollbar");
    assert!(
        vertical.is_none_or(|b| b.size.width == px(0.)),
        "nothing to scroll, so the vertical bar takes no room: {vertical:?}"
    );

    view.update(cx, |h, cx| {
        h.autohide = false;
        cx.notify();
    });
    cx.run_until_parked();
    // HIG p. 226: a bar without a scroller when everything is visible.
    assert_eq!(bar(cx, "scrollbar").size.width, px(15.));
    assert_eq!(bar(cx, "scrollbar-horizontal").size.height, px(15.));
    assert!(cx.debug_bounds("scroll-corner").is_some());
}

#[test]
fn leave_corner_stops_a_lone_vertical_bar_above_the_size_control() {
    let mut app = TestAppContext::single();
    let mut harness = Harness::new(40, 100.);
    harness.axes = ScrollAxes::Vertical;
    harness.leave_corner = true;
    let (_, cx) = open(&mut app, harness);
    let vertical = bar(cx, "scrollbar");
    let corner = bar(cx, "scroll-corner");
    assert_eq!(vertical.bottom(), corner.top());
    assert_eq!(corner.bottom(), px(200.));
}

#[test]
fn arrows_scroll_one_line_and_the_track_pages_by_a_windowful() {
    let mut app = TestAppContext::single();
    let (view, cx) = open(&mut app, Harness::new(40, 100.));
    let scroll = scroll_of(&view, cx);
    let vertical = bar(cx, "scrollbar");
    let viewport = scroll.viewport().height;
    assert_eq!(scroll.position().y, px(0.));

    // Down arrow: the last 16 px box (M: scroller.arrows).
    press(cx, point(vertical.center().x, vertical.bottom() - px(8.)));
    assert_eq!(scroll.position().y, px(ROW));
    // Up arrow: the box above the divider.
    press(cx, point(vertical.center().x, vertical.bottom() - px(25.)));
    assert_eq!(scroll.position().y, px(0.));

    // Track below the thumb: one windowful less one line (HIG p. 226).
    press(cx, point(vertical.center().x, vertical.top() + px(100.)));
    assert_eq!(scroll.position().y, viewport - px(ROW));
}

#[test]
fn jump_to_spot_centres_the_thumb_on_the_pointer() {
    let mut app = TestAppContext::single();
    let (view, cx) = open(&mut app, Harness::new(40, 100.));
    cx.update(|_, cx| {
        ScrollerPreferences {
            track_click: TrackClick::JumpToSpot,
            ..ScrollerPreferences::default()
        }
        .set(cx)
    });
    let scroll = scroll_of(&view, cx);
    let vertical = bar(cx, "scrollbar");
    // Bottom of the track (it ends 34 px above the bar's end): the furthest spot.
    press(cx, point(vertical.center().x, vertical.bottom() - px(36.)));
    assert_eq!(scroll.position().y, scroll.max_position().y);
}

#[test]
fn horizontal_arrows_move_the_content_sideways() {
    let mut app = TestAppContext::single();
    let (view, cx) = open(&mut app, Harness::new(40, 600.));
    let scroll = scroll_of(&view, cx);
    let horizontal = bar(cx, "scrollbar-horizontal");
    press(
        cx,
        point(horizontal.right() - px(8.), horizontal.center().y),
    );
    assert_eq!(scroll.position().x, px(ROW));
    assert_eq!(
        scroll.position().y,
        px(0.),
        "a horizontal click leaves y alone"
    );
}

#[test]
fn reveal_moves_only_as_far_as_needed() {
    let mut app = TestAppContext::single();
    let (view, cx) = open(&mut app, Harness::new(40, 100.));
    let scroll = scroll_of(&view, cx);
    let viewport = scroll.viewport().height;
    // Already visible: no movement (HIG p. 227).
    assert!(!scroll_source::reveal(
        &scroll,
        Axis::Vertical,
        px(20.),
        px(40.)
    ));
    // Below: the row's bottom lands on the viewport's bottom.
    assert!(scroll_source::reveal(
        &scroll,
        Axis::Vertical,
        px(400.),
        px(420.)
    ));
    assert_eq!(scroll.position().y, px(420.) - viewport);
    // Above: the row's top lands on the viewport's top.
    assert!(scroll_source::reveal(
        &scroll,
        Axis::Vertical,
        px(100.),
        px(120.)
    ));
    assert_eq!(scroll.position().y, px(100.));
}

struct UniformHarness {
    handle: UniformListScrollHandle,
}

impl Render for UniformHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(200.)).child(
            ScrollView::new("uniform", &self.handle).child(
                uniform_list("rows", 100, |range, _, _| {
                    range
                        .map(|i| div().h(px(ROW)).child(format!("Row {i}")))
                        .collect()
                })
                .size_full()
                .track_scroll(&self.handle),
            ),
        )
    }
}

#[test]
fn uniform_list_handle_drives_the_same_scroller() {
    let mut app = TestAppContext::single();
    install(&mut app);
    let handle = UniformListScrollHandle::new();
    let (_, cx) = app.add_window_view(|_, _| UniformHarness {
        handle: handle.clone(),
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert_eq!(handle.content_size().height, px(100. * ROW));
    let vertical = bar(cx, "scrollbar");
    press(cx, point(vertical.center().x, vertical.bottom() - px(8.)));
    assert_eq!(
        handle.position().y,
        px(scrollbar::DEFAULT_LINE),
        "no line set: the default unit"
    );
}

struct ListHarness {
    state: ListState,
}

impl Render for ListHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(200.)).child(
            ScrollView::new("variable", &self.state).child(
                list(self.state.clone(), |i, _, _| {
                    // Variable heights: 20 and 40 px rows.
                    let height = if i % 2 == 0 { 20. } else { 40. };
                    div()
                        .h(px(height))
                        .child(format!("Row {i}"))
                        .into_any_element()
                })
                .size_full(),
            ),
        )
    }
}

#[test]
fn list_state_drag_freezes_the_measured_height_until_release() {
    let mut app = TestAppContext::single();
    install(&mut app);
    let state = ListState::new(50, ListAlignment::Top, px(200.));
    let (_, cx) = app.add_window_view(|_, _| ListHarness {
        state: state.clone(),
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(state.max_position().y > px(0.));
    let vertical = bar(cx, "scrollbar");
    // The thumb starts at the top of the track.
    let grab = point(vertical.center().x, vertical.top() + px(5.));
    cx.simulate_mouse_down(grab, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert!(
        state.is_scrollbar_dragging(),
        "grabbing the thumb freezes the height"
    );
    cx.simulate_mouse_move(
        point(grab.x, grab.y + px(40.)),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    cx.run_until_parked();
    assert!(state.position().y > px(0.), "dragging scrolls live");
    cx.simulate_mouse_up(
        point(grab.x, grab.y + px(40.)),
        MouseButton::Left,
        Modifiers::default(),
    );
    cx.run_until_parked();
    assert!(!state.is_scrollbar_dragging(), "release ends the drag");
}
