//! Tab view: a group box with a centred tab control straddling its top edge
//! (HIG pp. 338-341; M: prefs.pane.geometry, groupbox.top, groupbox.left, groupbox.bottom).
//!
//! The view's top edge is the top of the tab control. Place it 12 or 14 px
//! below the title bar or toolbar, inset 20 px (16 px acceptable) from the
//! window sides and bottom, or extend it to the window edges (HIG p. 340).
//! [`crate::preferences::pane_tab_view`] applies the System Preferences
//! placement.
//!
//! The tabs are a [`SegmentedControl`], which is what 10.6 draws: the Date &
//! Time capture's tabs are the segmented bezel with the selected-segment blue.
//! Keyboard focus, arrow keys and Home/End come from that control.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, ElementId, IntoElement, ParentElement, Pixels,
    SharedString, Styled, Window,
};

use crate::controls::{ControlSize, Scrollbar, Segment, SegmentedControl};
use crate::{paint, ActiveTheme};

type SelectFn = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type OverflowFn = Rc<dyn Fn(gpui::Size<Pixels>, &mut Window, &mut App)>;

/// Space between pane content and the box's sides.
const CONTENT_SIDE: f32 = 16.0;
/// Space between pane content and the box's bottom, and below the tabs.
const CONTENT_GAP: f32 = 12.0;

#[derive(IntoElement)]
pub struct TabView {
    id: ElementId,
    tabs: Vec<Segment>,
    selected: Option<SharedString>,
    size: ControlSize,
    on_select: Option<SelectFn>,
    on_overflow: Option<OverflowFn>,
    scroll: Option<gpui::ScrollHandle>,
    content: Vec<AnyElement>,
}

impl TabView {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            tabs: Vec::new(),
            selected: None,
            size: ControlSize::Regular,
            on_select: None,
            on_overflow: None,
            scroll: None,
            content: Vec::new(),
        }
    }

    /// The tabs and the selected tab's id. Use 2 to about 6 tabs with
    /// title-style labels; multiple rows are not supported (HIG p. 339).
    /// Without tabs the view is a plain box.
    pub fn tabs(mut self, tabs: Vec<Segment>, selected: impl Into<SharedString>) -> Self {
        self.tabs = tabs;
        self.selected = Some(selected.into());
        self
    }

    /// Tab size. Label margins are 12 / 10 / 8 px (HIG p. 341).
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    /// Called with the chosen tab's id when the user picks a tab. Setting
    /// the selection through [`TabView::tabs`] does not call it.
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }

    /// Use a caller-owned scroll position for the pane content.
    pub fn scroll_handle(mut self, scroll: gpui::ScrollHandle) -> Self {
        self.scroll = Some(scroll);
        self
    }

    /// Reports each frame how much wider and taller (positive) or narrower
    /// and shorter (negative) the view needs to be: height for the content,
    /// width for the tab strip plus a 16 px inset each side. The window can
    /// then resize to the pane as 10.6 System Preferences does. Content that
    /// still does not fit scrolls with a [`Scrollbar`].
    pub fn on_overflow(
        mut self,
        f: impl Fn(gpui::Size<Pixels>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_overflow = Some(Rc::new(f));
        self
    }

    pub(crate) fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }
}

impl ParentElement for TabView {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl RenderOnce for TabView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let colors = theme.colors.clone();
        let group_radius = theme.metrics.group_radius;
        let has_tabs = !self.tabs.is_empty();
        let tab_height = match self.size {
            ControlSize::Regular => theme.metrics.segment_height,
            ControlSize::Small => theme.metrics.push_small_height,
            ControlSize::Mini => theme.metrics.push_mini_height,
        };
        let label_margin = match self.size {
            ControlSize::Regular => theme.metrics.tab_label_margin,
            ControlSize::Small => 10.0,
            ControlSize::Mini => 8.0,
        };
        // The box's top line runs through the tab control: 10 rows below
        // its top for the 21 px regular tabs (M: prefs.pane.geometry).
        let half = if has_tabs {
            (tab_height / 2.0).floor()
        } else {
            0.0
        };
        let pad_top = half + CONTENT_GAP;
        let content_height = Rc::new(Cell::new(px(0.)));
        let tabs_width = Rc::new(Cell::new(px(0.)));
        let viewport = Rc::new(Cell::new(px(0.)));
        let scroll = self.scroll.unwrap_or_else(|| {
            window
                .use_keyed_state(
                    ElementId::Name(format!("{}-scroll", self.id).into()),
                    cx,
                    |_, _| gpui::ScrollHandle::new(),
                )
                .read(cx)
                .clone()
        });

        let measured = content_height.clone();
        let measured_viewport = viewport.clone();
        let body = div()
            .id("tab-view-box")
            .debug_selector(|| "tab-view-box".into())
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .mt(px(half))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint::group_box_with_radius(window, bounds, &colors, group_radius)
                    },
                )
                .absolute()
                .inset_0(),
            )
            .child(
                div()
                    .id("tab-view-content")
                    .flex_1()
                    .min_h_0()
                    // Without this the row sizes to unwrapped text and pushes
                    // the scroller out of the box.
                    .min_w_0()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .pt(px(pad_top))
                    .px(px(CONTENT_SIDE))
                    .pb(px(CONTENT_GAP))
                    .child(
                        div()
                            .flex_none()
                            .w_full()
                            .relative()
                            .children(self.content)
                            .child(
                                canvas(move |b, _, _| measured.set(b.size.height), |_, _, _, _| {})
                                    .absolute()
                                    .inset_0(),
                            ),
                    ),
            )
            .child(Scrollbar::new(
                ElementId::Name(format!("{}-scrollbar", self.id).into()),
                &scroll,
            ))
            .child(
                canvas(
                    move |b, _, _| measured_viewport.set(b.size.height),
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );

        let mut root = div()
            .id(self.id.clone())
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(body);
        if has_tabs {
            let mut tabs = SegmentedControl::new(
                ElementId::Name(format!("{}-tabs", self.id).into()),
                "Tabs",
                self.tabs,
            )
            .size(self.size)
            .fit_labels(label_margin);
            if let Some(selected) = self.selected {
                tabs = tabs.selected(selected);
            }
            if let Some(f) = self.on_select {
                tabs = tabs.on_select(move |id, w, cx| f(id, w, cx));
            }
            let measured = tabs_width.clone();
            // Painted after the box so the tabs sit over its top line.
            root = root.child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .debug_selector(|| "tab-view-tabs".into())
                            .flex_none()
                            .relative()
                            .child(tabs)
                            .child(
                                canvas(move |b, _, _| measured.set(b.size.width), |_, _, _, _| {})
                                    .absolute()
                                    .inset_0(),
                            ),
                    ),
            );
        }
        if let Some(f) = self.on_overflow {
            // Last child: prepaints after the content and tabs are measured.
            root = root.child(
                canvas(
                    move |view, window, cx| {
                        let width = if has_tabs {
                            tabs_width.get() + px(CONTENT_SIDE * 2.0) - view.size.width
                        } else {
                            px(0.)
                        };
                        let height =
                            content_height.get() + px(pad_top + CONTENT_GAP) - viewport.get();
                        f(gpui::size(width, height), window, cx);
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        }
        root
    }
}
