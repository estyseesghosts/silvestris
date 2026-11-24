//! Sheets (#22): attachment inside the owner window, stacking with alerts and
//! owner-close cancellation.

use gpui::{
    div, prelude::*, px, Context, TestAppContext, VisualContext, VisualTestContext, Window,
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
}

impl gpui::Render for Owner {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new("Document").child(self.content.clone())
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

#[test]
fn a_sheet_is_drawn_inside_the_owner_window_under_its_title_bar() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    let (_owner, cx) = app.add_window_view(|_, cx| {
        let content = cx.new(|_| Body);
        Owner { content }
    });
    draw(cx);
    let titlebar = cx.debug_bounds("silvestris-titlebar").expect("titlebar");
    let session = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let sheet = cx.update(|_, cx| {
        sheet::Sheet::new(
            "Save",
            420.0,
            session.clone(),
            |_, _| {
                div()
                    .debug_selector(|| "sheet-panel-body".into())
                    .h(px(160.))
                    .w_full()
                    .into_any_element()
            },
            cx,
        )
    });
    let shown = {
        let owner_handle = cx.window_handle();
        cx.update(|_, cx| sheet::present(owner_handle, sheet, cx))
    };

    assert!(shown);
    draw(cx);
    let overlay = cx.debug_bounds("silvestris-sheet").expect("sheet overlay");
    let body = cx.debug_bounds("sheet-panel-body").expect("sheet body");
    assert!(
        overlay.top() >= titlebar.bottom(),
        "the sheet starts below the title bar: {} >= {}",
        overlay.top(),
        titlebar.bottom()
    );
    assert!(
        body.top() <= titlebar.bottom() + px(1.),
        "the panel is attached directly under the chrome"
    );
    cx.update(|window, cx| {
        let active = sheet::active_sessions(window, cx);
        assert_eq!(active, vec![session.clone()]);
        assert_eq!(session.read(cx).phase(), ModalPhase::Active);
    });
}

#[test]
fn closing_the_owner_cancels_the_sheet_and_removes_it() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        content: cx.new(|_| Body),
    });
    draw(cx);
    let session = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let sheet = cx.update(|_, cx| {
        sheet::Sheet::new(
            "Save",
            420.0,
            session.clone(),
            |_, _| div().h(px(80.)).into_any_element(),
            cx,
        )
    });
    let owner_handle = cx.window_handle();
    cx.update(|_, cx| sheet::present(owner_handle, sheet, cx));
    draw(cx);
    assert!(cx.debug_bounds("silvestris-sheet").is_some());
    cx.update(|_, cx| {
        assert_eq!(modal::cancel_owned(owner_handle, cx), 1);
    });
    draw(cx);
    assert!(cx.debug_bounds("silvestris-sheet").is_none());
    cx.update(|_, cx| {
        assert!(session.read(cx).is_ended());
        assert!(matches!(
            session.read(cx).phase(),
            ModalPhase::Ended(ModalResult::Cancelled)
        ));
    });
}

#[test]
fn an_alert_stacks_over_a_sheet_and_escape_returns_to_the_sheet() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        content: cx.new(|_| Body),
    });
    draw(cx);
    let panel_session = cx.update(|_, cx| cx.new(|_| ModalSession::new(ModalScope::Document)));
    let sheet = cx.update(|_, cx| {
        sheet::Sheet::new(
            "Save",
            420.0,
            panel_session.clone(),
            |_, _| {
                div()
                    .debug_selector(|| "panel-body".into())
                    .h(px(120.))
                    .into_any_element()
            },
            cx,
        )
    });
    let owner_handle = cx.window_handle();
    cx.update(|_, cx| sheet::present(owner_handle, sheet, cx));
    draw(cx);
    let alert = cx.update(|_, cx| {
        silvestris::controls::Alert::replace_existing("report.txt")
            .icon(silvestris::icons::Icon::Document)
            .present(owner_handle, cx)
    });
    draw(cx);
    assert!(cx.debug_bounds("silvestris-alert").is_some());
    cx.update(|window, cx| {
        let sessions = sheet::active_sessions(window, cx);
        assert_eq!(sessions.len(), 2, "alert over sheet");
        assert_eq!(
            modal::active(window, cx).map(|s| s.read(cx).scope()),
            Some(ModalScope::Application)
        );
    });
    cx.simulate_keystrokes("escape");
    draw(cx);
    assert!(cx.debug_bounds("silvestris-alert").is_none());
    assert!(
        cx.debug_bounds("panel-body").is_some(),
        "cancelling the alert returns to the Save sheet"
    );
    cx.update(|window, cx| {
        let sessions = sheet::active_sessions(window, cx);
        assert_eq!(sessions, vec![panel_session.clone()]);
        assert!(!panel_session.read(cx).is_ended());
        assert!(alert.read(cx).title().contains("report.txt"));
    });
}
