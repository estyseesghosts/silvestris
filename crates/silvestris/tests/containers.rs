//! Group boxes, wells and separators (#13) render in every appearance, and a
//! group box takes a control as its title (HIG p. 344).

use gpui::{div, prelude::*, px, Context, Render, TestAppContext, Window};
use silvestris::controls::{Checkbox, GroupBox, Separator, Well};
use silvestris::{ThemeId, ThemePreference};

struct Harness;

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                GroupBox::new("group")
                    .title("Group")
                    .child(div().child("Body")),
            )
            .child(
                GroupBox::new("checkbox-group")
                    .title_control(Checkbox::new("group-title", "Enable sharing").checked(true))
                    .child(div().child("Body")),
            )
            .child(Well::new("well").child(div().child("Well")))
            .child(Separator::horizontal())
            .child(div().h(px(40.)).flex().child(Separator::vertical()))
    }
}

#[test]
fn containers_render_in_every_theme() {
    for theme in ThemeId::ALL {
        let mut app = TestAppContext::single();
        app.update(|cx| silvestris::theme::install(ThemePreference::explicit(theme), cx));
        let (_, cx) = app.add_window_view(|_, _| Harness);
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for id in ["group", "checkbox-group", "well", "group-title"] {
            assert!(
                cx.debug_bounds(id).is_some(),
                "{theme:?} did not render {id}"
            );
        }
    }
}

#[test]
fn a_control_title_sits_above_the_box() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let (_, cx) = app.add_window_view(|_, _| Harness);
    cx.run_until_parked();
    let group = cx.debug_bounds("checkbox-group").expect("group");
    let title = cx.debug_bounds("group-title").expect("checkbox title");
    assert_eq!(
        title.origin.y, group.origin.y,
        "the title is the group's first row"
    );
    assert!(title.size.height < group.size.height);
}
