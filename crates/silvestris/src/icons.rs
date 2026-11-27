//! Typed artwork roles for file objects, template images and consumer artwork.
//!
//! File/object artwork stays full-colour and preserves its selected, inactive
//! and disabled rendering. Template images are monochrome and receive control
//! state ink from their host. Consumer artwork is supplied by the application
//! as a renderer and is constrained to the documented 32 px pictorial family.
//!
//! The artwork matrix is deliberate:
//!
//! |  | Silvestris-provided | Application-provided |
//! |---|---|---|
//! | Monochrome/template | [`TemplateImage`] | [`ConsumerTemplateImage`] |
//! | Full-colour/pictorial | [`PictorialImage`] | [`ConsumerArtwork`] |
//!
//! [`FileArtwork`] is a separate vocabulary: a file icon is full-colour, but it
//! is not generic toolbar pictorial artwork and the two are never exchanged.

use gpui::{
    canvas, div, point, prelude::*, px, AnyElement, App, Bounds, PathBuilder, Pixels, Point,
    SharedString, Window,
};
use silvestris_core::{Rgba8, Theme, ThemeFamily};

use crate::geist_pictograms::GEIST_PICTOGRAMS;
use std::rc::Rc;

use crate::convert::hsla;

/// Full-colour artwork representing files and other objects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FileArtwork {
    Folder,
    Home,
    Desktop,
    Documents,
    Volume,
    Network,
    SmartSearch,
    Document,
    Application,
}

/// Compatibility name for the file-panel artwork vocabulary.
pub type Icon = FileArtwork;

/// Monochrome system-image roles. The renderer uses host-provided ink so a
/// selected, inactive or disabled control is not baked into the artwork.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TemplateImage {
    Back,
    Forward,
    IconView,
    ListView,
    ColumnView,
    Search,
    Stop,
    Refresh,
    Add,
    Remove,
    Action,
    Info,
    Sidebar,
    Inspector,
    Eject,
    /// Provisional toolbar overflow affordance; replace when a measured 10.6
    /// capture is available.
    Overflow,
}

/// Fixed template-image slots supported by the Aqua toolbar hosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplateImageSlot {
    /// Rectangular toolbar image; HIG maximum is 19×19 px.
    Toolbar,
    /// Small image used inside a segmented control.
    Segmented,
}

impl TemplateImageSlot {
    /// Logical side length reserved by this host.
    #[must_use]
    pub const fn logical_size(self) -> f32 {
        match self {
            Self::Toolbar => 19.0,
            Self::Segmented => 13.0,
        }
    }
}

/// Standard Geist interface pictograms.
///
/// These roles are generic: a consumer maps its own categories to them in its
/// own code. Silvestris never recognises an application name. Geometry is
/// prepared once from the pinned Lucide release in `icons/lucide`, with a 2 px
/// nominal stroke at a 24 px source size and round caps and joins.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GeistPictogram {
    Settings,
    Database,
    MessageSquare,
    Monitor,
    Volume2,
    Gamepad2,
    Bug,
    Bot,
    Wrench,
    SlidersHorizontal,
}

impl GeistPictogram {
    /// The pinned Lucide role name.
    #[must_use]
    pub const fn role(self) -> &'static str {
        match self {
            Self::Settings => "settings",
            Self::Database => "database",
            Self::MessageSquare => "message-square",
            Self::Monitor => "monitor",
            Self::Volume2 => "volume-2",
            Self::Gamepad2 => "gamepad-2",
            Self::Bug => "bug",
            Self::Bot => "bot",
            Self::Wrench => "wrench",
            Self::SlidersHorizontal => "sliders-horizontal",
        }
    }

    /// All standard roles, in gallery order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Settings,
            Self::Database,
            Self::MessageSquare,
            Self::Monitor,
            Self::Volume2,
            Self::Gamepad2,
            Self::Bug,
            Self::Bot,
            Self::Wrench,
            Self::SlidersHorizontal,
        ]
    }

    fn paths(self) -> &'static [crate::geist_pictograms::PictogramPath] {
        let role = self.role();
        let (_, _, paths) = GEIST_PICTOGRAMS
            .iter()
            .find(|(name, _, _)| *name == role)
            .expect("generated pictogram for every Geist role");
        paths
    }

    /// Draw the glyph in host-supplied ink at a visible size. The full slot is
    /// still owned by the host so alignment and hit targets do not move.
    #[must_use]
    pub fn element(self, visible_size: f32, ink: Rgba8) -> AnyElement {
        let scale = visible_size / 24.0;
        let paths = self.paths();
        div()
            .size(px(visible_size))
            .child(canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    for path in paths {
                        let mut builder = PathBuilder::stroke(px(2.0 * scale));
                        let points = path.points;
                        let Some(&(first_x, first_y)) = points.first() else {
                            continue;
                        };
                        let at = |x: f32, y: f32| {
                            point(
                                bounds.origin.x + px(x * scale),
                                bounds.origin.y + px(y * scale),
                            )
                        };
                        builder.move_to(at(first_x, first_y));
                        for &(x, y) in &points[1..] {
                            builder.line_to(at(x, y));
                        }
                        if let Ok(path) = builder.build() {
                            window.paint_path(path, hsla(ink));
                        }
                    }
                },
            ))
            .into_any_element()
    }

    /// Resolve ink from theme tokens and control state, then draw the glyph.
    /// Aqua keeps its original artwork; this helper only supplies Geist's
    /// monochrome presentation when the active theme is Geist.
    #[must_use]
    pub fn themed_element(
        self,
        visible_size: f32,
        state: PictorialState,
        theme: &Theme,
    ) -> AnyElement {
        if theme.id.family != ThemeFamily::Geist {
            return div().size(px(visible_size)).into_any_element();
        }
        let ink = if !state.enabled {
            theme.colors.text_disabled
        } else if !state.active_window {
            theme.colors.text_secondary
        } else {
            theme.colors.text
        };
        self.element(visible_size, ink)
    }
}

