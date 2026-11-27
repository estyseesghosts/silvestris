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
//! [`PreferencesController`] owns the window-level behavior around those
//! pieces: pane navigation and history, title projection, natural-content
//! measurement after layout, legal resize targets inside the display's usable
//! region, resize animation cancellation, focus return and persistence of the
//! last selected pane. It does not require an overview or categories; a
//! one-pane consumer can use a `WindowRole::Preferences` form directly.
//!
//! Geometry cites reference/measurements.json: prefs.overview.geometry,
//! prefs.overview.bands, prefs.pane.geometry, groupbox.top, groupbox.left.
//! Values not in a capture are marked "estimate".
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui::{
    canvas, div, prelude::*, px, size, AnyElement, App, ElementId, FocusHandle, Pixels,
    SharedString, Size, Window,
};

use crate::binding::Binding;
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

// --- Window coordinator -------------------------------------------------------

/// Estimated duration of a pane-change resize. The 10.6 corpus has no
/// preference-window transition capture, so this is an estimate recorded in
/// `docs/exceptions.md`, not a measurement. It is linear interpolation, not a
/// spring.
pub const PANE_RESIZE: Duration = Duration::from_millis(160);

/// Fixed geometry used to fit a preferences pane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreferencesSizing {
    /// Fixed window width (System Preferences' measured 668 px by default).
    pub width: f32,
    /// Title bar plus fixed preferences toolbar, measured by the host from the
    /// active theme. The coordinator never invents a toolbar height.
    pub chrome_height: f32,
    /// Space kept between the window and the display's usable edges.
    pub display_margin: f32,
    /// Smallest legal window height, so a tiny pane does not collapse the
    /// title bar and toolbar.
    pub min_height: f32,
}

impl PreferencesSizing {
    pub fn new(width: f32, chrome_height: f32) -> Self {
        Self {
            width,
            chrome_height,
            display_margin: 20.0,
            min_height: 120.0,
        }
    }
}

/// A legal window target and whether the pane content must scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreferencesFit {
    pub window: Size<Pixels>,
    /// True when the natural content is taller than the display allows.
    pub scrolls: bool,
}

/// Fit a pane's natural content height into the display's usable region.
///
/// The natural height is the measured pane body plus the host's measured
/// chrome. If it exceeds the display, the window takes the maximum legal
/// height and only the pane content scrolls; the window itself stays fixed.
pub fn fit_pane(
    content_height: f32,
    sizing: PreferencesSizing,
    usable: Size<Pixels>,
) -> PreferencesFit {
    let usable_width = usable.width.as_f32();
    let usable_height = usable.height.as_f32();
    let width = sizing
        .width
        .min((usable_width - 2.0 * sizing.display_margin).max(0.0));
    let max_height = (usable_height - 2.0 * sizing.display_margin).max(0.0);
    let natural = (sizing.chrome_height + content_height).max(0.0);
    let scrolls = natural > max_height;
    let height = natural
        .min(max_height)
        .max(sizing.min_height.min(max_height));
    PreferencesFit {
        window: size(px(width), px(height)),
        scrolls,
    }
}

/// One linear resize from `from` to `to`. A retarget measures the current
/// position and starts again, so an outstanding A→B animation cannot win over
/// a later A→C request.
#[derive(Clone, Copy, Debug)]
pub struct PreferencesResize {
    from: Size<Pixels>,
    to: Size<Pixels>,
    started: Instant,
    duration: Duration,
}

impl PreferencesResize {
    pub fn new(from: Size<Pixels>, to: Size<Pixels>, started: Instant, duration: Duration) -> Self {
        Self {
            from,
            to,
            started,
            duration,
        }
    }

    /// The interpolated window size at `now`.
    pub fn at(&self, now: Instant) -> Size<Pixels> {
        let elapsed = now.saturating_duration_since(self.started).as_secs_f32();
        let total = self.duration.as_secs_f32();
        let t = if total <= f32::EPSILON {
            1.0
        } else {
            (elapsed / total).clamp(0.0, 1.0)
        };
        let mix = |a: Pixels, b: Pixels| a + (b - a) * t;
        size(
            mix(self.from.width, self.to.width),
            mix(self.from.height, self.to.height),
        )
    }

    pub fn is_done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.started) >= self.duration
    }

    pub fn target(&self) -> Size<Pixels> {
        self.to
    }

    /// Restart from the current interpolated position toward `target`.
    pub fn retarget(&mut self, target: Size<Pixels>, now: Instant) {
        self.from = self.at(now);
        self.to = target;
        self.started = now;
    }
}

