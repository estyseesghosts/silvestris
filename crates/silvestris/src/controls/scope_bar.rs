//! Scope bar (HIG Scope Bars; #46).
//!
//! A scope bar narrows a list, search or other result set. It is a row of
//! mutually exclusive scopes, not navigation and not a second toolbar, so the
//! control is deliberately small: an optional leading caption plus the
//! measured segmented control ([`SegmentedControl`]) in its small textured
//! form. Consumers use it above search results; ordinary navigation keeps its
//! own source list or segmented view switch.
//!
//! The bar only reports the chosen scope. Filtering, searching and result
//! recomputation belong to the consumer.

use gpui::{prelude::*, ElementId, IntoElement, RenderOnce, SharedString, Window};
use std::rc::Rc;

use crate::controls::segmented::{Segment, SegmentMode, SegmentStyle, SegmentedControl};
use crate::controls::ControlSize;
use crate::ActiveTheme;

/// One selectable scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    id: SharedString,
    label: SharedString,
    disabled: bool,
}

impl Scope {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

type SelectFn = Rc<dyn Fn(&SharedString, &mut Window, &mut gpui::App)>;

/// A small row of exclusive scopes with an optional leading caption.
#[derive(IntoElement)]
pub struct ScopeBar {
    id: ElementId,
    label: SharedString,
    caption: Option<SharedString>,
    scopes: Vec<Scope>,
    selected: Option<SharedString>,
    size: ControlSize,
    on_select: Option<SelectFn>,
}

impl ScopeBar {
    /// `label` is the external accessibility name; `caption`, when present, is
    /// the visible leading text ("Search:").
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        scopes: Vec<Scope>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            caption: None,
            scopes,
            selected: None,
            size: ControlSize::Small,
            on_select: None,
        }
    }

    pub fn caption(mut self, caption: impl Into<SharedString>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }

    /// Scope bars use the small control size; larger sizes exist for a
    /// consumer that has a documented reason.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }

    pub fn on_select(
        mut self,
        f: impl Fn(&SharedString, &mut Window, &mut gpui::App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ScopeBar {
    fn render(self, _: &mut Window, cx: &mut gpui::App) -> impl IntoElement {
        let theme = cx.theme();
        let caption = self.caption.clone();
        let mut bar = SegmentedControl::new(
            self.id.to_string(),
            self.label.clone(),
            self.scopes
                .into_iter()
                .map(|scope| Segment::new(scope.id, scope.label).disabled(scope.disabled))
                .collect(),
        )
        .mode(SegmentMode::SelectOne)
        .style(SegmentStyle::Textured)
        .size(self.size);
        if let Some(selected) = self.selected {
            bar = bar.selected(selected);
        }
        if let Some(on_select) = self.on_select {
            bar = bar.on_select(move |id, window, cx| on_select(id, window, cx));
        }
        let text = theme.colors.text_secondary;
        let size = theme.type_.size_small;
        gpui::div()
            .id(self.id)
            .debug_selector(|| "scope-bar".into())
            .role(gpui::Role::Group)
            .aria_label(self.label.clone())
            .flex()
            .flex_row()
            .items_center()
            .gap(gpui::px(6.))
            .text_size(gpui::px(size))
            .text_color(crate::convert::hsla(text))
            .children(caption.map(|caption| gpui::div().flex_none().child(caption)))
            .child(bar)
    }
}
