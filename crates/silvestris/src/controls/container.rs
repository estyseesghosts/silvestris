//! Container vocabulary for HIG grouping: group boxes, separators and
//! recessed content wells (HIG pp. 338-344, 361-363; M: groupbox.fill,
//! groupbox.top/left, prefs.pane.geometry).
//!
//! These are materials, not new interaction models. Consumers compose them
//! instead of drawing ad-hoc rounded rectangles, and the same types work in
//! Aqua Dark and Geist because every value is a theme token.

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, ElementId, IntoElement, ParentElement, Styled,
    Window,
};

use crate::{convert::hsla, paint, ActiveTheme};

/// A group box: optional title above a bordered, tinted container. The top
/// edge is darker than the sides in Aqua (M: groupbox.top vs groupbox.left).
#[derive(IntoElement)]
pub struct GroupBox {
    id: ElementId,
    title: Option<gpui::SharedString>,
    title_control: Option<AnyElement>,
    content: Vec<AnyElement>,
    padding: f32,
}

impl GroupBox {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            title_control: None,
            content: Vec::new(),
            padding: 12.0,
        }
    }

    /// Text title in the small system font, sentence-style (HIG p. 344).
    pub fn title(mut self, title: impl Into<gpui::SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A checkbox or pop-up button as the title (HIG p. 344), replacing any
    /// text title. With a checkbox title the contents are active only while
    /// it is checked; the caller disables them from the same state it binds
    /// to the checkbox.
    pub fn title_control(mut self, control: impl IntoElement) -> Self {
        self.title_control = Some(control.into_any_element());
        self
    }

    /// Inner padding. HIG grouped content uses 12 px; a preferences pane uses
    /// 16-20 px.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }
}

impl ParentElement for GroupBox {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl RenderOnce for GroupBox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = theme.colors.clone();
        let mut root = div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .relative()
            .flex()
            .flex_col();
        let title = match (self.title_control, self.title) {
            (Some(control), _) => Some(control),
            // HIG p. 344: the default title font is the small system font.
            (None, Some(text)) => Some(
                div()
                    .font_family(theme.type_.ui)
                    .text_size(px(theme.type_.size_small))
                    .text_color(hsla(colors.text))
                    .child(text)
                    .into_any_element(),
            ),
            (None, None) => None,
        };
        if let Some(title) = title {
            root = root.child(div().flex().mb(px(4.0)).pl(px(2.0)).child(title));
        }
        let body = div()
            .relative()
            .flex()
            .flex_col()
            // Content clears the painted edge row, as it cleared the 1 px
            // CSS border this replaced.
            .p(px(self.padding + 1.0))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| paint::group_box(window, bounds, &colors),
                )
                .absolute()
                .inset_0(),
            )
            .children(self.content);
        root.child(body)
    }
}

/// Horizontal or vertical one-device-row rule (HIG p. 342).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeparatorAxis {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(IntoElement)]
pub struct Separator {
    axis: SeparatorAxis,
    length: Option<f32>,
    inset: f32,
}

impl Separator {
    /// A rule on the chosen axis.
    pub fn new(axis: SeparatorAxis) -> Self {
        Self {
            axis,
            length: None,
            inset: 0.0,
        }
    }

    pub fn horizontal() -> Self {
        Self {
            axis: SeparatorAxis::Horizontal,
            length: None,
            inset: 0.0,
        }
    }

    pub fn vertical() -> Self {
        Self {
            axis: SeparatorAxis::Vertical,
            length: None,
            inset: 0.0,
        }
    }

    /// Optional fixed length; the default stretches in the parent.
    pub fn length(mut self, length: f32) -> Self {
        self.length = Some(length.max(0.0));
        self
    }

    /// Inset at each end, as grouped-content separators use.
    pub fn inset(mut self, inset: f32) -> Self {
        self.inset = inset.max(0.0);
        self
    }
}

impl RenderOnce for Separator {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colour = theme.colors.separator;
        let thin = crate::convert::hairline(window);
        let line = div().flex_none().child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| match self.axis {
                    SeparatorAxis::Horizontal => {
                        let h = crate::convert::hairline(window);
                        let y = bounds.origin.y + (bounds.size.height - h) / 2.0;
                        paint::hline(
                            window,
                            bounds.origin.x + px(self.inset),
                            y,
                            bounds.size.width - px(self.inset * 2.0),
                            colour,
                        );
                    }
                    SeparatorAxis::Vertical => {
                        let h = crate::convert::hairline(window);
                        let x = bounds.origin.x + (bounds.size.width - h) / 2.0;
                        paint::vline(
                            window,
                            x,
                            bounds.origin.y + px(self.inset),
                            bounds.size.height - px(self.inset * 2.0),
                            colour,
                        );
                    }
                },
            )
            .size_full(),
        );
        match self.axis {
            SeparatorAxis::Horizontal => line
                .w_full()
                .h(self.length.map(px).unwrap_or(thin))
                .into_any_element(),
            SeparatorAxis::Vertical => line
                .h(self.length.map(px).unwrap_or(thin))
                .w(thin)
                .into_any_element(),
        }
    }
}

/// A recessed content well: bordered, darker fill and an inner top shadow
/// (M: groupbox.fill/left/top; HIG p. 363).
#[derive(IntoElement)]
pub struct Well {
    id: ElementId,
    content: Vec<AnyElement>,
    padding: f32,
    radius: f32,
}

impl Well {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            content: Vec::new(),
            padding: 8.0,
            radius: 4.0,
        }
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius.max(0.0);
        self
    }
}

impl ParentElement for Well {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl RenderOnce for Well {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = theme.colors.clone();
        let radius = self.radius;
        let line = crate::convert::hairline(window);
        div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .relative()
            .flex()
            .flex_col()
            .bg(hsla(colors.well_fill))
            .border_1()
            .border_color(hsla(colors.well_border))
            .rounded(px(radius))
            .p(px(self.padding))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let h = crate::convert::hairline(window);
                        paint::hline(
                            window,
                            bounds.origin.x + px(radius),
                            bounds.origin.y + h / 2.0,
                            bounds.size.width - px(radius * 2.0),
                            colors.well_shadow,
                        );
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(line),
            )
            .children(self.content)
    }
}
