//! Scroll view (#40): content plus its scroll bars and the corner between
//! them, so views never assemble scrollers by hand (HIG pp. 225-227).
//!
//! The content element owns the scrolling; attach the same source to it:
//!
//! ```ignore
//! let scroll = ScrollHandle::new();
//! ScrollView::new("log", &scroll).axes(ScrollAxes::Both).child(
//!     div().id("log-content").size_full().overflow_scroll().track_scroll(&scroll).children(rows),
//! )
//! ```
//!
//! `uniform_list(..).track_scroll(&handle)` with a `UniformListScrollHandle`,
//! or `list(state)` with a `ListState`, work the same way.
//!
//! The corner square is filled with the content background (M:
//! scroller.corner: white in the Finder list view). A window's size control
//! is drawn by `WindowFrame` over the window corner (HIG p. 225); a scroll
//! view in that corner with only one bar should set
//! [`leave_corner`](ScrollView::leave_corner) so the bar stops short of it.
use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, Axis, ElementId, Window};

use super::scroll_source::ScrollSource;
use super::scrollbar::{self, Scrollbar};
use super::ControlSize;
use crate::convert::hsla;
use crate::ActiveTheme;

/// Which scroll bars a [`ScrollView`] has (HIG p. 225: horizontal,
/// vertical, both or neither).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxes {
    #[default]
    Vertical,
    Horizontal,
    Both,
}

impl ScrollAxes {
    fn has(self, axis: Axis) -> bool {
        matches!(
            (self, axis),
            (ScrollAxes::Both, _)
                | (ScrollAxes::Vertical, Axis::Vertical)
                | (ScrollAxes::Horizontal, Axis::Horizontal)
        )
    }
}

#[derive(IntoElement)]
pub struct ScrollView {
    id: ElementId,
    source: Rc<dyn ScrollSource>,
    axes: ScrollAxes,
    size: ControlSize,
    line: f32,
    autohide: bool,
    leave_corner: bool,
    content: Option<AnyElement>,
}

impl ScrollView {
    pub fn new<S: ScrollSource + Clone>(id: impl Into<ElementId>, source: &S) -> Self {
        Self {
            id: id.into(),
            source: Rc::new(source.clone()),
            axes: ScrollAxes::Vertical,
            size: ControlSize::Regular,
            line: scrollbar::DEFAULT_LINE,
            autohide: true,
            leave_corner: false,
            content: None,
        }
    }

    pub fn axes(mut self, axes: ScrollAxes) -> Self {
        self.axes = axes;
        self
    }

    /// Scroll bar size (HIG p. 227: small and mini only beside small or mini
    /// controls).
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// One arrow-click unit, also the overlap kept when paging (HIG p. 226).
    pub fn line(mut self, line: f32) -> Self {
        self.line = line;
        self
    }

    /// `false` keeps empty tracks while the content fits (HIG p. 226).
    pub fn autohide(mut self, autohide: bool) -> Self {
        self.autohide = autohide;
        self
    }

    /// Keep the corner square empty even when only one bar shows, for a view
    /// that sits under the window's size control.
    pub fn leave_corner(mut self, leave_corner: bool) -> Self {
        self.leave_corner = leave_corner;
        self
    }

    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for ScrollView {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let thickness = px(scrollbar::thickness(self.size, cx));
        let corner_fill = hsla(cx.theme().colors.content);
        let source = self.source.as_ref();
        let shows = |axis: Axis| {
            self.axes.has(axis) && (!self.autohide || scrollbar::scrollable(source, axis))
        };
        let vertical = shows(Axis::Vertical);
        let horizontal = shows(Axis::Horizontal);
        let corner = (vertical && horizontal) || (self.leave_corner && (vertical || horizontal));
        let bar = |axis: Axis, suffix: &str| {
            let id = ElementId::Name(format!("{}-{suffix}", element_name(&self.id)).into());
            Scrollbar::from_rc(id, self.source.clone(), axis)
                .size(self.size)
                .line(self.line)
                .autohide(false)
        };
        let corner_square = || {
            div()
                .debug_selector(|| "scroll-corner".into())
                .flex_none()
                .size(thickness)
                .bg(corner_fill)
        };

        let mut main = div().flex().flex_1().min_h_0().w_full().child(
            div()
                .relative()
                .flex_1()
                .min_w_0()
                .h_full()
                .children(self.content),
        );
        if vertical {
            let mut column = div().flex().flex_col().flex_none().h_full().child(
                div()
                    .flex_1()
                    .min_h_0()
                    .child(bar(Axis::Vertical, "vertical")),
            );
            if corner && !horizontal {
                column = column.child(corner_square());
            }
            main = main.child(column);
        }
        let mut root = div()
            .id(self.id.clone())
            .debug_selector(|| "scroll-view".into())
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(main);
        if horizontal {
            let mut bottom = div().flex().flex_none().w_full().h(thickness).child(
                div()
                    .flex_1()
                    .min_w_0()
                    .child(bar(Axis::Horizontal, "horizontal")),
            );
            if corner {
                bottom = bottom.child(corner_square());
            }
            root = root.child(bottom);
        }
        root
    }
}

fn element_name(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => name.to_string(),
        other => format!("{other:?}"),
    }
}