/// The state a full-colour pictorial artwork receives. The consumer chooses
/// how `pressed` and `selected` look, but must not recolour artwork with the
/// control ink: colour artwork keeps its designed colours in every state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PictorialState {
    /// The command is available.
    pub enabled: bool,
    /// The window is key.
    pub active_window: bool,
    /// The control is pressed by pointer or keyboard.
    pub pressed: bool,
    /// A toggle or segment is on.
    pub selected: bool,
}

impl PictorialState {
    #[must_use]
    pub const fn normal() -> Self {
        Self {
            enabled: true,
            active_window: true,
            pressed: false,
            selected: false,
        }
    }

    #[must_use]
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::normal()
        }
    }

    #[must_use]
    pub const fn inactive() -> Self {
        Self {
            active_window: false,
            ..Self::normal()
        }
    }

    /// Common Silvestris dimming for disabled and inactive windows. Pressed
    /// darkens the artwork; selected is the consumer's decision.
    fn dim(self) -> f32 {
        if !self.enabled {
            0.55
        } else if !self.active_window {
            0.22
        } else {
            0.0
        }
    }

    fn shade(self) -> f32 {
        if self.pressed {
            0.12
        } else {
            0.0
        }
    }
}

impl Default for PictorialState {
    fn default() -> Self {
        Self::normal()
    }
}

type ArtworkRenderer = Rc<dyn Fn(PictorialState, &mut Window, &mut App) -> AnyElement>;

/// Original pictorial artwork supplied by a consuming application.
///
/// This adapter deliberately accepts a renderer instead of a Silvestris
/// application-specific icon name. Toolbar and preference-category hosts
/// constrain the artwork to the documented 32×32 px family. [`Self::new`]
/// ignores the pictorial state; [`Self::with_state`] receives it.
#[derive(Clone)]
pub struct ConsumerArtwork {
    label: SharedString,
    render: ArtworkRenderer,
}

impl ConsumerArtwork {
    /// Registers an application-owned artwork renderer with an external label.
    pub fn new<E: IntoElement>(
        label: impl Into<SharedString>,
        render: impl Fn(&mut Window, &mut App) -> E + 'static,
    ) -> Self {
        Self::with_state(label, move |_, window, cx| render(window, cx))
    }

    /// Registers state-aware artwork for a standalone toolbar command.
    pub fn with_state<E: IntoElement>(
        label: impl Into<SharedString>,
        render: impl Fn(PictorialState, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            render: Rc::new(move |state, window, cx| render(state, window, cx).into_any_element()),
        }
    }

    /// Descriptive name for accessibility and gallery presentation.
    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// Pictorial toolbar artwork in the standard 32×32 px slot at rest.
    pub fn toolbar_element(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        self.stateful_toolbar_element(PictorialState::normal(), window, cx)
    }

    /// Pictorial toolbar artwork in the standard 32×32 px slot for a state.
    pub fn stateful_toolbar_element(
        &self,
        state: PictorialState,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        self.sized_element(32.0, state, window, cx)
    }

    /// Preference-category artwork in the standard 32×32 px slot.
    pub fn category_element(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        self.sized_element(32.0, PictorialState::normal(), window, cx)
    }

    /// Renderer callback for views that collect artwork before rendering.
    pub fn category_renderer(&self) -> impl Fn(&mut Window, &mut App) -> AnyElement + 'static {
        let artwork = self.clone();
        move |window, cx| artwork.category_element(window, cx)
    }

    fn sized_element(
        &self,
        size: f32,
        state: PictorialState,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_center()
            .flex_none()
            .size(px(size))
            .child((self.render)(state, window, cx))
            .into_any_element()
    }
}

type TemplatePainter = Rc<dyn Fn(Bounds<Pixels>, Rgba8, &mut Window)>;

/// Monochrome artwork supplied by a consuming application.
///
/// The painter receives the fixed host slot's bounds and the control ink for
/// the current state. Silvestris owns the 19 px toolbar / 13 px segmented
/// slots, enabled/disabled/selected/inactive ink, scaling and alignment; the
/// application owns only the silhouette.
#[derive(Clone)]
pub struct ConsumerTemplateImage {
    label: SharedString,
    paint: TemplatePainter,
}

