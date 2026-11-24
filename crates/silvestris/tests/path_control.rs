//! Path pop-up and breadcrumb (#41): the provider's entries, the Recent
//! Places heading and navigation events.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window,
};
use silvestris::controls::{PathControl, PathEntry, PathPopUp};
use silvestris::icons::Icon;
use silvestris::transient::TransientLayer;
use silvestris::{ThemeId, ThemePreference};

type Navigated = Rc<RefCell<Vec<String>>>;

struct PopUpView {
    layer: Entity<TransientLayer>,
    navigated: Navigated,
}

impl gpui::Render for PopUpView {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let navigated = self.navigated.clone();
        div().size_full().p(gpui::px(40.)).child(
            PathPopUp::new(
                "path-popup",
                PathEntry::new("documents", "Documents", Icon::Documents),
            )
            .ancestors(vec![
                PathEntry::new("home", "terezi", Icon::Home),
                PathEntry::new("volume", "Macintosh HD", Icon::Volume),
            ])
            .recent(vec![PathEntry::new("desktop", "Desktop", Icon::Desktop)])
            .layer(self.layer.clone())
            .on_navigate(move |id, _, _| navigated.borrow_mut().push(id.to_string())),
        )
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn popup_fixture(
    app: &mut TestAppContext,
) -> (Entity<PopUpView>, Navigated, &mut VisualTestContext) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let navigated: Navigated = Rc::default();
    let result = navigated.clone();
    let (view, cx) = app.add_window_view(|window, cx| {
        let layer = cx.new(|cx| TransientLayer::new(window, cx));
        PopUpView { layer, navigated }
    });
    draw(cx);
    (view, result, cx)
}

#[test]
fn the_path_popup_lists_ancestors_then_recent_places_and_navigates() {
    let mut app = TestAppContext::single();
    let (_view, navigated, cx) = popup_fixture(&mut app);

    let button = cx.debug_bounds("path-popup").expect("path pop-up");
    cx.simulate_click(button.center(), Modifiers::default());
    draw(cx);
    assert!(cx.debug_bounds("popup-panel").is_some());
    assert!(cx.debug_bounds("popup-item-0").is_some());
    assert_eq!(
        cx.debug_bounds("popup-item-3")
            .expect("heading")
            .size
            .height,
        cx.debug_bounds("popup-item-1")
            .expect("ancestor")
            .size
            .height
    );

    let recent = cx.debug_bounds("popup-item-4").expect("Desktop row");
    cx.simulate_click(recent.center(), Modifiers::default());
    draw(cx);
    assert_eq!(*navigated.borrow(), ["desktop"]);
    assert!(cx.debug_bounds("popup-panel").is_none());
}

#[test]
fn the_recent_places_heading_does_not_navigate() {
    let mut app = TestAppContext::single();
    let (_view, navigated, cx) = popup_fixture(&mut app);
    let button = cx.debug_bounds("path-popup").expect("path pop-up");
    cx.simulate_click(button.center(), Modifiers::default());
    draw(cx);
    let heading = cx.debug_bounds("popup-item-3").expect("heading row");
    cx.simulate_click(heading.center(), Modifiers::default());
    draw(cx);
    assert!(navigated.borrow().is_empty());
}

struct BreadcrumbView {
    navigated: Navigated,
}

impl gpui::Render for BreadcrumbView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let navigated = self.navigated.clone();
        div().size_full().p(gpui::px(40.)).child(
            PathControl::new(
                "path-control-test",
                vec![
                    PathEntry::new("volume", "Macintosh HD", Icon::Volume),
                    PathEntry::new("home", "terezi", Icon::Home),
                    PathEntry::new("documents", "Documents", Icon::Documents),
                ],
            )
            .on_navigate(move |id, _, _| navigated.borrow_mut().push(id.to_string())),
        )
    }
}

#[test]
fn clicking_a_breadcrumb_segment_navigates_to_its_location() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let navigated: Navigated = Rc::default();
    let result = navigated.clone();
    let (_view, cx) = app.add_window_view(move |_, _| BreadcrumbView { navigated });
    draw(cx);
    assert!(cx.debug_bounds("path-segment-volume").is_some());
    assert!(cx.debug_bounds("path-segment-home").is_some());
    let home = cx.debug_bounds("path-segment-home").expect("home segment");
    cx.simulate_click(home.center(), Modifiers::default());
    draw(cx);
    assert_eq!(*result.borrow(), ["home"]);
}
