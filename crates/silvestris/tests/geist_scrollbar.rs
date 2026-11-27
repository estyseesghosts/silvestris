//! Geist's flat scrollbar keeps the shared scrolling model.
//!
//! The bars use the same `ScrollSource` and pointer code as Aqua. Only the
//! track layout and painting differ: no arrow boxes, a 6 px thumb centred in
//! the 10 px layout allocation, and flat rounded ends.

use gpui::{
    div, point, prelude::*, px, Context, Modifiers, MouseButton, Pixels, Point, Render,
    ScrollHandle, TestAppContext, Window,
};
use silvestris::controls::scroll_source::ScrollSource;
use silvestris::controls::{ScrollAxes, ScrollView};
use silvestris::{ThemeId, ThemePreference};

struct Harness {
    scroll: ScrollHandle,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows = (0..40).map(|i| {
            div()
                .flex_none()
                .h(px(20.))
                .w(px(600.))
                .child(format!("Row {i}"))
        });
        div().size(px(200.)).child(
            ScrollView::new("geist-view", &self.scroll)
                .axes(ScrollAxes::Both)
                .line(20.)
                .child(
                    div()
                        .id("geist-content")
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

#[test]
fn geist_allocation_is_ten_pixels_and_the_flat_bar_has_no_arrow_boxes() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::GEIST_DARK), cx));
    let (_, cx) = app.add_window_view(|_, _| Harness {
        scroll: ScrollHandle::new(),
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let vertical = cx.debug_bounds("scrollbar").expect("vertical bar");
    let horizontal = cx
        .debug_bounds("scrollbar-horizontal")
        .expect("horizontal bar");
    assert_eq!(vertical.size.width, px(10.));
    assert_eq!(horizontal.size.height, px(10.));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let quads = cx.update(|window, _| {
        let scale = window.scale_factor();
        let vertical = vertical.scale(scale);
        let horizontal = horizontal.scale(scale);
        window
            .painted_quads()
            .into_iter()
            .filter(|q| {
                (vertical.contains(&q.bounds.center())
                    && q.bounds.size.width <= vertical.size.width)
                    || (horizontal.contains(&q.bounds.center())
                        && q.bounds.size.height <= horizontal.size.height)
            })
            .count()
    });
    // A rounded fill paints five quads; each thumb draws a border and a
    // fill. Aqua adds 30 arrow-row quads and two glyph paths on top.
    assert!(quads <= 21, "flat bar painted {quads} quads");
}

#[test]
fn dragging_the_geist_thumb_preserves_live_scroll_and_pointer_capture() {
    let mut app = TestAppContext::single();
    app.update(|cx| {
        silvestris::theme::install(ThemePreference::explicit(ThemeId::GEIST_LIGHT), cx)
    });
    let (view, cx) = app.add_window_view(|_, _| Harness {
        scroll: ScrollHandle::new(),
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bar = cx.debug_bounds("scrollbar").expect("vertical bar");
    let start: Point<Pixels> = point(bar.center().x, bar.top() + px(12.));
    let end: Point<Pixels> = point(bar.center().x, bar.bottom() - px(8.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    let moved = cx.read(|cx| view.read(cx).scroll.position().y);
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    cx.run_until_parked();
    assert!(moved > px(100.), "drag did not scroll live: {moved:?}");
}