impl ConsumerTemplateImage {
    /// Registers an application-owned silhouette that paints with host ink.
    pub fn new(
        label: impl Into<SharedString>,
        paint: impl Fn(Bounds<Pixels>, Rgba8, &mut Window) + 'static,
    ) -> Self {
        Self {
            label: label.into(),
            paint: Rc::new(paint),
        }
    }

    /// Descriptive name for tests and gallery presentation.
    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }

    /// Render in the fixed slot for a toolbar or segmented control.
    pub fn element(&self, slot: TemplateImageSlot, color: Rgba8) -> AnyElement {
        let size = slot.logical_size();
        let paint = self.paint.clone();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| paint(bounds, color, window),
        )
        .flex_none()
        .size(px(size))
        .into_any_element()
    }

    /// Renderer callback for `Button::icon`, preserving the button's state ink.
    pub fn button_icon_renderer(
        &self,
        slot: TemplateImageSlot,
    ) -> impl Fn(gpui::Rgba) -> AnyElement + 'static {
        let image = self.clone();
        move |ink| image.element(slot, rgba8(ink))
    }
}

/// How a full-colour file icon is drawn in its row or cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileIconState {
    /// The row is selected. 10.6 keeps the icon coloured; the field is here so
    /// callers state the state and a future corpus capture can refine it.
    pub selected: bool,
    /// The window is inactive: desaturate slightly.
    pub inactive: bool,
    /// A disallowed file type in Save: desaturate strongly.
    pub disabled: bool,
}

impl FileIconState {
    #[must_use]
    pub const fn normal() -> Self {
        Self {
            selected: false,
            inactive: false,
            disabled: false,
        }
    }

    #[must_use]
    pub const fn selected() -> Self {
        Self {
            selected: true,
            inactive: false,
            disabled: false,
        }
    }

    fn dim(self) -> f32 {
        if self.disabled {
            0.55
        } else if self.inactive {
            0.22
        } else {
            0.0
        }
    }
}

impl FileArtwork {
    /// Paint a full-colour file icon into `bounds`.
    pub fn file_paint(self, window: &mut Window, bounds: Bounds<Pixels>, state: FileIconState) {
        let f = Frame {
            b: bounds,
            dim: state.dim(),
            shade: 0.0,
        };
        match self {
            FileArtwork::Folder => f.folder(window),
            FileArtwork::Home => f.home(window),
            FileArtwork::Desktop => f.desktop(window),
            FileArtwork::Documents => f.documents(window),
            FileArtwork::Volume => f.volume(window),
            FileArtwork::Network => f.network(window),
            FileArtwork::SmartSearch => f.smart_search(window),
            FileArtwork::Document => f.document(window),
            FileArtwork::Application => f.application(window),
        }
    }

    /// A sized full-colour icon for the sidebar, list cell or grid cell.
    pub fn file_element(self, size: f32, state: FileIconState) -> AnyElement {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| self.file_paint(window, bounds, state),
        )
        .flex_none()
        .size(px(size))
        .into_any_element()
    }

    /// The role string shared with `SourceNode::icon` and
    /// the file-panel model.
    #[must_use]
    pub fn role(self) -> &'static str {
        match self {
            FileArtwork::Folder => "folder",
            FileArtwork::Home => "home",
            FileArtwork::Desktop => "desktop",
            FileArtwork::Documents => "documents",
            FileArtwork::Volume => "volume",
            FileArtwork::Network => "network",
            FileArtwork::SmartSearch => "smart-search",
            FileArtwork::Document => "document",
            FileArtwork::Application => "application",
        }
    }

    /// Parse a file/object artwork role string.
    pub fn from_role(role: &str) -> Option<Self> {
        Some(match role {
            "folder" => FileArtwork::Folder,
            "home" => FileArtwork::Home,
            "desktop" => FileArtwork::Desktop,
            "documents" => FileArtwork::Documents,
            "volume" => FileArtwork::Volume,
            "network" | "network-volume" => FileArtwork::Network,
            "smart-search" => FileArtwork::SmartSearch,
            "document" => FileArtwork::Document,
            "application" => FileArtwork::Application,
            _ => return None,
        })
    }
}

/// Original, reusable full-colour toolbar artwork. Deliberately small: add a
/// variant only when a Silvestris consumer or reference application needs it.
/// No Apple artwork is traced or recoloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PictorialImage {
    /// A new document/item.
    New,
    /// Open an existing document/item.
    Open,
    /// Save the current document.
    Save,
    /// Preferences/general settings.
    Preferences,
}

impl PictorialImage {
    /// Stable role string for tests and gallery labels.
    #[must_use]
    pub fn role(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::Open => "open",
            Self::Save => "save",
            Self::Preferences => "preferences",
        }
    }

    /// Fixed 32 px slot; the host never lets the application resize it.
    pub const SLOT: f32 = 32.0;

    /// Paint the artwork into `bounds` with the common pictorial state
    /// treatment.
    pub fn paint(self, window: &mut Window, bounds: Bounds<Pixels>, state: PictorialState) {
        let frame = Frame {
            b: bounds,
            dim: state.dim(),
            shade: state.shade(),
        };
        match self {
            Self::New => frame.pictorial_new(window),
            Self::Open => frame.pictorial_open(window),
            Self::Save => frame.pictorial_save(window),
            Self::Preferences => frame.pictorial_preferences(window),
        }
    }

    /// A sized pictorial element for a standalone toolbar icon button.
    pub fn element(self, state: PictorialState) -> AnyElement {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| self.paint(window, bounds, state),
        )
        .flex_none()
        .size(px(Self::SLOT))
        .into_any_element()
    }
}

