//! Semantic icon vocabulary for the file panels (#41, #48 subset).
//!
//! Two families live here:
//!
//! - Full-colour, designed 16 px file icons for the sidebar, list and grid:
//!   folder, document, application, home, desktop, documents, volume, network
//!   volume, smart search. The artwork is original and drawn from vector
//!   shapes; no Apple artwork is traced or recoloured. `docs/exceptions.md`
//!   records the set as designed.
//! - Monochrome toolbar glyphs (back, forward, icon/list/column view, search,
//!   eject) painted in the caller's token colour so segmented controls and
//!   buttons keep their state colours.
//!
//! Selected file icons keep their colours, as in 10.6; inactive windows and
//! disabled rows desaturate through [`FileIconState`]. Toolbar glyphs still
//! take their colour from the theme, never per-control SVG colours.

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, Bounds, PathBuilder, Pixels, Point, Window,
};
use silvestris_core::Rgba8;

use crate::convert::hsla;

/// The icons the Open and Save panels need.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Folder,
    Home,
    Desktop,
    Documents,
    Volume,
    Network,
    SmartSearch,
    Document,
    Application,
    Back,
    Forward,
    IconView,
    List,
    Columns,
    Eject,
    Search,
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

impl Icon {
    /// Paint a monochrome toolbar glyph into `bounds`.
    pub fn paint(self, window: &mut Window, bounds: Bounds<Pixels>, color: Rgba8) {
        let f = Frame {
            b: bounds,
            dim: 0.0,
        };
        match self {
            Icon::Back => f.chevron(color, false, window),
            Icon::Forward => f.chevron(color, true, window),
            Icon::IconView => {
                for (x, y) in [(0.10, 0.14), (0.54, 0.14), (0.10, 0.56), (0.54, 0.56)] {
                    f.rect(x, y, 0.30, 0.30, 0.8, color, window);
                }
            }
            Icon::List => {
                for row in 0..3 {
                    let y = 0.22 + row as f32 * 0.22;
                    f.rect(0.10, y + 0.01, 0.09, 0.07, 0.5, color, window);
                    f.rect(0.27, y + 0.02, 0.63, 0.05, 0.0, color, window);
                }
            }
            Icon::Columns => {
                f.rect(0.08, 0.16, 0.24, 0.68, 0.5, color, window);
                f.rect(0.38, 0.16, 0.24, 0.68, 0.5, color, window);
                f.rect(0.68, 0.16, 0.24, 0.68, 0.5, color, window);
            }
            Icon::Eject => {
                f.poly(&[(0.50, 0.20), (0.84, 0.62), (0.16, 0.62)], color, window);
                f.rect(0.16, 0.72, 0.68, 0.09, 0.5, color, window);
            }
            Icon::Search => {
                f.ring(0.14, 0.12, 0.54, 0.54, 0.09, color, window);
                f.poly(
                    &[(0.58, 0.62), (0.88, 0.88), (0.81, 0.95), (0.52, 0.68)],
                    color,
                    window,
                );
            }
            // Full-colour icons can still be asked for a monochrome glyph
            // (documentation rows, toolbar previews); use the designed
            // silhouette in the supplied colour.
            _ => self.paint_flat(window, bounds, color),
        }
    }

    fn paint_flat(self, window: &mut Window, bounds: Bounds<Pixels>, color: Rgba8) {
        let f = Frame {
            b: bounds,
            dim: 0.0,
        };
        match self {
            Icon::Folder => {
                f.poly(
                    &[(0.08, 0.24), (0.42, 0.24), (0.49, 0.32), (0.08, 0.32)],
                    color,
                    window,
                );
                f.rect(0.08, 0.30, 0.84, 0.53, 1.5, color, window);
            }
            Icon::Home => {
                f.poly(
                    &[
                        (0.50, 0.10),
                        (0.94, 0.46),
                        (0.84, 0.46),
                        (0.78, 0.86),
                        (0.22, 0.86),
                        (0.16, 0.46),
                        (0.06, 0.46),
                    ],
                    color,
                    window,
                );
            }
            Icon::Desktop => {
                f.rect(0.08, 0.16, 0.84, 0.54, 1.2, color, window);
                f.rect(0.34, 0.74, 0.32, 0.07, 0.0, color, window);
                f.rect(0.22, 0.83, 0.56, 0.05, 0.0, color, window);
            }
            Icon::Documents => {
                f.rect(0.20, 0.14, 0.58, 0.70, 1.0, color, window);
                f.rect(0.30, 0.24, 0.58, 0.70, 1.0, color, window);
            }
            Icon::Volume | Icon::Network => {
                f.rect(0.08, 0.24, 0.84, 0.52, 2.0, color, window);
            }
            Icon::SmartSearch => {
                f.poly(
                    &[(0.08, 0.24), (0.42, 0.24), (0.49, 0.32), (0.08, 0.32)],
                    color,
                    window,
                );
                f.rect(0.08, 0.30, 0.84, 0.53, 1.5, color, window);
            }
            Icon::Document => {
                f.poly(
                    &[
                        (0.24, 0.08),
                        (0.63, 0.08),
                        (0.78, 0.26),
                        (0.78, 0.92),
                        (0.24, 0.92),
                    ],
                    color,
                    window,
                );
            }
            Icon::Application => {
                f.rect(0.10, 0.14, 0.80, 0.72, 1.8, color, window);
            }
            _ => {}
        }
    }

