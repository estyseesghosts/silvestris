//! Compact labelled values for status bars and tile edges (handbook 13).
use gpui::{App, Div, ElementId, Rgba, SharedString, Stateful, div, prelude::*, px};

use crate::{ActiveTheme, convert::hsla};

pub fn value(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    ink: Rgba,
    cx: &App,
) -> Stateful<Div> {
    let theme = cx.theme();
    let label = label.into();
    let value = value.into();
    let detail = detail.into();
    super::tooltip::attach(
        div()
            .id(id.into())
            .aria_label(format!("{label}: {value}"))
            .flex()
            .items_center()
            .gap(px(4.0))
            .px(px(4.0))
            .min_w_0()
            .h(px(22.0))
            .overflow_hidden()
            .text_size(px(theme.type_.size_small))
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(hsla(theme.colors.text_secondary))
                    .child(label.clone()),
            )
            .child(
                div()
                    .min_w_0()
                    .truncate()
                    .font_family(theme.type_.mono)
                    .text_color(ink)
                    .child(value.clone()),
            ),
        format!("{label}: {value}\n{detail}"),
    )
}
