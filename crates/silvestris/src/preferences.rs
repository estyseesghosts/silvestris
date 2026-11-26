//! System Preferences-style preferences windows (HIG pp. 243-244, 347-350).
//!
//! The pieces are independent so a consumer composes only what it needs:
//!
//! - [`PreferencesNavigator`]: pane selection with back/forward history,
//!   where "Show All" (the overview) is a history entry like any pane.
//! - [`PreferencesToolbar`]: back/forward, Show All and an optional search
//!   slot. Preferences toolbars are not customizable and have no show/hide
//!   control (HIG p. 244), so this is a fixed row, not a toolbar model.
//! - [`PreferencesOverview`]: category bands of icon cells.
//! - [`TabView`]: the group box with a centred tab control straddling its top
//!   edge, as in the Desktop & Screen Saver pane.
//! - [`Form`]: right-aligned `Label:` column with controls aligned in a second
//!   column; label-less rows (checkboxes) start at the control column.
//!
//! Window: open with `chrome::fixed_window_options` and render inside
//! `WindowFrame::fixed_size()`. The title should be the selected pane's name
//! (HIG p. 244); [`PreferencesNavigator::title`] provides it.
//!
//! Geometry cites reference/measurements.json: prefs.overview.geometry,
//! prefs.overview.bands, prefs.pane.geometry, groupbox.top, groupbox.left.
//! Values not in a capture are marked "estimate".

use std::rc::Rc;

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, ElementId, FocusHandle, FontWeight, Pixels,
    SharedString, Window,
};

pub use crate::controls::TabView;
use crate::controls::{Button, ButtonStyle, ControlSize, Segment, SegmentedControl};
use crate::convert::hsla;
use crate::icons::ConsumerArtwork;
use crate::{paint, ActiveTheme};

// --- Measured geometry (logical px) --------------------------------------
/// Window width of the 10.6 System Preferences overview (x 40..707 inclusive).
pub const WINDOW_WIDTH: f32 = 668.0;
/// Icon cell pitch (M: prefs.overview.geometry icon_x 70/151/232/312).
pub const CELL_WIDTH: f32 = 81.0;
/// Overview grid inset from the window's left edge: first icon at x+30 with a
/// centred 32 px icon in an 81 px cell puts the cell at x+5 (estimate ±1).
const GRID_INSET: f32 = 5.0;
pub const ICON_SIZE: f32 = 32.0;
/// Band height excluding its 1 px rule. The capture's bands are 103, 99, 101
/// and 99 rows (79-181, 183-281, 283-383, 385-483); 100 is their mean.
pub const BAND_HEIGHT: f32 = 100.0;
const HEADING_TOP: f32 = 7.0; // ink 9 px below band top less ascent space (estimate)
const HEADING_LEFT: f32 = 14.0;
const ICON_TOP: f32 = 32.0;
const LABEL_TOP: f32 = 66.0; // ink at 68 px; line box starts ~2 px higher (estimate)
/// Pane box inset from the window sides and bottom (HIG p. 349: 20 px).
pub const PANE_MARGIN: f32 = 20.0;
/// Box top line below the toolbar separator (113 - 79).
const BOX_TOP: f32 = 34.0;
/// HIG p. 349: 8 px between controls; 12 px around separators.
pub const CONTROL_GAP: f32 = 8.0;

// --- Navigation -----------------------------------------------------------

/// Pane selection and history. `None` is the Show All overview.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PreferencesNavigator {
    current: Option<SharedString>,
    back: Vec<Option<SharedString>>,
    forward: Vec<Option<SharedString>>,
}

impl PreferencesNavigator {
    pub fn new(current: Option<SharedString>) -> Self {
        Self {
            current,
            ..Self::default()
        }
    }
    pub fn current(&self) -> Option<&SharedString> {
        self.current.as_ref()
    }
    /// Go to a pane (`Some`) or the overview (`None`). Returns false when
    /// already there, so the history gains no duplicate entry.
    pub fn go(&mut self, to: Option<SharedString>) -> bool {
        if to == self.current {
            return false;
        }
        self.back.push(std::mem::replace(&mut self.current, to));
        self.forward.clear();
        true
    }
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }
    pub fn go_back(&mut self) -> bool {
        let Some(to) = self.back.pop() else {
            return false;
        };
        self.forward.push(std::mem::replace(&mut self.current, to));
        true
    }
    pub fn go_forward(&mut self) -> bool {
        let Some(to) = self.forward.pop() else {
            return false;
        };
        self.back.push(std::mem::replace(&mut self.current, to));
        true
    }
    /// Drop history entries for panes that no longer exist (for example a
    /// scope change that hides a category); the current pane falls back to
    /// the overview.
    pub fn retain(&mut self, exists: impl Fn(&SharedString) -> bool) {
        let keep = |entry: &Option<SharedString>| entry.as_ref().is_none_or(&exists);
        self.back.retain(keep);
        self.forward.retain(keep);
        self.back.dedup();
        self.forward.dedup();
        if !keep(&self.current) {
            self.current = None;
        }
    }
    /// Window title: the pane's name, or the application's overview title.
    pub fn title(&self, categories: &[PreferenceCategory], overview: &str) -> SharedString {
        self.current
            .as_ref()
            .and_then(|id| find(categories, id))
            .map(|pane| pane.label.clone())
            .unwrap_or_else(|| overview.to_owned().into())
    }
}