    /// Paint a full-colour file icon into `bounds`.
    pub fn file_paint(self, window: &mut Window, bounds: Bounds<Pixels>, state: FileIconState) {
        let f = Frame {
            b: bounds,
            dim: state.dim(),
        };
        match self {
            Icon::Folder => f.folder(window),
            Icon::Home => f.home(window),
            Icon::Desktop => f.desktop(window),
            Icon::Documents => f.documents(window),
            Icon::Volume => f.volume(window),
            Icon::Network => f.network(window),
            Icon::SmartSearch => f.smart_search(window),
            Icon::Document => f.document(window),
            Icon::Application => f.application(window),
            _ => self.paint(window, bounds, Rgba8::hex(0x4c4c4c)),
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

    /// A sized monochrome glyph.
    pub fn element(self, size: f32, color: Rgba8) -> AnyElement {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| self.paint(window, bounds, color),
        )
        .flex_none()
        .size(px(size))
        .into_any_element()
    }

    /// Element for a segmented-control or button icon renderer.
    pub fn sized_for(self, size: f32) -> impl Fn(Rgba8) -> AnyElement + 'static {
        move |color| self.element(size, color)
    }

    /// The role string shared with `SourceNode::icon` and
    /// `SegmentedControl::icon`.
    #[must_use]
    pub fn role(self) -> &'static str {
        match self {
            Icon::Folder => "folder",
            Icon::Home => "home",
            Icon::Desktop => "desktop",
            Icon::Documents => "documents",
            Icon::Volume => "volume",
            Icon::Network => "network",
            Icon::SmartSearch => "smart-search",
            Icon::Document => "document",
            Icon::Application => "application",
            Icon::Back => "back",
            Icon::Forward => "forward",
            Icon::IconView => "icon",
            Icon::List => "list",
            Icon::Columns => "columns",
            Icon::Eject => "eject",
            Icon::Search => "search",
        }
    }

    /// Parse the role strings `SegmentedControl::icon` and `SourceNode::icon`
    /// use.
    pub fn from_role(role: &str) -> Option<Self> {
        Some(match role {
            "folder" => Icon::Folder,
            "home" => Icon::Home,
            "desktop" => Icon::Desktop,
            "documents" => Icon::Documents,
            "volume" => Icon::Volume,
            "network" | "network-volume" => Icon::Network,
            "smart-search" => Icon::SmartSearch,
            "document" => Icon::Document,
            "application" => Icon::Application,
            "back" => Icon::Back,
            "forward" => Icon::Forward,
            "icon" | "icon-view" => Icon::IconView,
            "list" => Icon::List,
            "columns" | "column-browser" => Icon::Columns,
            "eject" => Icon::Eject,
            "search" => Icon::Search,
            _ => return None,
        })
    }
}

struct Frame {
    b: Bounds<Pixels>,
    /// 0 is full colour, 1 is the inactive grey.
    dim: f32,
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
        if self.dim <= 0.0 {
            color
        } else {
            mix(color, 0xb8, self.dim)
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
}

/// A row of semantic icons, useful in documentation and the gallery.
pub fn icon_row(icons: &[Icon], color: Rgba8, size: f32) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(icons.iter().map(|icon| icon.element(size, color)))
        .into_any_element()
}

/// A row of full-colour file icons.
pub fn file_icon_row(icons: &[Icon], size: f32) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .children(
            icons
                .iter()
                .map(|icon| icon.file_element(size, FileIconState::normal())),
        )
        .into_any_element()
}

/// Convenience for a role-based monochrome icon renderer used by
/// `SegmentedControl`.
pub fn role_renderer(size: f32) -> impl Fn(&str, Rgba8) -> AnyElement + 'static {
    move |role, color| match Icon::from_role(role) {
        Some(icon) => icon.element(size, color),
        None => div().size(px(size)).into_any_element(),
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
