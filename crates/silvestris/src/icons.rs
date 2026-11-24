//! Semantic icon vocabulary for the file panels (#41, #48 subset).
//!
//! Every icon is a vector path painted in one colour supplied by the caller.
//! Controls pick that colour from a token (`colors.text`,
//! `lists.source_active.text`, or a segmented control's state), so selected
//! and inactive states follow the theme rather than per-icon SVG colours.
//! Artwork is original: simple silhouettes sized for the 16 px sidebar row and
//! the 12 px mini toolbar control.

use gpui::{
    canvas, div, prelude::*, px, AnyElement, App, Bounds, PathBuilder, Pixels, Point, Window,
};
use silvestris_core::Rgba8;

use crate::convert::hsla;
use crate::ActiveTheme;

/// The icons the Open and Save panels need.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Folder,
    Home,
    Desktop,
    Documents,
    Volume,
    Document,
    Application,
    Back,
    Forward,
    List,
    Columns,
    Eject,
    Search,
}

impl Icon {
    /// Paint the icon into `bounds`, in one colour.
    pub fn paint(self, window: &mut Window, bounds: Bounds<Pixels>, color: Rgba8) {
        let f = Frame { b: bounds };
        match self {
            Icon::Folder => {
                f.poly(
                    &[(0.08, 0.25), (0.41, 0.25), (0.48, 0.34), (0.08, 0.34)],
                    color,
                    window,
                );
                f.rect(0.08, 0.32, 0.84, 0.50, 1.0, color, window);
            }
            Icon::Home => {
                f.poly(
                    &[
                        (0.50, 0.13),
                        (0.94, 0.48),
                        (0.84, 0.48),
                        (0.78, 0.83),
                        (0.22, 0.83),
                        (0.16, 0.48),
                        (0.06, 0.48),
                    ],
                    color,
                    window,
                );
                f.rect(0.43, 0.57, 0.14, 0.26, 0.0, color, window);
            }
            Icon::Desktop => {
                f.rect(0.10, 0.18, 0.80, 0.52, 1.5, color, window);
                f.rect(0.35, 0.72, 0.30, 0.08, 0.0, color, window);
                f.rect(0.24, 0.82, 0.52, 0.05, 0.0, color, window);
            }
            Icon::Documents => {
                f.rect(0.22, 0.14, 0.54, 0.68, 1.0, color, window);
                f.rect(0.32, 0.22, 0.54, 0.68, 1.0, color, window);
            }
            Icon::Volume => {
                f.rect(0.08, 0.26, 0.84, 0.48, 2.0, color, window);
                f.rect(0.16, 0.36, 0.16, 0.05, 0.0, color, window);
                f.rect(0.72, 0.36, 0.12, 0.05, 0.0, color, window);
            }
            Icon::Document => {
                f.poly(
                    &[
                        (0.24, 0.10),
                        (0.63, 0.10),
                        (0.78, 0.27),
                        (0.78, 0.90),
                        (0.24, 0.90),
                    ],
                    color,
                    window,
                );
                f.poly(
                    &[(0.63, 0.10), (0.63, 0.27), (0.78, 0.27)],
                    Rgba8 {
                        a: color.a / 2,
                        ..color
                    },
                    window,
                );
            }
            Icon::Application => {
                f.rect(0.12, 0.16, 0.76, 0.68, 1.5, color, window);
                f.rect(0.12, 0.16, 0.76, 0.12, 1.5, color, window);
                f.rect(0.22, 0.42, 0.56, 0.05, 0.0, color, window);
                f.rect(0.22, 0.56, 0.40, 0.05, 0.0, color, window);
            }
            Icon::Back => {
                f.poly(
                    &[
                        (0.62, 0.12),
                        (0.66, 0.16),
                        (0.32, 0.50),
                        (0.66, 0.84),
                        (0.62, 0.88),
                        (0.24, 0.50),
                    ],
                    color,
                    window,
                );
            }
            Icon::Forward => {
                f.poly(
                    &[
                        (0.38, 0.12),
                        (0.34, 0.16),
                        (0.68, 0.50),
                        (0.34, 0.84),
                        (0.38, 0.88),
                        (0.76, 0.50),
                    ],
                    color,
                    window,
                );
            }
            Icon::List => {
                for row in 0..3 {
                    let y = 0.26 + row as f32 * 0.22;
                    f.rect(0.10, y, 0.10, 0.08, 0.5, color, window);
                    f.rect(0.28, y + 0.015, 0.62, 0.05, 0.0, color, window);
                }
            }
            Icon::Columns => {
                f.rect(0.08, 0.18, 0.24, 0.64, 0.5, color, window);
                f.rect(0.38, 0.18, 0.24, 0.64, 0.5, color, window);
                f.rect(0.68, 0.18, 0.24, 0.64, 0.5, color, window);
            }
            Icon::Eject => {
                f.poly(&[(0.50, 0.20), (0.84, 0.62), (0.16, 0.62)], color, window);
                f.rect(0.16, 0.72, 0.68, 0.10, 0.5, color, window);
            }
            Icon::Search => {
                f.ring(0.14, 0.12, 0.56, 0.56, 0.09, color, window);
                f.poly(
                    &[(0.60, 0.62), (0.88, 0.88), (0.80, 0.94), (0.54, 0.68)],
                    color,
                    window,
                );
            }
        }
    }

    /// A sized element for composition in a row. The colour comes from the
    /// caller's token.
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
            Icon::Document => "document",
            Icon::Application => "application",
            Icon::Back => "back",
            Icon::Forward => "forward",
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
            "document" => Icon::Document,
            "application" => Icon::Application,
            "back" => Icon::Back,
            "forward" => Icon::Forward,
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
}

impl Frame {
    fn at(&self, x: f32, y: f32) -> Point<Pixels> {
        Point::new(
            self.b.origin.x + self.b.size.width * x,
            self.b.origin.y + self.b.size.height * y,
        )
    }

    fn poly(&self, points: &[(f32, f32)], color: Rgba8, window: &mut Window) {
        let points: Vec<Point<Pixels>> = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        let mut path = PathBuilder::fill();
        path.add_polygon(&points, true);
        if let Ok(path) = path.build() {
            window.paint_path(path, hsla(color));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn rect(&self, x: f32, y: f32, w: f32, h: f32, radius: f32, color: Rgba8, window: &mut Window) {
        let bounds = Bounds::from_corners(self.at(x, y), self.at(x + w, y + h));
        let radius = px(radius * self.b.size.width.as_f32() / 16.0);
        window.paint_quad(gpui::fill(bounds, hsla(color)).corner_radii(radius));
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

/// Convenience for a role-based icon renderer used by `SourceList` and
/// `SegmentedControl`.
pub fn role_renderer(size: f32) -> impl Fn(&str, Rgba8) -> AnyElement + 'static {
    move |role, color| match Icon::from_role(role) {
        Some(icon) => icon.element(size, color),
        None => div().size(px(size)).into_any_element(),
    }
}

/// The same renderer with the source-list booleans: selected and disabled
/// rows take the theme's selected text or disabled ink.
pub fn source_renderer(
    size: f32,
) -> impl Fn(&str, bool, bool, &mut Window, &mut App) -> AnyElement + 'static {
    move |role, selected, disabled, _, cx| match Icon::from_role(role) {
        Some(icon) => {
            let theme = cx.theme();
            let color = if disabled {
                theme.colors.text_disabled
            } else if selected {
                theme.lists.source_active.text
            } else {
                theme.colors.text
            };
            icon.element(size, color)
        }
        None => div().size(px(size)).into_any_element(),
    }
}