type LoadPane = Rc<dyn Fn(&App) -> Option<SharedString>>;
type SavePane = Rc<dyn Fn(Option<&SharedString>, &mut App)>;

/// Owns the behavior around a preferences window's pane widgets.
///
/// The host owns the window, toolbar and pane rendering; the controller owns
/// which pane is current, its title, the measured target size, cancellation of
/// an outstanding resize, focus return and the stored selection. It works with
/// or without an overview: pass the panes that exist and render whatever the
/// current id selects.
pub struct PreferencesController {
    navigator: PreferencesNavigator,
    panes: Vec<PreferencePane>,
    overview_title: SharedString,
    sizing: PreferencesSizing,
    duration: Duration,
    resize: Option<PreferencesResize>,
    target: Option<Size<Pixels>>,
    scrolls: bool,
    focus_return: Option<FocusHandle>,
    load: Option<LoadPane>,
    save: Option<SavePane>,
}

impl PreferencesController {
    pub fn new(
        overview_title: impl Into<SharedString>,
        panes: Vec<PreferencePane>,
        sizing: PreferencesSizing,
    ) -> Self {
        Self {
            navigator: PreferencesNavigator::default(),
            panes,
            overview_title: overview_title.into(),
            sizing,
            duration: PANE_RESIZE,
            resize: None,
            target: None,
            scrolls: false,
            focus_return: None,
            load: None,
            save: None,
        }
    }

    /// Restore a previously stored selection. Unknown panes fall back to the
    /// overview. Call once while building the window.
    pub fn with_persistence(
        mut self,
        load: impl Fn(&App) -> Option<SharedString> + 'static,
        save: impl Fn(Option<&SharedString>, &mut App) + 'static,
    ) -> Self {
        self.load = Some(Rc::new(load));
        self.save = Some(Rc::new(save));
        self
    }

    /// Apply the stored selection without adding history.
    pub fn restore(&mut self, cx: &App) -> bool {
        let Some(load) = &self.load else {
            return false;
        };
        let Some(saved) = load(cx) else {
            return false;
        };
        if self.panes.iter().any(|pane| pane.id == saved) {
            self.navigator = PreferencesNavigator::new(Some(saved));
            true
        } else {
            false
        }
    }

    /// Focus target that receives focus after a pane change. A pane-switch
    /// rebuilds content, so the host supplies the control that should regain
    /// focus (its search field or first control).
    pub fn focus_return(mut self, focus: FocusHandle) -> Self {
        self.focus_return = Some(focus);
        self
    }

    /// Override the estimated resize duration (animation tests use this).
    pub fn duration(mut self, duration: Duration) -> Self {
        self.duration = duration;
        self
    }

    #[must_use]
    pub fn navigator(&self) -> &PreferencesNavigator {
        &self.navigator
    }

    #[must_use]
    pub fn pane_scrolls(&self) -> bool {
        self.scrolls
    }

    #[must_use]
    pub fn current(&self) -> Option<&SharedString> {
        self.navigator.current()
    }

    #[must_use]
    pub fn title(&self) -> SharedString {
        self.navigator
            .current()
            .and_then(|id| self.panes.iter().find(|pane| pane.id == *id))
            .map(|pane| pane.label.clone())
            .unwrap_or_else(|| self.overview_title.clone())
    }

    /// Select a pane (`Some`) or the overview (`None`). Updates the window
    /// title, stores the selection and returns focus to the supplied target.
    pub fn select(&mut self, to: Option<SharedString>, window: &mut Window, cx: &mut App) -> bool {
        if !self.navigator.go(to) {
            return false;
        }
        self.projected(window, cx);
        true
    }

    pub fn show_all(&mut self, window: &mut Window, cx: &mut App) -> bool {
        self.select(None, window, cx)
    }

    pub fn go_back(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if !self.navigator.go_back() {
            return false;
        }
        self.projected(window, cx);
        true
    }

    pub fn go_forward(&mut self, window: &mut Window, cx: &mut App) -> bool {
        if !self.navigator.go_forward() {
            return false;
        }
        self.projected(window, cx);
        true
    }

    fn projected(&mut self, window: &mut Window, cx: &mut App) {
        window.set_window_title(self.title().as_ref());
        if let Some(focus) = &self.focus_return {
            window.focus(focus, cx);
        }
        if let Some(save) = &self.save {
            save(self.navigator.current(), cx);
        }
    }