/// History or overview request from the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Navigation {
    Back,
    Forward,
    ShowAll,
}

// --- Model ------------------------------------------------------------------

type IconFn = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// One overview cell. The icon is consumer artwork drawn into a 32 px square.
#[derive(Clone)]
pub struct PreferencePane {
    pub id: SharedString,
    pub label: SharedString,
    icon: Option<IconFn>,
}
impl PreferencePane {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
        }
    }
    pub fn icon<E: IntoElement>(
        mut self,
        icon: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.icon = Some(Rc::new(move |w, cx| icon(w, cx).into_any_element()));
        self
    }

    /// Uses consumer-owned pictorial artwork in the standard 32×32 category
    /// icon slot.
    pub fn artwork(mut self, artwork: ConsumerArtwork) -> Self {
        self.icon = Some(Rc::new(move |window, cx| {
            artwork.category_element(window, cx)
        }));
        self
    }
}

/// One overview band ("Personal", "Hardware"…).
#[derive(Clone)]
pub struct PreferenceCategory {
    pub label: SharedString,
    pub panes: Vec<PreferencePane>,
}
impl PreferenceCategory {
    pub fn new(label: impl Into<SharedString>, panes: Vec<PreferencePane>) -> Self {
        Self {
            label: label.into(),
            panes,
        }
    }
}

pub fn find<'a>(categories: &'a [PreferenceCategory], id: &str) -> Option<&'a PreferencePane> {
    categories
        .iter()
        .flat_map(|c| &c.panes)
        .find(|p| p.id.as_ref() == id)
}

