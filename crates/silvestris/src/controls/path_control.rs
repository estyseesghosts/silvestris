//! Path pop-up and breadcrumb control (#41).
//!
//! Both render a location the *provider* supplies as
//! `(LocationId, label, Icon)` tuples. Neither parses a filesystem path nor
//! owns the current location: choosing an entry emits
//! [`PathEvent::Navigate`] and the caller moves `FileBrowserModel`.
//!
//! - [`PathPopUp`] is the Open panel's "Documents" pop-up: the ancestors of
//!   the current location, then "Recent Places". It reuses `PopUpButton`, so
//!   keyboard, Escape and outside-click behaviour are the shared ones.
//! - [`PathControl`] is the Finder breadcrumb: one clickable segment per
//!   ancestor, root first.

use gpui::{div, prelude::*, px, App, ElementId, Entity, SharedString, Window};
use std::rc::Rc;

use super::popup_button::PopUpButton;
use super::ControlSize;
use crate::convert::hsla;
use crate::icons::Icon;
use crate::transient::TransientLayer;
use crate::ActiveTheme;

/// The provider's stable location id.
pub type LocationId = SharedString;

/// One entry in a path control, supplied by the provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathEntry {
    pub id: LocationId,
    pub label: SharedString,
    pub icon: Icon,
}

impl PathEntry {
    pub fn new(id: impl Into<LocationId>, label: impl Into<SharedString>, icon: Icon) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathEvent {
    Navigate(LocationId),
}

type NavigateFn = Rc<dyn Fn(&LocationId, &mut Window, &mut App)>;

/// The toolbar pop-up: current location first, then its ancestors, then
/// Recent Places under a non-selectable heading.
#[derive(IntoElement)]
pub struct PathPopUp {
    id: ElementId,
    current: Option<PathEntry>,
    ancestors: Vec<PathEntry>,
    recent: Vec<PathEntry>,
    width: Option<f32>,
    layer: Option<Entity<TransientLayer>>,
    on_navigate: Option<NavigateFn>,
}

impl PathPopUp {
    pub fn new(id: impl Into<ElementId>, current: PathEntry) -> Self {
        Self {
            id: id.into(),
            current: Some(current),
            ancestors: Vec::new(),
            recent: Vec::new(),
            width: None,
            layer: None,
            on_navigate: None,
        }
    }

    /// Exact closed-button width (M: panel.path_popup: 219 px at 1x).
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Ancestors from the current location's parent to the root.
    pub fn ancestors(mut self, entries: Vec<PathEntry>) -> Self {
        self.ancestors = entries;
        self
    }

    pub fn recent(mut self, entries: Vec<PathEntry>) -> Self {
        self.recent = entries;
        self
    }

    /// Use the window's shared transient layer for the menu.
    pub fn layer(mut self, layer: Entity<TransientLayer>) -> Self {
        self.layer = Some(layer);
        self
    }

    /// Choosing any real entry reports its location. The Recent Places heading
    /// is disabled and reports nothing.
    pub fn on_navigate(mut self, f: impl Fn(&LocationId, &mut Window, &mut App) + 'static) -> Self {
        self.on_navigate = Some(Rc::new(f));
        self
    }

    fn entries(&self) -> (Vec<PathEntry>, Vec<usize>) {
        let mut entries = Vec::new();
        let mut headings = Vec::new();
        if let Some(current) = &self.current {
            entries.push(current.clone());
        }
        entries.extend(self.ancestors.iter().cloned());
        if !self.recent.is_empty() {
            headings.push(entries.len());
            entries.push(PathEntry::new("", "Recent Places", Icon::Folder));
            entries.extend(self.recent.iter().cloned());
        }
        (entries, headings)
    }
}

impl RenderOnce for PathPopUp {
    fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
        let (entries, headings) = self.entries();
        let labels: Vec<SharedString> = entries.iter().map(|entry| entry.label.clone()).collect();
        let icon = self.current.as_ref().map(|entry| {
            entry
                .icon
                .file_element(16.0, crate::icons::FileIconState::normal())
        });
        let current = self.current.clone();
        let mut popup = PopUpButton::new(self.id, "Location", labels)
            .selected(Some(0))
            .size(ControlSize::Regular)
            .unselectable(headings.clone())
            .min_width(140.0);
        if let Some(width) = self.width {
            popup = popup.width(width);
        }
        if let Some(layer) = self.layer {
            popup = popup.layer(layer);
        }
        if let Some(icon) = icon {
            popup = popup.leading_icon(icon);
        }
        let navigate = self.on_navigate;
        popup.on_choose(move |index, window, cx| {
            if headings.contains(index) {
                return;
            }
            let Some(entry) = entries.get(*index) else {
                return;
            };
            if Some(&entry.id) == current.as_ref().map(|c| &c.id) {
                return;
            }
            if let Some(f) = &navigate {
                f(&entry.id, window, cx);
            }
        })
    }
}

/// The Finder breadcrumb: root first, separated by chevrons.
#[derive(IntoElement)]
pub struct PathControl {
    id: ElementId,
    entries: Vec<PathEntry>,
    on_navigate: Option<NavigateFn>,
}

impl PathControl {
    pub fn new(id: impl Into<ElementId>, entries: Vec<PathEntry>) -> Self {
        Self {
            id: id.into(),
            entries,
            on_navigate: None,
        }
    }

    pub fn on_navigate(mut self, f: impl Fn(&LocationId, &mut Window, &mut App) + 'static) -> Self {
        self.on_navigate = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for PathControl {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let navigate = self.on_navigate;
        let last = self.entries.len().saturating_sub(1);
        let mut row = div()
            .id(self.id)
            .debug_selector(|| "path-control".into())
            .flex()
            .items_center()
            .gap(px(3.));
        for (index, entry) in self.entries.iter().enumerate() {
            if index > 0 {
                row = row.child(
                    div()
                        .flex_none()
                        .text_size(px(theme.type_.size_small))
                        .text_color(hsla(theme.colors.text_secondary))
                        .child("\u{203A}"),
                );
            }
            let entry = entry.clone();
            let handler = navigate.clone();
            let is_last = index == last;
            row = row.child(
                div()
                    .id(SharedString::from(format!("path-segment-{}", entry.id)))
                    .debug_selector({
                        let id = entry.id.clone();
                        move || format!("path-segment-{id}")
                    })
                    .role(gpui::Role::Button)
                    .aria_label(entry.label.clone())
                    .flex()
                    .items_center()
                    .gap(px(3.))
                    .px(px(2.))
                    .cursor_pointer()
                    .rounded(px(3.))
                    .text_size(px(theme.type_.size_small))
                    .text_color(hsla(theme.colors.text))
                    .when(is_last, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                    .hover(|s| s.text_color(hsla(theme.colors.accent)))
                    .child(
                        entry
                            .icon
                            .file_element(12.0, crate::icons::FileIconState::normal()),
                    )
                    .child(entry.label.clone())
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &handler {
                            f(&entry.id, window, cx);
                        }
                    }),
            );
        }
        row
    }
}
