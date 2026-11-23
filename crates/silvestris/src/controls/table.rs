//! Alternating, virtualized data rows (HIG pp. 346–351).
//! Row contents and selection/action policy belong to the caller.

use std::ops::Range;

use gpui::{div, prelude::*, px, uniform_list, App, Div, ElementId, Stateful, UniformList, Window};

use crate::{convert::hsla, ActiveTheme};

pub fn virtual_table(
    id: impl Into<ElementId>,
    row_count: usize,
    render: impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<Stateful<Div>> + 'static,
) -> UniformList {
    let id = id.into();
    uniform_list(id, row_count, move |range, window, cx| {
        let theme = cx.theme();
        let content = hsla(theme.colors.content);
        let alternating = hsla(theme.colors.row_alternate);
        range
            .clone()
            .zip(render(range, window, cx))
            .map(|(index, row)| {
                row.role(gpui::Role::Row)
                    .bg(if index % 2 == 0 { content } else { alternating })
            })
            .collect::<Vec<_>>()
    })
}

pub fn code_row(id: impl Into<ElementId>, height: f32, cx: &App) -> Stateful<Div> {
    let theme = cx.theme();
    div()
        .id(id.into())
        .flex()
        .items_center()
        .overflow_hidden()
        .px_2()
        .gap_2()
        .h(px(height))
        .font_family(theme.type_.mono)
        .text_size(px(theme.type_.size_mono))
}