/// Lays `content` out at its natural height and reports the viewport height
/// it needs (its bottom edge, which includes the frame's title bar and
/// toolbar above it) after layout. Fixed-size windows resize to this so the
/// overview fits exactly whatever its labels wrap to (HIG p. 244: the window
/// resizes to fit each pane). The report fires every frame; compare it with
/// `window.viewport_size()` before resizing.
pub fn fit_content(
    content: impl IntoElement,
    on_height: impl Fn(Pixels, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div().flex_none().w_full().relative().child(content).child(
        canvas(
            move |bounds, window, cx| on_height(bounds.bottom(), window, cx),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

// --- Toolbar -----------------------------------------------------------------

type NavigateFn = Rc<dyn Fn(Navigation, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct PreferencesToolbar {
    can_back: bool,
    can_forward: bool,
    on_navigate: Option<NavigateFn>,
    search: Option<AnyElement>,
}
impl PreferencesToolbar {
    pub fn new(navigator: &PreferencesNavigator) -> Self {
        Self {
            can_back: navigator.can_go_back(),
            can_forward: navigator.can_go_forward(),
            on_navigate: None,
            search: None,
        }
    }
    pub fn on_navigate(mut self, f: impl Fn(Navigation, &mut Window, &mut App) + 'static) -> Self {
        self.on_navigate = Some(Rc::new(f));
        self
    }
    /// Right-aligned search field (System Preferences' Spotlight field).
    pub fn search(mut self, field: impl IntoElement) -> Self {
        self.search = Some(field.into_any_element());
        self
    }
}
impl RenderOnce for PreferencesToolbar {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let nav = self.on_navigate;
        let history = {
            let nav = nav.clone();
            // Momentary segments: no selection is kept (HIG p. 287 "momentary").
            SegmentedControl::new(
                "preferences-history",
                "Back and Forward",
                vec![
                    Segment::new("back", "\u{25C0}").disabled(!self.can_back),
                    Segment::new("forward", "\u{25B6}").disabled(!self.can_forward),
                ],
            )
            .min_segment_width(14.0)
            .on_select(move |id, window, cx| {
                let to = if id.as_ref() == "back" {
                    Navigation::Back
                } else {
                    Navigation::Forward
                };
                if let Some(f) = &nav {
                    f(to, window, cx);
                }
            })
        };
        div()
            .id("preferences-toolbar")
            .debug_selector(|| "preferences-toolbar".into())
            .size_full()
            .flex()
            .items_center()
            .gap(px(CONTROL_GAP))
            .px(px(8.0))
            // Toolbar presses must not reach the window body (same contract
            // as the Lotus unified toolbar).
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(history)
            .child(
                Button::new("preferences-show-all", "Show All")
                    .style(ButtonStyle::Textured)
                    .size(ControlSize::Small)
                    .on_click(move |_, window, cx| {
                        if let Some(f) = &nav {
                            f(Navigation::ShowAll, window, cx);
                        }
                    }),
            )
            .child(div().flex_1())
            .children(self.search)
    }
}

// --- Overview -----------------------------------------------------------------

type SelectFn = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct PreferencesOverview {
    categories: Vec<PreferenceCategory>,
    on_select: Option<SelectFn>,
}
impl PreferencesOverview {
    pub fn new(categories: Vec<PreferenceCategory>) -> Self {
        Self {
            categories,
            on_select: None,
        }
    }
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}
impl RenderOnce for PreferencesOverview {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let c = &theme.colors;
        let mut root = div()
            .id("preferences-overview")
            .debug_selector(|| "preferences-overview".into())
            .size_full()
            .flex()
            .flex_col()
            .overflow_y_scroll();
        for (index, category) in self.categories.into_iter().enumerate() {
            // M: prefs.overview.bands — window colour, then the group fill.
            let fill = if index % 2 == 0 {
                c.window
            } else {
                c.group_fill
            };
            let mut grid = div()
                .flex()
                .flex_wrap()
                .pl(px(GRID_INSET))
                .pt(px(ICON_TOP - 6.0));
            for pane in category.panes {
                grid = grid.child(cell(pane, self.on_select.clone(), window, cx));
            }
            root = root.child(
                div()
                    .flex_none()
                    .min_h(px(BAND_HEIGHT))
                    .relative()
                    .bg(hsla(fill))
                    .when(index > 0, |el| {
                        el.border_t_1().border_color(hsla(c.band_rule))
                    })
                    .pb(px(6.0))
                    .child(
                        div()
                            .absolute()
                            .top(px(HEADING_TOP))
                            .left(px(HEADING_LEFT))
                            .font_weight(FontWeight::BOLD)
                            .text_size(px(theme.type_.size_regular))
                            .child(category.label),
                    )
                    .child(grid),
            );
        }
        root
    }
}

fn cell(
    pane: PreferencePane,
    on_select: Option<SelectFn>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let id = pane.id.clone();
    let element_id = ElementId::Name(format!("preference-pane-{id}").into());
    let focus: FocusHandle = window
        .use_keyed_state(element_id.clone(), cx, |_, cx| {
            cx.focus_handle().tab_stop(true)
        })
        .read(cx)
        .clone();
    let ring_focus = focus.clone();
    let ring = theme.colors.focus_ring;
    let profile = theme.metrics.focus_ring;
    let icon = pane.icon.as_ref().map(|f| f(window, cx));
    let selector = format!("preference-pane-{id}");
    div()
        .id(element_id)
        .debug_selector(move || selector.clone())
        .role(gpui::Role::Button)
        .aria_label(pane.label.clone())
        .track_focus(&focus)
        .w(px(CELL_WIDTH))
        .h(px(BAND_HEIGHT - ICON_TOP + 4.0))
        .flex()
        .flex_col()
        .items_center()
        .pt(px(6.0))
        .when_some(on_select, |el, f| {
            // GPUI turns Enter/Space on a focused element into a click
            // (div.rs keyboard click), as for Button.
            el.on_click(move |_, window, cx| f(&id, window, cx))
        })
        .child(
            div()
                .relative()
                .size(px(ICON_SIZE))
                .flex_none()
                .children(icon)
                .child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                                paint::focus_ring(window, bounds, 4.0, ring, profile);
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                ),
        )
        .child(
            div()
                .mt(px(LABEL_TOP - ICON_TOP - ICON_SIZE))
                .w(px(CELL_WIDTH - 4.0))
                .text_center()
                .text_size(px(theme.type_.size_small))
                .line_height(px(13.0))
                .child(pane.label),
        )
}

// --- Pane content ---------------------------------------------------------------

/// A pane body: the window body with the measured outer margins.
pub fn pane_body() -> gpui::Div {
    div()
        .size_full()
        .flex()
        .flex_col()
        .px(px(PANE_MARGIN))
        .pb(px(PANE_MARGIN))
}

/// Places a [`TabView`] as a System Preferences pane does: the box's top line
/// 34 px below the toolbar separator, the tab control straddling it
/// (M: prefs.pane.geometry). Put the result inside [`pane_body`], which
/// supplies the 20 px side and bottom margins.
pub fn pane_tab_view(tab_view: TabView, cx: &App) -> gpui::Div {
    let top = if tab_view.has_tabs() {
        BOX_TOP - (cx.theme().metrics.segment_height / 2.0).floor()
    } else {
        BOX_TOP - 14.0
    };
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .pt(px(top))
        .child(tab_view)
}
/// Two-column form (HIG pp. 347-349): labels right-aligned with a colon,
/// controls left-aligned in one column, 8 px between rows.
#[derive(IntoElement)]
pub struct Form {
    label_width: f32,
    rows: Vec<FormRow>,
}
pub struct FormRow {
    label: Option<SharedString>,
    control: AnyElement,
    note: Option<SharedString>,
    id: Option<ElementId>,
}
impl FormRow {
    /// A labelled control. The colon is added here.
    pub fn new(label: impl Into<SharedString>, control: impl IntoElement) -> Self {
        Self {
            label: Some(label.into()),
            control: control.into_any_element(),
            note: None,
            id: None,
        }
    }
    /// A control that carries its own label (checkbox, radio group, button).
    pub fn control(control: impl IntoElement) -> Self {
        Self {
            label: None,
            control: control.into_any_element(),
            note: None,
            id: None,
        }
    }
    /// Small explanatory text under the control.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }
}
impl Form {
    pub fn new(label_width: f32) -> Self {
        Self {
            label_width,
            rows: Vec::new(),
        }
    }
    pub fn row(mut self, row: FormRow) -> Self {
        self.rows.push(row);
        self
    }
    pub fn rows(mut self, rows: impl IntoIterator<Item = FormRow>) -> Self {
        self.rows.extend(rows);
        self
    }
}
impl RenderOnce for Form {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let secondary = hsla(theme.colors.text_secondary);
        let row_height = theme.metrics.push_small_height;
        let small = theme.type_.size_small;
        let label_width = self.label_width;
        div().flex().flex_col().gap(px(CONTROL_GAP)).children(
            self.rows.into_iter().enumerate().map(move |(index, row)| {
                let id = row.id.unwrap_or(ElementId::Integer(index as u64));
                let selector = id.to_string();
                div()
                    .debug_selector(move || selector.clone())
                    .id(id)
                    .flex()
                    .items_start()
                    .gap(px(CONTROL_GAP))
                    .child(
                        div()
                            .flex_none()
                            .w(px(label_width))
                            .min_h(px(row_height))
                            .flex()
                            .items_center()
                            .justify_end()
                            .text_right()
                            .children(row.label.map(|l| format!("{l}:"))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .min_h(px(row_height))
                                    .flex()
                                    .items_center()
                                    .child(row.control),
                            )
                            .children(row.note.map(|note| {
                                div().text_size(px(small)).text_color(secondary).child(note)
                            })),
                    )
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> Option<SharedString> {
        Some(SharedString::from(s.to_owned()))
    }

    #[test]
    fn history_treats_show_all_as_an_entry_and_skips_duplicates() {
        let mut nav = PreferencesNavigator::default();
        assert!(!nav.go(None));
        assert!(nav.go(id("general")));
        assert!(!nav.go(id("general")));
        assert!(nav.go(id("audio")));
        assert!(nav.go_back());
        assert_eq!(nav.current(), id("general").as_ref());
        assert!(nav.can_go_forward());
        assert!(nav.go_back());
        assert_eq!(nav.current(), None);
        assert!(!nav.go_back());
        assert!(nav.go_forward() && nav.go_forward());
        assert_eq!(nav.current(), id("audio").as_ref());
        // A new destination clears forward history.
        nav.go_back();
        nav.go(None);
        assert!(!nav.can_go_forward());
    }

    #[test]
    fn retain_drops_removed_panes_and_returns_to_the_overview() {
        let mut nav = PreferencesNavigator::default();
        nav.go(id("a"));
        nav.go(id("b"));
        nav.go(id("a"));
        nav.retain(|p| p.as_ref() == "a");
        assert_eq!(nav.current(), id("a").as_ref());
        // back was [None, a, b] -> [None, a]
        assert!(nav.go_back());
        assert_eq!(nav.current(), id("a").as_ref());
        nav.retain(|_| false);
        assert_eq!(nav.current(), None);
    }
}