    /// Drop panes that no longer exist, falling back to the overview if the
    /// current pane disappeared. The caller repaints.
    pub fn retain(
        &mut self,
        exists: impl Fn(&SharedString) -> bool,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let before = self.navigator.current().cloned();
        self.navigator.retain(exists);
        if self.navigator.current().cloned() != before {
            self.projected(window, cx);
            return true;
        }
        false
    }

    /// Measure the pane content after layout. `content` is the pane body's
    /// natural size; the host's `sizing.chrome_height` supplies the title bar
    /// and fixed toolbar. The target is clamped to the display's usable region.
    /// A same-target frame does not restart an outstanding animation.
    pub fn measure(&mut self, content: Size<Pixels>, window: &mut Window, cx: &mut App) {
        let usable = window
            .display(cx)
            .map(|display| display.visible_bounds().size)
            .unwrap_or_else(|| window.viewport_size());
        let fit = fit_pane(content.height.as_f32(), self.sizing, usable);
        self.scrolls = fit.scrolls;
        let current = window.viewport_size();
        if fit.window == current {
            self.resize = None;
            self.target = Some(fit.window);
            return;
        }
        if self.target == Some(fit.window) {
            return;
        }
        self.target = Some(fit.window);
        let now = Instant::now();
        if cx.reduce_motion() {
            self.resize = None;
            window.resize(fit.window);
        } else {
            self.resize = Some(PreferencesResize::new(
                current,
                fit.window,
                now,
                self.duration,
            ));
            window.request_animation_frame();
        }
    }

    /// Advance the resize. Call from the host's render while it returns true.
    /// Returns whether another frame is needed.
    pub fn tick(&mut self, window: &mut Window) -> bool {
        let Some(resize) = self.resize else {
            return false;
        };
        let now = Instant::now();
        let done = resize.is_done(now);
        let next = resize.at(now);
        window.resize(next);
        if done {
            self.resize = None;
            self.target = Some(next);
            false
        } else {
            window.request_animation_frame();
            true
        }
    }
}

