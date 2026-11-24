//! Sheets (#22): attachment inside the owner window, one-sheet rule, growth,
//! animation end state and owner-close cancellation.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    div, prelude::*, px, Context, Entity, TestAppContext, VisualContext, VisualTestContext, Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::modal::{self, ModalPhase, ModalResult, ModalScope, ModalSession};
use silvestris::sheet;
use silvestris::{ThemeId, ThemePreference};

struct Body;

impl gpui::Render for Body {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "sheet-body".into())
            .size_full()
            .p(px(20.))
            .child("Sheet content")
    }
}

struct Owner {
    content: gpui::Entity<Body>,
    toolbar: bool,
}

impl gpui::Render for Owner {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut frame = WindowFrame::new("Document");
        if self.toolbar {
            frame = frame.toolbar(
                div().debug_selector(|| "owner-toolbar".into()).size_full(),
                32.0,
            );
        }
        frame.child(self.content.clone())
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn setup(toolbar: bool, reduce_motion: bool, size: gpui::Size<gpui::Pixels>) -> VisualTestContext {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(reduce_motion));
    let owner = app.open_window(size, |_, cx| Owner {
        content: cx.new(|_| Body),
        toolbar,
    });
    let mut cx = VisualTestContext::from_window(*owner, &app);
    draw(&mut cx);
    cx
}

fn simple_sheet(
    cx: &mut VisualTestContext,
    session: Entity<ModalSession>,
    height: f32,
) -> Entity<sheet::Sheet> {
    cx.update(|_, cx| {
        sheet::Sheet::new_sized(
            "Save",
            420.0,
            height,
            session,
            |_, _| {
                div()
                    .debug_selector(|| "sheet-panel-body".into())
                    .size_full()
                    .into_any_element()
            },
            cx,
        )
    })
}

#[test]
fn a_sheet_is_flush_with_the_content_top_with_and_without_a_toolbar() {
    for toolbar in [false, true] {
        let mut cx = setup(toolbar, true, gpui::size(px(800.), px(600.)));
        let titlebar = cx.debug_bounds("silvestris-titlebar").expect("titlebar");
        let band_bottom = if toolbar {
            cx.debug_bounds("owner-toolbar").expect("toolbar").bottom()
        } else {
            titlebar.bottom()
        };
        let session = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
        let sheet = simple_sheet(&mut cx, session, 160.0);
        let owner = cx.window_handle();
        assert!(cx.update(|_, cx| sheet::present(owner, sheet, cx)));
        draw(&mut cx);
        let card = cx.debug_bounds("silvestris-sheet").expect("sheet card");
        assert_eq!(
            card.top(),
            band_bottom,
            "the sheet's top edge is the band's bottom edge (toolbar {toolbar})"
        );
        assert!(
            cx.debug_bounds("silvestris-sheet-scrim").is_none(),
            "the parent is not dimmed"
        );
    }
}

#[test]
fn a_large_sheet_grows_the_owner_and_cancel_restores_the_exact_size() {
    let mut cx = setup(false, true, gpui::size(px(400.), px(300.)));
    let small = cx.update(|window, _| window.bounds().size);
    assert_eq!(
        small,
        gpui::size(px(400.), px(300.)),
        "the test window starts at 400 x 300"
    );

    let session = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let sheet = simple_sheet(&mut cx, session.clone(), 398.0);
    let owner = cx.window_handle();
    cx.update(|_, cx| sheet::present(owner, sheet, cx));
    draw(&mut cx);
    let grown = cx.update(|window, _| window.bounds().size);
    assert!(
        grown.width >= px(420.),
        "the owner grew to the sheet width: {grown:?}"
    );
    assert!(
        grown.height >= px(398.),
        "the owner grew to the sheet height: {grown:?}"
    );

    cx.update(|window, cx| {
        sheet::dismiss(session, ModalResult::Rejected, window, cx);
    });
    draw(&mut cx);
    let restored = cx.update(|window, _| window.bounds().size);
    assert_eq!(
        restored, small,
        "Cancel restores the exact previous content size (got {restored:?})"
    );
}

