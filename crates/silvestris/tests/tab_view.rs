//! Tab view (#13): geometry against M: prefs.pane.geometry, selection
//! callbacks, and the overflow/scroll contract from 82ce65a.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Context, Modifiers, Pixels, Render, ScrollHandle, SharedString, Size,
    TestAppContext, Window,
};
use silvestris::controls::{ControlSize, Segment, TabView};
use silvestris::{ThemeId, ThemePreference};

struct Harness {
    selected: SharedString,
    picked: Rc<RefCell<Vec<SharedString>>>,
    overflow: Rc<Cell<Option<Size<Pixels>>>>,
    scroll: ScrollHandle,
    rows: usize,
    size: ControlSize,
}

impl Harness {
    fn new(rows: usize) -> Self {
        Self {
            selected: "date".into(),
            picked: Rc::default(),
            overflow: Rc::default(),
            scroll: ScrollHandle::new(),
            rows,
            size: ControlSize::Regular,
        }
    }
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picked = self.picked.clone();
        let overflow = self.overflow.clone();
        let entity = cx.entity();
        div().size_full().flex().flex_col().p(px(20.)).child(
            TabView::new("tabs")
                .size(self.size)
                .tabs(
                    vec![
                        Segment::new("date", "Date & Time"),
                        Segment::new("zone", "Time Zone"),
                        Segment::new("clock", "Clock"),
                    ],
                    self.selected.clone(),
                )
                .on_select(move |id, _, cx| {
                    picked.borrow_mut().push(id.clone());
                    entity.update(cx, |this, cx| {
                        this.selected = id.clone();
                        cx.notify();
                    });
                })
                .on_overflow(move |size, _, _| overflow.set(Some(size)))
                .scroll_handle(self.scroll.clone())
                .children((0..self.rows).map(|i| div().h(px(20.)).child(format!("Row {i}")))),
        )
    }
}

fn install(cx: &mut TestAppContext, theme: ThemeId) {
    cx.update(|cx| silvestris::theme::install(ThemePreference::explicit(theme), cx));
}

#[test]
fn box_top_line_runs_through_the_middle_of_centred_tabs() {
    let mut app = TestAppContext::single();
    install(&mut app, ThemeId::AQUA_LIGHT);
    let (_, cx) = app.add_window_view(|_, _| Harness::new(2));
    cx.run_until_parked();
    let tabs = cx.debug_bounds("tab-view-tabs").expect("tab strip");
    let body = cx.debug_bounds("tab-view-box").expect("box");
    // 21 px regular tabs; the box line is 10 rows below their top
    // (M: prefs.pane.geometry tabs_y 103-124, box_y 113).
    assert_eq!(tabs.size.height, px(21.));
    assert_eq!(body.origin.y - tabs.origin.y, px(10.));
    let tabs_mid = tabs.origin.x + tabs.size.width / 2.0;
    let body_mid = body.origin.x + body.size.width / 2.0;
    assert!(
        (tabs_mid - body_mid).abs() <= px(0.5),
        "tabs are centred on the box"
    );
}

#[test]
fn small_and_mini_tabs_straddle_the_box_at_half_their_height() {
    for (size, height) in [(ControlSize::Small, 18.), (ControlSize::Mini, 15.)] {
        let mut app = TestAppContext::single();
        install(&mut app, ThemeId::AQUA_LIGHT);
        let (_, cx) = app.add_window_view(move |_, _| Harness {
            size,
            ..Harness::new(2)
        });
        cx.run_until_parked();
        let tabs = cx.debug_bounds("tab-view-tabs").expect("tab strip");
        let body = cx.debug_bounds("tab-view-box").expect("box");
        assert_eq!(tabs.size.height, px(height));
        assert_eq!(
            body.origin.y - tabs.origin.y,
            px((height / 2.0_f32).floor())
        );
    }
}

#[test]
fn clicking_a_tab_reports_it_and_programmatic_selection_does_not() {
    let mut app = TestAppContext::single();
    install(&mut app, ThemeId::AQUA_LIGHT);
    let (view, cx) = app.add_window_view(|_, _| Harness::new(2));
    cx.run_until_parked();
    let picked = view.read_with(cx, |h, _| h.picked.clone());

    view.update(cx, |h, cx| {
        h.selected = "clock".into();
        cx.notify();
    });
    cx.run_until_parked();
    assert!(
        picked.borrow().is_empty(),
        "setting the selection is silent"
    );

    let zone = cx.debug_bounds("segment-zone").expect("Time Zone tab");
    cx.simulate_click(zone.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(picked.borrow().as_slice(), &[SharedString::from("zone")]);
    view.read_with(cx, |h, _| {
        assert_eq!(h.selected, SharedString::from("zone"))
    });
}

#[test]
fn tall_content_reports_overflow_and_scrolls() {
    let mut app = TestAppContext::single();
    install(&mut app, ThemeId::AQUA_LIGHT);
    let (view, cx) = app.add_window_view(|_, _| Harness::new(200));
    cx.run_until_parked();
    let (overflow, scroll) = view.read_with(cx, |h, _| (h.overflow.get(), h.scroll.clone()));
    let overflow = overflow.expect("overflow reported");
    assert!(
        overflow.height > px(0.),
        "200 rows do not fit: {overflow:?}"
    );
    assert!(scroll.max_offset().y > px(0.), "the pane scrolls");
}

#[test]
fn short_content_reports_spare_height() {
    let mut app = TestAppContext::single();
    install(&mut app, ThemeId::AQUA_LIGHT);
    let (view, cx) = app.add_window_view(|_, _| Harness::new(1));
    cx.run_until_parked();
    let overflow = view
        .read_with(cx, |h, _| h.overflow.get())
        .expect("overflow reported");
    assert!(
        overflow.height < px(0.),
        "one row leaves room: {overflow:?}"
    );
    assert!(
        overflow.width < px(0.),
        "three tabs fit the window: {overflow:?}"
    );
}

#[test]
fn renders_in_every_appearance() {
    for theme in ThemeId::ALL {
        let mut app = TestAppContext::single();
        install(&mut app, theme);
        let (_, cx) = app.add_window_view(|_, _| Harness::new(3));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("tab-view-box").is_some(), "{theme:?}");
    }
}