/// Wrap pane content so its natural size is reported after layout. Pass the
/// reported size to [`PreferencesController::measure`].
pub fn measure_content(
    content: impl IntoElement,
    on_size: impl Fn(Size<Pixels>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div().flex_none().w_full().relative().child(content).child(
        canvas(
            move |bounds, window, cx| on_size(bounds.size, window, cx),
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0(),
    )
}

/// The pane viewport. Only the pane content scrolls when the display cannot
/// fit its natural height; the fixed preferences window itself does not grow
/// past the usable region.
pub fn pane_viewport(content: impl IntoElement, scrolls: bool) -> impl IntoElement {
    div()
        .id("preferences-pane-viewport")
        .w_full()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .when(scrolls, |el| el.overflow_y_scroll())
        .child(content)
}

/// A natural-height pane body for measurement. [`pane_body`] keeps filling the
/// window; use this variant inside [`measure_content`] so the body reports its
/// required height rather than the viewport height.
pub fn pane_body_natural() -> gpui::Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .px(px(PANE_MARGIN))
        .pb(px(PANE_MARGIN))
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
                            .font_weight(crate::theme::heading_font_weight(&theme))
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
/// Explicit row state for a form row (HIG pp. 240-241 validation moments;
/// pp. 347-350 preferences layout).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FormRowState {
    #[default]
    Normal,
    /// The row depends on another setting and is not currently usable.
    Disabled,
    /// An informational note below the control.
    Note,
    /// A non-blocking warning below the control.
    Warning,
    /// A validation error below the control.
    Error,
}

pub struct FormRow {
    label: Option<SharedString>,
    control: AnyElement,
    note: Option<SharedString>,
    state: FormRowState,
    dependent: Option<Binding<bool>>,
    disabled: bool,
    id: Option<ElementId>,
}
impl FormRow {
    /// A labelled control. The colon is added here.
    pub fn new(label: impl Into<SharedString>, control: impl IntoElement) -> Self {
        Self {
            label: Some(label.into()),
            control: control.into_any_element(),
            note: None,
            state: FormRowState::Normal,
            dependent: None,
            disabled: false,
            id: None,
        }
    }
    /// A control that carries its own label (checkbox, radio group, button).
    pub fn control(control: impl IntoElement) -> Self {
        Self {
            label: None,
            control: control.into_any_element(),
            note: None,
            state: FormRowState::Normal,
            dependent: None,
            disabled: false,
            id: None,
        }
    }
    /// Small explanatory text under the control. Keeps a warning or error if
    /// one has already been set.
    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        if self.state == FormRowState::Normal {
            self.state = FormRowState::Note;
        }
        self
    }
    /// A non-blocking warning in the standard message slot.
    pub fn warning(mut self, message: impl Into<SharedString>) -> Self {
        self.note = Some(message.into());
        self.state = FormRowState::Warning;
        self
    }
    /// A validation error in the standard message slot. The row keeps its
    /// control and value; only the message and state change.
    pub fn error(mut self, message: impl Into<SharedString>) -> Self {
        self.note = Some(message.into());
        self.state = FormRowState::Error;
        self
    }
    /// Disable the row explicitly. The consumer still passes a disabled
    /// control when the control owns interactive semantics.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        if disabled && matches!(self.state, FormRowState::Normal | FormRowState::Note) {
            self.state = FormRowState::Disabled;
        }
        self
    }
    /// Follow a model value without rebuilding the form: the row is disabled
    /// whenever the binding is false.
    pub fn dependent_on(mut self, enabled: Binding<bool>) -> Self {
        self.dependent = Some(enabled);
        self
    }
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    fn is_enabled(&self, cx: &App) -> bool {
        !self.disabled
            && self
                .dependent
                .as_ref()
                .is_none_or(|binding| binding.value(cx))
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
        let warning = hsla(theme.colors.warning);
        let error = hsla(theme.colors.error);
        let on_color = hsla(theme.colors.text_on_accent);
        let row_height = theme.metrics.push_small_height;
        let small = theme.type_.size_small;
        let label_width = self.label_width;
        div().flex().flex_col().gap(px(CONTROL_GAP)).children(
            self.rows.into_iter().enumerate().map(move |(index, row)| {
                let enabled = row.is_enabled(cx);
                let id = row.id.unwrap_or(ElementId::Integer(index as u64));
                let selector = id.to_string();
                let message_color = match row.state {
                    FormRowState::Warning => warning,
                    FormRowState::Error => error,
                    _ => secondary,
                };
                let marked = matches!(row.state, FormRowState::Warning | FormRowState::Error);
                let marker = marked.then(|| {
                    div()
                        .size(px(12.0))
                        .flex_none()
                        .rounded_full()
                        .bg(message_color)
                        .text_color(on_color)
                        .text_size(px(9.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child("!")
                });
                let message_selector = format!("{selector}-message");
                div()
                    .debug_selector(move || selector.clone())
                    .id(id)
                    .flex()
                    .items_start()
                    .gap(px(CONTROL_GAP))
                    .when(!enabled, |el| el.opacity(0.55))
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
                                div()
                                    .debug_selector(move || message_selector.clone())
                                    .flex()
                                    .items_start()
                                    .gap(px(4.0))
                                    .text_size(px(small))
                                    .text_color(message_color)
                                    .children(marker)
                                    .child(div().min_w_0().child(note))
                            })),
                    )
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Checkbox;
    use gpui::{Context, Render, TestAppContext};

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

    fn sizing() -> PreferencesSizing {
        PreferencesSizing {
            width: 668.0,
            chrome_height: 54.0,
            display_margin: 20.0,
            min_height: 120.0,
        }
    }

    #[test]
    fn fit_pane_grows_to_the_natural_content() {
        let fit = fit_pane(240.0, sizing(), size(px(1440.), px(900.)));
        assert_eq!(fit.window.width.as_f32(), 668.0);
        assert_eq!(fit.window.height.as_f32(), 294.0);
        assert!(!fit.scrolls);
    }

    #[test]
    fn fit_pane_clamps_to_the_display_and_marks_the_pane_scrollable() {
        let fit = fit_pane(2000.0, sizing(), size(px(800.), px(600.)));
        assert_eq!(fit.window.height.as_f32(), 560.0);
        assert!(fit.scrolls);
        let narrow = fit_pane(200.0, sizing(), size(px(640.), px(600.)));
        assert_eq!(narrow.window.width.as_f32(), 600.0);
    }

    #[test]
    fn retargeting_starts_from_the_current_position_not_the_old_target() {
        let start = Instant::now();
        let mut resize = PreferencesResize::new(
            size(px(100.), px(100.)),
            size(px(200.), px(200.)),
            start,
            Duration::from_millis(100),
        );
        let halfway = start + Duration::from_millis(50);
        assert_eq!(resize.at(halfway).width.as_f32(), 150.0);
        resize.retarget(size(px(300.), px(300.)), halfway);
        let later = halfway + Duration::from_millis(50);
        assert_eq!(resize.at(later).width.as_f32(), 225.0);
        assert!(!resize.is_done(later));
        assert!(resize.is_done(halfway + Duration::from_millis(100)));
    }

    #[test]
    fn controller_projects_the_pane_title_and_retains_only_existing_panes() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
        });
        let (_view, cx) = app.add_window_view(|_, _| Empty);
        let focus = cx.update(|_, cx| cx.focus_handle());
        let mut controller = PreferencesController::new(
            "Preferences",
            vec![
                PreferencePane::new("general", "General"),
                PreferencePane::new("audio", "Sound"),
            ],
            sizing(),
        )
        .focus_return(focus.clone());
        cx.update(|window, cx| {
            assert!(controller.select(id("audio"), window, cx));
            assert_eq!(controller.title().as_ref(), "Sound");
            assert!(focus.is_focused(window));
        });
        assert!(cx.update(|window, cx| controller.retain(
            |pane| pane.as_ref() == "general",
            window,
            cx
        )));
        assert_eq!(controller.title().as_ref(), "Preferences");
    }

    #[test]
    fn controller_restores_and_saves_the_last_pane() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
        });
        let saved = std::rc::Rc::new(std::cell::RefCell::new(Some(SharedString::from("audio"))));
        let read = saved.clone();
        let written = saved.clone();
        let mut controller = PreferencesController::new(
            "Preferences",
            vec![
                PreferencePane::new("general", "General"),
                PreferencePane::new("audio", "Sound"),
            ],
            sizing(),
        )
        .with_persistence(
            move |_| read.borrow().clone(),
            move |selection, _| *written.borrow_mut() = selection.cloned(),
        );
        app.update(|cx| assert!(controller.restore(cx)));
        assert_eq!(controller.title().as_ref(), "Sound");
        let (_view, cx) = app.add_window_view(|_, _| Empty);
        cx.update(|window, cx| {
            assert!(controller.select(id("general"), window, cx));
        });
        assert_eq!(saved.borrow().as_ref().map(|s| s.as_ref()), Some("general"));
    }

    #[test]
    fn measure_resizes_to_the_natural_pane_or_scrolls_when_the_display_is_too_small() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
            cx.set_reduce_motion(true);
        });
        let (_view, cx) = app.add_window_view(|_, _| Empty);
        cx.simulate_resize(size(px(668.), px(200.)));
        let mut controller = PreferencesController::new(
            "Preferences",
            vec![PreferencePane::new("general", "General")],
            sizing(),
        );
        cx.update(|window, cx| {
            controller.measure(size(px(500.), px(120.)), window, cx);
            assert!(!controller.pane_scrolls());
            controller.measure(size(px(500.), px(2000.)), window, cx);
            assert!(controller.pane_scrolls());
        });
    }

    struct Empty;

    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    struct FormHost {
        dependent: Binding<bool>,
    }

    impl Render for FormHost {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            Form::new(90.0)
                .row(
                    FormRow::new(
                        "Font",
                        Checkbox::new("font-control", "Monospaced").checked(true),
                    )
                    .id("font")
                    .error("Unknown font family"),
                )
                .row(
                    FormRow::new(
                        "Spacing",
                        Checkbox::new("spacing-control", "Automatic").checked(false),
                    )
                    .id("spacing")
                    .note("Applies to new documents"),
                )
                .row(
                    FormRow::control(
                        Checkbox::new("dependent-control", "Use custom size").checked(true),
                    )
                    .id("dependent")
                    .dependent_on(self.dependent.clone()),
                )
        }
    }

    #[test]
    fn form_rows_expose_notes_and_errors_in_one_message_slot() {
        let mut app = TestAppContext::single();
        app.update(|cx| {
            crate::theme::install(crate::ThemePreference::default(), cx);
        });
        let dependent = {
            let state = std::rc::Rc::new(std::cell::Cell::new(false));
            let read = state.clone();
            let write = state;
            Binding::new(move |_| read.get(), move |value, _| write.set(value))
        };
        let (_view, cx) = app.add_window_view(move |_, _| FormHost { dependent });
        cx.simulate_resize(size(px(500.), px(400.)));
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("font-control").is_some());
        assert!(cx.debug_bounds("font-message").is_some());
        assert!(cx.debug_bounds("spacing-message").is_some());
        assert!(cx.debug_bounds("dependent-control").is_some());
        let control = cx.debug_bounds("font-control").expect("control");
        let message = cx.debug_bounds("font-message").expect("message");
        assert!(message.origin.x <= control.origin.x + px(1.0));
    }
}