#[test]
fn a_second_sheet_on_the_same_window_is_refused() {
    let mut cx = setup(false, true, gpui::size(px(800.), px(600.)));
    let owner = cx.window_handle();
    let one = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let two = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let first = simple_sheet(&mut cx, one.clone(), 80.0);
    let second = simple_sheet(&mut cx, two.clone(), 80.0);
    assert!(cx.update(|_, cx| sheet::present(owner, first, cx)));
    assert!(
        !cx.update(|_, cx| sheet::present(owner, second, cx)),
        "one sheet per window (HIG p. 236)"
    );
    draw(&mut cx);
    cx.update(|window, cx| {
        assert_eq!(sheet::active_sessions(window, cx), vec![one.clone()]);
    });
}

#[test]
fn the_emerge_and_retract_end_states_are_reachable_without_sleeping() {
    let mut cx = setup(false, false, gpui::size(px(800.), px(600.)));
    let session = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let sheet = simple_sheet(&mut cx, session.clone(), 100.0);
    let owner = cx.window_handle();
    cx.update(|_, cx| sheet::present(owner, sheet, cx));
    draw(&mut cx);
    cx.update(|_, cx| {
        assert_eq!(sheet::phase_for(&session, cx), Some("emerging"));
    });
    cx.executor()
        .advance_clock(sheet::EMERGE + Duration::from_millis(1));
    draw(&mut cx);
    cx.update(|_, cx| {
        assert_eq!(sheet::phase_for(&session, cx), Some("active"));
    });
    cx.update(|window, cx| {
        sheet::dismiss(session.clone(), ModalResult::Accepted, window, cx);
    });
    draw(&mut cx);
    cx.update(|_, cx| {
        assert_eq!(sheet::phase_for(&session, cx), Some("retracting"));
    });
    cx.executor()
        .advance_clock(sheet::RETRACT + Duration::from_millis(1));
    draw(&mut cx);
    cx.update(|window, cx| {
        assert!(session.read(cx).is_ended());
        assert!(sheet::active_sessions(window, cx).is_empty());
    });
    assert!(cx.debug_bounds("silvestris-sheet").is_none());
}

#[test]
fn the_modal_result_is_delivered_after_the_retract() {
    let mut cx = setup(false, false, gpui::size(px(800.), px(600.)));
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let session = cx.update(|_, cx| {
        cx.new(|_| {
            ModalSession::new(ModalScope::Document).on_end(move |result| {
                assert_eq!(result, ModalResult::Rejected);
                count.set(count.get() + 1);
            })
        })
    });
    let sheet = simple_sheet(&mut cx, session.clone(), 100.0);
    let owner = cx.window_handle();
    cx.update(|_, cx| sheet::present(owner, sheet, cx));
    draw(&mut cx);
    cx.update(|window, cx| {
        sheet::dismiss(session.clone(), ModalResult::Rejected, window, cx);
        assert!(!session.read(cx).is_ended(), "still retracting");
        assert_eq!(calls.get(), 0);
    });
    cx.executor()
        .advance_clock(sheet::RETRACT + Duration::from_millis(1));
    draw(&mut cx);
    cx.update(|_, cx| {
        assert!(session.read(cx).is_ended());
        assert_eq!(calls.get(), 1);
    });
}

#[test]
fn owner_close_during_the_emerge_presents_cancelled_once() {
    let mut cx = setup(false, false, gpui::size(px(800.), px(600.)));
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let session = cx.update(|_, cx| {
        cx.new(|_| {
            ModalSession::new(ModalScope::Document).on_end(move |result| {
                assert_eq!(result, ModalResult::Cancelled);
                count.set(count.get() + 1);
            })
        })
    });
    let sheet = simple_sheet(&mut cx, session.clone(), 100.0);
    let owner = cx.window_handle();
    cx.update(|_, cx| sheet::present(owner, sheet, cx));
    draw(&mut cx);
    cx.update(|_, cx| {
        assert_eq!(modal::cancel_owned(owner, cx), 1);
        assert_eq!(calls.get(), 1, "Cancelled is delivered once");
        assert!(matches!(
            session.read(cx).phase(),
            ModalPhase::Ended(ModalResult::Cancelled)
        ));
    });
    draw(&mut cx);
    assert!(cx.debug_bounds("silvestris-sheet").is_none());
}
