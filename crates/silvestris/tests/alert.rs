//! Alerts (#23): the replace-existing preset, Return and Escape mapping, and
//! the suppression checkbox.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, Context, Modifiers, TestAppContext, VisualContext, VisualTestContext, Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::controls::{Alert, AlertResult};
use silvestris::{ThemeId, ThemePreference};

struct Body;

impl gpui::Render for Body {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

struct Owner {
    body: gpui::Entity<Body>,
}

impl gpui::Render for Owner {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new("Document").child(self.body.clone())
    }
}

type ResultSlot = Rc<RefCell<Option<AlertResult>>>;

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn present_alert(app: &mut TestAppContext) -> (ResultSlot, &mut VisualTestContext) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    draw(cx);
    let result: ResultSlot = Rc::default();
    let slot = result.clone();
    let owner = cx.window_handle();
    cx.update(|_, cx| {
        Alert::replace_existing("report.txt")
            .icon(silvestris::icons::Icon::Document)
            .on_result(move |result, _, _| *slot.borrow_mut() = Some(result.clone()))
            .present(owner, cx);
    });
    draw(cx);
    (result, cx)
}

#[test]
fn return_chooses_the_default_replace_button() {
    let mut app = TestAppContext::single();
    let (result, cx) = present_alert(&mut app);
    assert!(cx.debug_bounds("silvestris-alert").is_some());
    assert!(cx.debug_bounds("alert-replace").is_some());
    cx.simulate_keystrokes("enter");
    draw(cx);
    let result = result.borrow().clone().expect("a result");
    assert_eq!(result.choice.as_deref(), Some("alert-replace"));
    assert!(!result.cancelled);
    assert!(cx.debug_bounds("silvestris-alert").is_none());
}

#[test]
fn escape_chooses_cancel() {
    let mut app = TestAppContext::single();
    let (result, cx) = present_alert(&mut app);
    cx.simulate_keystrokes("escape");
    draw(cx);
    let result = result.borrow().clone().expect("a result");
    assert_eq!(result.choice.as_deref(), Some("alert-cancel"));
    assert!(result.cancelled);
}

#[test]
fn clicking_replace_reports_the_choice() {
    let mut app = TestAppContext::single();
    let (result, cx) = present_alert(&mut app);
    let replace = cx.debug_bounds("alert-replace").expect("replace button");
    cx.simulate_click(replace.center(), Modifiers::default());
    draw(cx);
    let result = result.borrow().clone().expect("a result");
    assert_eq!(result.choice.as_deref(), Some("alert-replace"));
    assert!(!result.cancelled);
}

#[test]
fn the_suppression_checkbox_travels_with_the_result() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    let (_owner, cx) = app.add_window_view(|_, cx| {
        let body = cx.new(|_| Body);
        Owner { body }
    });
    draw(cx);
    let result: ResultSlot = Rc::default();
    let slot = result.clone();
    let owner = cx.window_handle();
    cx.update(|_, cx| {
        Alert::new("Delete the backup?")
            .message("This cannot be undone.")
            .suppressible("Do not ask again")
            .button(
                silvestris::controls::AlertButton::new("cancel", "Cancel")
                    .role(silvestris::controls::AlertRole::Cancel),
            )
            .button(
                silvestris::controls::AlertButton::new("delete", "Delete")
                    .role(silvestris::controls::AlertRole::Default),
            )
            .on_result(move |result, _, _| *slot.borrow_mut() = Some(result.clone()))
            .present(owner, cx);
    });
    draw(cx);
    let checkbox = cx.debug_bounds("alert-suppress").expect("checkbox");
    cx.simulate_click(checkbox.center(), Modifiers::default());
    draw(cx);
    cx.simulate_keystrokes("enter");
    draw(cx);
    let result = result.borrow().clone().expect("a result");
    assert!(result.suppressed);
    assert_eq!(result.choice.as_deref(), Some("delete"));
}
