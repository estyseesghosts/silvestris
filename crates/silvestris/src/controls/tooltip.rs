//! Theme-aware help tags (HIG pp. 163–165).

use gpui::{Context, Div, Render, SharedString, Stateful, Window, div, prelude::*, px};

use crate::{ActiveTheme, convert::hsla};

pub fn attach(element: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text = text.into();
    element.tooltip(move |_, cx| cx.new(|_| HelpTag(text.clone())).into())
}

struct HelpTag(SharedString);

impl Render for HelpTag {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .max_w(px(360.0))
            .px_2()
            .py_1()
            .rounded(px(3.0))
            .border_1()
            .border_color(hsla(theme.colors.tooltip_border))
            .bg(hsla(theme.colors.tooltip))
            .font_family(theme.type_.ui)
            .text_size(px(theme.type_.size_small))
            .text_color(hsla(theme.colors.tooltip_text))
            .child(self.0.clone())
    }
}
