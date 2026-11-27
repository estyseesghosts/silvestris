//! Geist control-state rendering in the shared button implementation.

use gpui::{div, prelude::*, px, Context, Modifiers, Render, TestAppContext, Window};
use silvestris::controls::Button;
use silvestris::{ActiveTheme, ThemeId, ThemePreference};

struct Harness;

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(200.))
            .child(Button::new("hover-button", "Save").on_click(|_, _, _| {}))
    }
}

#[test]
fn geist_button_hover_moves_from_normal_to_the_hover_material() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::GEIST_DARK), cx));
    let (_, cx) = app.add_window_view(|_, _| Harness);
    cx.run_until_parked();
    let bounds = cx.debug_bounds("hover-button").expect("button");
    let fill = |cx: &mut gpui::VisualTestContext| {
        cx.update(|window, _| {
            let inner = silvestris::paint::inset(bounds, silvestris::convert::hairline(window))
                .scale(window.scale_factor());
            window
                .painted_quads()
                .iter()
                .find(|quad| quad.bounds == inner)
                .expect("bezel fill")
                .background
        })
    };
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let resting = fill(cx);
    let outside = gpui::point(bounds.center().x, bounds.bottom() + px(10.));
    cx.simulate_mouse_move(outside, None, Modifiers::default());
    cx.run_until_parked();
    cx.simulate_mouse_move(bounds.center(), None, Modifiers::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let hovered = fill(cx);
    assert_ne!(resting, hovered, "hover must use a distinct Geist material");
    let expected = cx.update(|_, cx| cx.theme().controls.hover.upper.from);
    assert_eq!(
        hovered,
        gpui::solid_background(silvestris::convert::hsla(expected)),
        "hover fill must come from the resolved palette"
    );
}