/// Convert GPUI ink to the core colour type used by painters.
fn rgba8(ink: gpui::Rgba) -> Rgba8 {
    Rgba8 {
        r: (ink.r * 255.0).round().clamp(0.0, 255.0) as u8,
        g: (ink.g * 255.0).round().clamp(0.0, 255.0) as u8,
        b: (ink.b * 255.0).round().clamp(0.0, 255.0) as u8,
        a: (ink.a * 255.0).round().clamp(0.0, 255.0) as u8,
    }
}

impl TemplateImage {
    /// Stable role string accepted by `SegmentedControl::icon`.
    #[must_use]
    pub fn role(self) -> &'static str {
        match self {
            Self::Back => "back",
            Self::Forward => "forward",
            Self::IconView => "icon",
            Self::ListView => "list",
            Self::ColumnView => "columns",
            Self::Search => "search",
            Self::Stop => "stop",
            Self::Refresh => "refresh",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Action => "action",
            Self::Info => "info",
            Self::Sidebar => "sidebar",
            Self::Inspector => "inspector",
            Self::Eject => "eject",
            Self::Overflow => "overflow",
        }
    }

    /// Resolves the stable role strings used by segmented controls.
    pub fn from_role(role: &str) -> Option<Self> {
        Some(match role {
            "back" => Self::Back,
            "forward" => Self::Forward,
            "icon" | "icon-view" => Self::IconView,
            "list" | "list-view" => Self::ListView,
            "columns" | "column-browser" | "column-view" => Self::ColumnView,
            "search" => Self::Search,
            "stop" => Self::Stop,
            "refresh" => Self::Refresh,
            "add" => Self::Add,
            "remove" => Self::Remove,
            "action" => Self::Action,
            "info" => Self::Info,
            "sidebar" => Self::Sidebar,
            "inspector" => Self::Inspector,
            "eject" => Self::Eject,
            "overflow" => Self::Overflow,
            _ => return None,
        })
    }

    /// Paint this template glyph using the ink supplied by its control.
    pub fn paint(self, window: &mut Window, bounds: Bounds<Pixels>, color: Rgba8) {
        let frame = Frame {
            b: bounds,
            dim: 0.0,
            shade: 0.0,
        };
        match self {
            Self::Back => frame.chevron(color, false, window),
            Self::Forward => frame.chevron(color, true, window),
            Self::IconView => {
                for (x, y) in [(0.10, 0.14), (0.54, 0.14), (0.10, 0.56), (0.54, 0.56)] {
                    frame.rect(x, y, 0.30, 0.30, 0.8, color, window);
                }
            }
            Self::ListView => {
                for row in 0..3 {
                    let y = 0.22 + row as f32 * 0.22;
                    frame.rect(0.10, y + 0.01, 0.09, 0.07, 0.5, color, window);
                    frame.rect(0.27, y + 0.02, 0.63, 0.05, 0.0, color, window);
                }
            }
            Self::ColumnView => {
                for x in [0.08, 0.38, 0.68] {
                    frame.rect(x, 0.16, 0.24, 0.68, 0.5, color, window);
                }
            }
            Self::Search => {
                frame.ring(0.14, 0.12, 0.54, 0.54, 0.09, color, window);
                frame.poly(
                    &[(0.58, 0.62), (0.88, 0.88), (0.81, 0.95), (0.52, 0.68)],
                    color,
                    window,
                );
            }
            Self::Stop => frame.rect(0.20, 0.20, 0.60, 0.60, 0.0, color, window),
            Self::Refresh => {
                frame.ring(0.18, 0.18, 0.64, 0.64, 0.10, color, window);
                frame.poly(&[(0.54, 0.08), (0.88, 0.22), (0.58, 0.38)], color, window);
            }
            Self::Add => {
                frame.rect(0.42, 0.14, 0.16, 0.72, 0.0, color, window);
                frame.rect(0.14, 0.42, 0.72, 0.16, 0.0, color, window);
            }
            Self::Remove => frame.rect(0.14, 0.42, 0.72, 0.16, 0.0, color, window),
            Self::Action => {
                for (x, y) in [
                    (0.16, 0.16),
                    (0.42, 0.16),
                    (0.68, 0.16),
                    (0.16, 0.68),
                    (0.42, 0.68),
                    (0.68, 0.68),
                ] {
                    frame.rect(x, y, 0.16, 0.16, 0.0, color, window);
                }
                frame.rect(0.42, 0.42, 0.16, 0.16, 0.0, color, window);
            }
            Self::Info => {
                frame.rect(0.43, 0.12, 0.14, 0.14, 0.0, color, window);
                frame.rect(0.43, 0.36, 0.14, 0.52, 0.0, color, window);
            }
            Self::Sidebar => {
                frame.rect(0.12, 0.16, 0.26, 0.68, 0.0, color, window);
                frame.rect(0.46, 0.16, 0.42, 0.68, 0.0, color, window);
            }
            Self::Inspector => {
                frame.rect(0.12, 0.16, 0.42, 0.68, 0.0, color, window);
                frame.rect(0.62, 0.16, 0.26, 0.68, 0.0, color, window);
            }
            Self::Eject => {
                frame.poly(&[(0.50, 0.20), (0.84, 0.62), (0.16, 0.62)], color, window);
                frame.rect(0.16, 0.72, 0.68, 0.09, 0.5, color, window);
            }
            Self::Overflow => {
                frame.poly(
                    &[
                        (0.20, 0.35),
                        (0.50, 0.65),
                        (0.80, 0.35),
                        (0.80, 0.50),
                        (0.50, 0.80),
                        (0.20, 0.50),
                    ],
                    color,
                    window,
                );
            }
        }
    }

    /// Render in the fixed slot for a toolbar or segmented control.
    pub fn element(self, slot: TemplateImageSlot, color: Rgba8) -> AnyElement {
        let size = slot.logical_size();
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| self.paint(window, bounds, color),
        )
        .flex_none()
        .size(px(size))
        .into_any_element()
    }

    /// Renderer callback for `Button::icon`, preserving the button's state ink.
    pub fn button_icon_renderer(
        self,
        slot: TemplateImageSlot,
    ) -> impl Fn(gpui::Rgba) -> AnyElement + 'static {
        move |ink| self.element(slot, rgba8(ink))
    }
}

