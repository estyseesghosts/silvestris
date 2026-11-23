//! Theme-aware help tags (HIG pp. 163–165).

use gpui::{div, prelude::*, px, Context, Div, Entity, Render, SharedString, Stateful, Window};

use crate::transient::TransientLayer;
use crate::{convert::hsla, ActiveTheme};

pub fn attach(element: Stateful<Div>, text: impl Into<SharedString>) -> Stateful<Div> {
    let text = text.into();
    element.tooltip(move |_, cx| cx.new(|_| HelpTag(text.clone())).into())
}

/// Attach a help tag that is suppressed while a menu, popover or completion
/// list is open (#60). GPUI still owns hover timing and the tooltip window;
/// this only makes the layer the authority on whether a tooltip may appear.
pub fn attach_in(
    element: Stateful<Div>,
    text: impl Into<SharedString>,
    layer: Entity<TransientLayer>,
) -> Stateful<Div> {
    let text = text.into();
    element.tooltip(move |_, cx| {
        if layer.read(cx).allows_tooltip() {
            cx.new(|_| HelpTag(text.clone())).into()
        } else {
            cx.new(|_| Suppressed).into()
        }
    })
}

struct Suppressed;

impl Render for Suppressed {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
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
