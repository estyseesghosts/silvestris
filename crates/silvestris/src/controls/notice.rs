//! A workspace placeholder for blank, unavailable, and failed content (HIG pp. 117–121).
use gpui::{div, prelude::*, px, AnyElement, App, FontWeight, SharedString, Window};

use crate::{convert::hsla, ActiveTheme};

#[derive(IntoElement)]
pub struct Notice {
    title: SharedString,
    description: SharedString,
    failure: bool,
    children: Vec<AnyElement>,
}

impl Notice {
    pub fn new(title: impl Into<SharedString>, description: impl Into<SharedString>) -> Self {
        Self {
            title: title.into(),
            description: description.into(),
            failure: false,
            children: Vec::new(),
        }
    }

    pub fn failure(mut self, failure: bool) -> Self {
        self.failure = failure;
        self
    }
}

impl ParentElement for Notice {
    fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(children);
    }
}

impl RenderOnce for Notice {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.0))
            .p(px(16.0))
            .child(
                div()
                    .text_center()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hsla(if self.failure {
                        theme.colors.error
                    } else {
                        theme.colors.text
                    }))
                    .child(self.title),
            )
            .when(!self.description.is_empty(), |el| {
                el.child(
                    div()
                        .max_w(px(420.0))
                        .text_center()
                        .text_size(px(theme.type_.size_small))
                        .text_color(hsla(theme.colors.text_secondary))
                        .child(self.description),
                )
            })
            .children(self.children)
    }
}

/// A themed error banner with an optional action such as Dismiss.
pub fn error_banner(
    message: impl Into<SharedString>,
    action: impl IntoElement,
    cx: &App,
) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .id("silvestris-error-banner")
        .role(gpui::Role::Alert)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.0))
        .mx(px(12.0))
        .mt(px(8.0))
        .px(px(8.0))
        .py(px(5.0))
        .rounded(px(4.0))
        .border_1()
        .border_color(hsla(theme.colors.error))
        .bg(hsla(theme.colors.window))
        .text_size(px(theme.type_.size_small))
        .child(div().flex_1().min_w_0().child(message.into()))
        .child(action)
}