struct Frame {
    b: Bounds<Pixels>,
    /// 0 is full colour, 1 is the inactive grey.
    dim: f32,
    /// Darkening for a pressed pictorial control.
    shade: f32,
}

// Original designed palette. No Apple artwork is sampled or traced.
const FOLDER_BACK: Rgba8 = Rgba8::hex(0x2f7fb8);
const FOLDER_BODY: Rgba8 = Rgba8::hex(0x63b7e8);
const FOLDER_LIGHT: Rgba8 = Rgba8::hex(0xa8dcf7);
const FOLDER_EDGE: Rgba8 = Rgba8::hex(0x1d5f91);
const PAPER: Rgba8 = Rgba8::hex(0xfdfdfd);
const PAPER_EDGE: Rgba8 = Rgba8::hex(0x9aa4ad);
const INK: Rgba8 = Rgba8::hex(0x59636d);
const INK_LIGHT: Rgba8 = Rgba8::hex(0xb9c2ca);
const APP_BLUE: Rgba8 = Rgba8::hex(0x2d6fb5);
const APP_LIGHT: Rgba8 = Rgba8::hex(0x5fa6e2);
const APP_DARK: Rgba8 = Rgba8::hex(0x1a4675);
const WINDOW_WHITE: Rgba8 = Rgba8::hex(0xf7fbff);
const ROOF: Rgba8 = Rgba8::hex(0x3d77b5);
const HOUSE: Rgba8 = Rgba8::hex(0xf2ead7);
const HOUSE_EDGE: Rgba8 = Rgba8::hex(0xbba98a);
const DOOR: Rgba8 = Rgba8::hex(0x8b5a2b);
const SCREEN: Rgba8 = Rgba8::hex(0x86bfe8);
const SCREEN_DARK: Rgba8 = Rgba8::hex(0x3d6f9a);
const METAL: Rgba8 = Rgba8::hex(0xd7dadd);
const METAL_DARK: Rgba8 = Rgba8::hex(0x8c939a);
const LED: Rgba8 = Rgba8::hex(0x4fc35a);
const GLOBE: Rgba8 = Rgba8::hex(0x2f8fc4);
const GLOBE_LIGHT: Rgba8 = Rgba8::hex(0x86d4f2);
const BADGE_DARK: Rgba8 = Rgba8::hex(0x1f5f9e);
const BADGE_LIGHT: Rgba8 = Rgba8::hex(0x64b6ee);
const SAVE_BLUE: Rgba8 = Rgba8::hex(0x3679b8);
const SAVE_LIGHT: Rgba8 = Rgba8::hex(0x8cc6ee);
const SLATE: Rgba8 = Rgba8::hex(0x7c878f);
const SLATE_LIGHT: Rgba8 = Rgba8::hex(0xc6cdd2);
const SLATE_DARK: Rgba8 = Rgba8::hex(0x4a545c);

fn mix(color: Rgba8, grey: u8, amount: f32) -> Rgba8 {
    let blend = |channel: u8| {
        (f32::from(channel) * (1.0 - amount) + f32::from(grey) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Rgba8 {
        r: blend(color.r),
        g: blend(color.g),
        b: blend(color.b),
        a: color.a,
    }
}

impl Frame {
    fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        Point::new(
            self.b.origin.x + self.b.size.width * x,
            self.b.origin.y + self.b.size.height * y,
        )
    }

    fn colour(&self, color: Rgba8) -> Rgba8 {
        let dimmed = if self.dim <= 0.0 {
            color
        } else {
            mix(color, 0xb8, self.dim)
        };
        if self.shade > 0.0 {
            mix(dimmed, 0x00, self.shade)
        } else {
            dimmed
        }
    }

    fn poly(&self, points: &[(f32, f32)], color: Rgba8, window: &mut Window) {
        let points: Vec<Point<Pixels>> = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        let mut path = PathBuilder::fill();
        path.add_polygon(&points, true);
        if let Ok(path) = path.build() {
            window.paint_path(path, hsla(self.colour(color)));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Rgba8, window: &mut Window) {
        let bounds = Bounds::from_corners(self.at(x, y), self.at(x + w, y + h));
        let radius = px(radius * self.b.size.width.as_f32() / 16.0);
        window.paint_quad(gpui::fill(bounds, hsla(self.colour(color))).corner_radii(radius));
    }

    #[allow(clippy::too_many_arguments)]
    fn ring(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        thickness: f32,
        color: Rgba8,
        window: &mut Window,
    ) {
        self.rect(x, y, w, thickness, 0.5, color, window);
        self.rect(x, y + h - thickness, w, thickness, 0.5, color, window);
        self.rect(x, y, thickness, h, 0.5, color, window);
        self.rect(x + w - thickness, y, thickness, h, 0.5, color, window);
    }

    fn chevron(&self, color: Rgba8, forward: bool, window: &mut Window) {
        let points = if forward {
            [
                (0.38, 0.12),
                (0.34, 0.16),
                (0.68, 0.50),
                (0.34, 0.84),
                (0.38, 0.88),
                (0.76, 0.50),
            ]
        } else {
            [
                (0.62, 0.12),
                (0.66, 0.16),
                (0.32, 0.50),
                (0.66, 0.84),
                (0.62, 0.88),
                (0.24, 0.50),
            ]
        };
        self.poly(&points, color, window);
    }

    fn folder(&self, window: &mut Window) {
        self.poly(
            &[(0.08, 0.20), (0.44, 0.20), (0.50, 0.28), (0.08, 0.28)],
            FOLDER_BACK,
            window,
        );
        self.rect(0.08, 0.24, 0.84, 0.18, 1.2, FOLDER_BACK, window);
        self.rect(0.08, 0.31, 0.84, 0.52, 1.6, FOLDER_BODY, window);
        self.rect(0.10, 0.31, 0.80, 0.10, 1.2, FOLDER_LIGHT, window);
        self.rect(0.08, 0.79, 0.84, 0.04, 0.8, FOLDER_EDGE, window);
    }

    fn documents(&self, window: &mut Window) {
        self.rect(0.11, 0.09, 0.58, 0.73, 1.0, PAPER_EDGE, window);
        self.rect(0.13, 0.11, 0.54, 0.68, 0.8, PAPER, window);
        self.poly(
            &[(0.68, 0.10), (0.68, 0.24), (0.54, 0.24)],
            INK_LIGHT,
            window,
        );
        for row in 0..3 {
            let y = 0.30 + row as f32 * 0.10;
            self.rect(0.21, y, 0.40, 0.035, 0.0, INK_LIGHT, window);
        }
        self.rect(0.22, 0.36, 0.52, 0.50, 1.4, FOLDER_BODY, window);
        self.rect(0.24, 0.36, 0.48, 0.08, 1.0, FOLDER_LIGHT, window);
    }

    fn document(&self, window: &mut Window) {
        self.poly(
            &[
                (0.23, 0.06),
                (0.62, 0.06),
                (0.80, 0.25),
                (0.80, 0.94),
                (0.23, 0.94),
            ],
            PAPER_EDGE,
            window,
        );
        self.poly(
            &[
                (0.25, 0.08),
                (0.61, 0.08),
                (0.78, 0.26),
                (0.78, 0.92),
                (0.25, 0.92),
            ],
            PAPER,
            window,
        );
        self.poly(
            &[(0.61, 0.08), (0.61, 0.26), (0.78, 0.26)],
            PAPER_EDGE,
            window,
        );
        self.rect(0.24, 0.925, 0.55, 0.02, 0.0, PAPER_EDGE, window);
        for row in 0..5 {
            let y = 0.32 + row as f32 * 0.10;
            self.rect(
                0.31,
                y,
                0.40,
                0.035,
                0.0,
                if row == 0 { INK } else { INK_LIGHT },
                window,
            );
        }
        self.rect(0.31, 0.26, 0.24, 0.035, 0.0, INK, window);
    }

    fn application(&self, window: &mut Window) {
        self.rect(0.09, 0.11, 0.80, 0.76, 2.2, APP_DARK, window);
        self.rect(0.10, 0.11, 0.78, 0.70, 2.0, APP_BLUE, window);
        self.rect(0.12, 0.13, 0.74, 0.34, 1.6, APP_LIGHT, window);
        self.rect(0.21, 0.31, 0.58, 0.44, 1.2, WINDOW_WHITE, window);
        self.rect(0.21, 0.31, 0.58, 0.09, 1.0, PAPER_EDGE, window);
        for dot in 0..3 {
            self.rect(
                0.25 + dot as f32 * 0.06,
                0.335,
                0.035,
                0.035,
                0.5,
                INK,
                window,
            );
        }
        self.rect(0.26, 0.47, 0.34, 0.035, 0.0, INK_LIGHT, window);
        self.rect(0.26, 0.55, 0.46, 0.035, 0.0, INK_LIGHT, window);
        self.rect(0.26, 0.63, 0.28, 0.035, 0.0, INK_LIGHT, window);
    }

    fn home(&self, window: &mut Window) {
        self.poly(
            &[
                (0.50, 0.08),
                (0.95, 0.45),
                (0.86, 0.50),
                (0.81, 0.88),
                (0.19, 0.88),
                (0.14, 0.50),
                (0.05, 0.45),
            ],
            ROOF,
            window,
        );
        self.rect(0.22, 0.50, 0.56, 0.38, 0.6, HOUSE, window);
        self.rect(0.44, 0.60, 0.16, 0.28, 0.4, DOOR, window);
        self.rect(0.28, 0.56, 0.09, 0.09, 0.2, SCREEN, window);
        self.rect(0.63, 0.56, 0.09, 0.09, 0.2, SCREEN, window);
        self.rect(0.22, 0.86, 0.56, 0.02, 0.0, HOUSE_EDGE, window);
    }

    fn desktop(&self, window: &mut Window) {
        self.rect(0.06, 0.13, 0.88, 0.58, 1.2, METAL_DARK, window);
        self.rect(0.09, 0.16, 0.82, 0.48, 0.8, SCREEN_DARK, window);
        self.rect(0.11, 0.18, 0.78, 0.40, 0.6, SCREEN, window);
        self.rect(0.11, 0.18, 0.78, 0.12, 0.6, GLOBE_LIGHT, window);
        self.rect(0.32, 0.73, 0.36, 0.05, 0.2, METAL, window);
        self.rect(0.22, 0.80, 0.56, 0.05, 0.5, METAL, window);
        self.rect(0.22, 0.80, 0.56, 0.02, 0.5, METAL_DARK, window);
    }

    fn volume(&self, window: &mut Window) {
        self.rect(0.06, 0.22, 0.88, 0.54, 2.0, METAL_DARK, window);
        self.rect(0.07, 0.22, 0.86, 0.48, 2.0, METAL, window);
        self.rect(0.09, 0.24, 0.82, 0.18, 1.6, PAPER, window);
        self.rect(0.12, 0.52, 0.76, 0.16, 0.8, METAL_DARK, window);
        self.rect(0.74, 0.55, 0.09, 0.09, 0.5, LED, window);
        self.rect(0.14, 0.34, 0.20, 0.035, 0.0, INK_LIGHT, window);
        self.rect(0.14, 0.40, 0.34, 0.025, 0.0, INK_LIGHT, window);
    }

    fn network(&self, window: &mut Window) {
        self.rect(0.12, 0.12, 0.76, 0.76, 3.0, GLOBE, window);
        self.rect(0.14, 0.14, 0.72, 0.30, 2.4, GLOBE_LIGHT, window);
        self.rect(0.26, 0.14, 0.035, 0.72, 0.0, PAPER, window);
        self.rect(0.70, 0.14, 0.035, 0.72, 0.0, PAPER, window);
        self.rect(0.12, 0.44, 0.76, 0.035, 0.0, PAPER, window);
        self.rect(0.20, 0.28, 0.60, 0.025, 0.0, PAPER, window);
        self.rect(0.20, 0.66, 0.60, 0.025, 0.0, PAPER, window);
        self.poly(&[(0.50, 0.88), (0.70, 0.70), (0.70, 0.88)], METAL, window);
    }

    fn smart_search(&self, window: &mut Window) {
        self.poly(
            &[(0.06, 0.22), (0.40, 0.22), (0.47, 0.30), (0.06, 0.30)],
            FOLDER_BACK,
            window,
        );
        self.rect(0.06, 0.28, 0.76, 0.52, 1.5, FOLDER_BODY, window);
        self.rect(0.08, 0.28, 0.72, 0.09, 1.0, FOLDER_LIGHT, window);
        self.ring(0.46, 0.42, 0.34, 0.34, 0.07, INK, window);
        self.poly(
            &[(0.74, 0.70), (0.94, 0.90), (0.89, 0.95), (0.69, 0.75)],
            INK,
            window,
        );
    }

    /// Pictorial "new document": a sheet with a folded corner and a blue badge.
    fn pictorial_new(&self, window: &mut Window) {
        self.poly(
            &[
                (0.18, 0.05),
                (0.60, 0.05),
                (0.80, 0.25),
                (0.80, 0.90),
                (0.18, 0.90),
            ],
            PAPER_EDGE,
            window,
        );
        self.poly(
            &[
                (0.21, 0.08),
                (0.59, 0.08),
                (0.77, 0.27),
                (0.77, 0.87),
                (0.21, 0.87),
            ],
            PAPER,
            window,
        );
        self.poly(
            &[(0.59, 0.08), (0.59, 0.27), (0.77, 0.27)],
            PAPER_EDGE,
            window,
        );
        for row in 0..3 {
            self.rect(
                0.27,
                0.36 + row as f32 * 0.11,
                0.36,
                0.035,
                0.0,
                INK_LIGHT,
                window,
            );
        }
        self.rect(0.54, 0.55, 0.36, 0.36, 3.6, BADGE_DARK, window);
        self.rect(0.56, 0.55, 0.32, 0.24, 3.2, BADGE_LIGHT, window);
        self.rect(0.67, 0.59, 0.10, 0.28, 0.4, WINDOW_WHITE, window);
        self.rect(0.58, 0.68, 0.28, 0.10, 0.4, WINDOW_WHITE, window);
    }

    /// Pictorial "open": a front-facing open folder.
    fn pictorial_open(&self, window: &mut Window) {
        self.poly(
            &[
                (0.06, 0.18),
                (0.44, 0.18),
                (0.52, 0.28),
                (0.94, 0.28),
                (0.94, 0.44),
                (0.06, 0.44),
            ],
            FOLDER_BACK,
            window,
        );
        self.rect(0.08, 0.26, 0.84, 0.20, 1.4, FOLDER_BODY, window);
        self.rect(0.10, 0.28, 0.80, 0.07, 1.0, FOLDER_LIGHT, window);
        self.poly(
            &[(0.04, 0.42), (0.96, 0.42), (0.86, 0.84), (0.14, 0.84)],
            FOLDER_BODY,
            window,
        );
        self.poly(
            &[(0.10, 0.46), (0.90, 0.46), (0.82, 0.62), (0.18, 0.62)],
            FOLDER_LIGHT,
            window,
        );
        self.rect(0.12, 0.82, 0.72, 0.04, 0.8, FOLDER_EDGE, window);
    }

    /// Pictorial "save": a blue floppy disk with a white label.
    fn pictorial_save(&self, window: &mut Window) {
        self.rect(0.10, 0.08, 0.80, 0.84, 1.6, FOLDER_EDGE, window);
        self.rect(0.13, 0.08, 0.74, 0.78, 1.2, SAVE_BLUE, window);
        self.rect(0.14, 0.09, 0.72, 0.30, 1.0, SAVE_LIGHT, window);
        self.rect(0.30, 0.08, 0.40, 0.26, 0.4, METAL, window);
        self.rect(0.52, 0.10, 0.14, 0.22, 0.3, METAL_DARK, window);
        self.rect(0.18, 0.48, 0.64, 0.40, 0.6, PAPER, window);
        for row in 0..3 {
            self.rect(
                0.24,
                0.56 + row as f32 * 0.08,
                0.52,
                0.025,
                0.0,
                INK_LIGHT,
                window,
            );
        }
        self.rect(0.13, 0.86, 0.74, 0.03, 0.4, FOLDER_EDGE, window);
    }

    /// Pictorial "preferences": a single slate gear with a top-left highlight.
    fn pictorial_preferences(&self, window: &mut Window) {
        const TEETH: usize = 8;
        for tooth in 0..TEETH {
            let angle = tooth as f32 * std::f32::consts::TAU / TEETH as f32;
            let near = 0.30;
            let far = 0.47;
            let half = 0.13;
            let point = |radius: f32, offset: f32| {
                let a = angle + offset;
                (0.50 + radius * a.cos(), 0.50 + radius * a.sin())
            };
            let points = [
                point(near, -half),
                point(far, -half * 0.6),
                point(far, half * 0.6),
                point(near, half),
            ];
            self.poly(&points, SLATE_DARK, window);
        }
        self.rect(0.14, 0.14, 0.72, 0.72, 9.0, SLATE_DARK, window);
        self.rect(0.16, 0.15, 0.68, 0.60, 8.0, SLATE, window);
        self.rect(0.18, 0.16, 0.64, 0.24, 7.0, SLATE_LIGHT, window);
        self.rect(0.34, 0.34, 0.32, 0.32, 4.0, SLATE_DARK, window);
        self.rect(0.37, 0.37, 0.26, 0.22, 3.2, PAPER, window);
    }
}

/// A row of template images, useful in documentation and the gallery.
pub fn template_image_row(
    images: &[TemplateImage],
    color: Rgba8,
    slot: TemplateImageSlot,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(images.iter().map(|image| image.element(slot, color)))
        .into_any_element()
}

/// A row of full-colour file icons.
pub fn file_icon_row(artwork: &[FileArtwork], size: f32) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(
            artwork
                .iter()
                .map(|icon| icon.file_element(size, FileIconState::normal())),
        )
        .into_any_element()
}

/// Convenience for resolving typed template-image roles in
/// `SegmentedControl`.
pub fn role_renderer(slot: TemplateImageSlot) -> impl Fn(&str, Rgba8) -> AnyElement + 'static {
    move |role, color| match TemplateImage::from_role(role) {
        Some(image) => image.element(slot, color),
        None => div().size(px(slot.logical_size())).into_any_element(),
    }
}

/// The source-list renderer: file icons keep their designed colours; disabled
/// rows and inactive windows desaturate through `FileIconState`.
pub fn source_renderer(
    size: f32,
) -> impl Fn(&str, bool, bool, &mut Window, &mut App) -> AnyElement + 'static {
    move |role, selected, disabled, window, _| match Icon::from_role(role) {
        Some(icon) => icon.file_element(
            size,
            FileIconState {
                selected,
                inactive: !window.is_window_active(),
                disabled,
            },
        ),
        None => div().size(px(size)).into_any_element(),
    }
}
