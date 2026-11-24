//! Material painter: the handbook 10 paint stack expressed as GPUI quads.
//!
//! Painters take resolved tokens and bounds, never theme names, so Aqua and
//! Geist share one code path and differ only in data (#121).

use gpui::{
    point, px, quad, size, transparent_black, BorderStyle, Bounds, ContentMask, Corners, Pixels,
    Point, Window,
};
use silvestris_core::tokens::{Bevel, Chrome, Field, GripStyle, Selection};
use silvestris_core::Rgba8;

use crate::convert::{band, hairline, hsla, rows};

pub fn inset(b: Bounds<Pixels>, d: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(b.origin.x + d, b.origin.y + d),
        size(
            (b.size.width - d * 2.0).max(px(0.)),
            (b.size.height - d * 2.0).max(px(0.)),
        ),
    )
}

pub fn outset(b: Bounds<Pixels>, d: Pixels) -> Bounds<Pixels> {
    inset(b, -d)
}

/// Round a logical coordinate to the nearest device-pixel **edge**. Use it for
/// the outside of a quad: at 1.25x, 10.1 logical becomes 10.4 and 10.3 becomes
/// 10.4, so a one-device border starts on a device row instead of straddling
/// two.
#[must_use]
pub fn snap_edge(window: &Window, v: Pixels) -> Pixels {
    let s = window.scale_factor();
    px((v.as_f32() * s).round() / s)
}

/// Round a logical coordinate to the nearest device-pixel **centre**. Use it
/// for a stroke centred on a path: a one-device stroke at logical 10.0 spans
/// two device pixels at 1.25x, while its centre at 10.4 (half a device px from
/// the edge at 10.0) covers exactly one.
#[must_use]
pub fn snap_stroke_center(window: &Window, v: Pixels) -> Pixels {
    let s = window.scale_factor();
    px(((v.as_f32() * s - 0.5).round() + 0.5) / s)
}

/// Snap a quad to the device grid, keeping the origin and the far edge on
/// device edges. This is the rule every painter applies before it paints.
#[must_use]
pub fn snap_bounds(window: &Window, b: Bounds<Pixels>) -> Bounds<Pixels> {
    let x0 = snap_edge(window, b.origin.x);
    let y0 = snap_edge(window, b.origin.y);
    let x1 = snap_edge(window, b.origin.x + b.size.width);
    let y1 = snap_edge(window, b.origin.y + b.size.height);
    Bounds::new(
        point(x0, y0),
        size((x1 - x0).max(px(0.)), (y1 - y0).max(px(0.))),
    )
}

/// Backwards-compatible private name used inside this module.
fn snap(window: &Window, v: Pixels) -> Pixels {
    snap_edge(window, v)
}

fn fill_rounded(window: &mut Window, b: Bounds<Pixels>, r: Pixels, bg: gpui::Background) {
    window.paint_quad(quad(
        b,
        r,
        bg,
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    ));
}

/// Paint a bezel: emboss line under it, border ring (top→bottom gradient),
/// then the lower gloss band and the upper band clipped above the split
/// (M: button.push.normal and siblings; handbook 10 "layered gloss").
///
/// Both bands are painted as the full inner rounded rectangle so capsule
/// ends stay round; the upper band is clipped with a content mask, and the
/// lower band holds `lower.from` above its first stop.
pub fn bevel(window: &mut Window, bounds: Bounds<Pixels>, radius: f32, b: &Bevel) {
    let bounds = snap_bounds(window, bounds);
    let h = hairline(window);
    let r = px(radius.min(bounds.size.height.as_f32() / 2.0));
    if b.emboss.a > 0 {
        let mut e = bounds;
        e.origin.y += h;
        fill_rounded(window, e, r, gpui::solid_background(hsla(b.emboss)));
    }
    let scale = window.scale_factor();
    fill_rounded(
        window,
        bounds,
        r,
        rows(b.border, bounds.size.height.as_f32() * scale),
    );
    let inner = inset(bounds, h);
    let ir = (r - h).max(px(0.));
    let ih = inner.size.height.as_f32();
    if ih <= 0.0 {
        return;
    }
    let split_y = snap(window, inner.origin.y + px(ih * b.split));
    let split = ((split_y - inner.origin.y).as_f32() / ih).clamp(0.0, 1.0);
    // Half a device row, as a fraction of the inner height: stops sit on
    // row centres so the first and last rows carry the measured colours.
    let half = 0.5 / (ih * scale);
    fill_rounded(window, inner, ir, band(b.lower, split + half, 1.0 - half));
    if split > 0.0 {
        let mask = ContentMask {
            bounds: Bounds::new(
                inner.origin,
                size(inner.size.width, split_y - inner.origin.y),
            ),
        };
        window.with_content_mask(Some(mask), |window| {
            fill_rounded(
                window,
                inner,
                ir,
                band(b.upper, half, (split - half).max(half)),
            );
        });
    }
}

/// Vertical scroller thumb: a capsule whose bevel runs across the bar, the
/// `upper` band on the leading side of `split` and `lower` after it
/// (M: scroller.thumb.row: edge 0751c0, leading 88b4e5..98c3ef, trailing
/// 5da5e9..a4e0ff).
pub fn thumb(window: &mut Window, bounds: Bounds<Pixels>, b: &Bevel) {
    let bounds = snap_bounds(window, bounds);
    use gpui::{linear_color_stop, linear_gradient};
    let h = hairline(window);
    let r = bounds.size.width / 2.0;
    fill_rounded(
        window,
        bounds,
        r,
        crate::convert::across(b.border, bounds.size.width.as_f32()),
    );
    let inner = inset(bounds, h);
    let iw = inner.size.width.as_f32();
    if iw <= 0.0 || inner.size.height <= px(0.) {
        return;
    }
    let ir = (r - h).max(px(0.));
    let split_x = snap(window, inner.origin.x + px(iw * b.split));
    let split = ((split_x - inner.origin.x).as_f32() / iw).clamp(0.0, 1.0);
    let half = 0.5 / (iw * window.scale_factor());
    let stops = |g: silvestris_core::Gradient, a: f32, z: f32| {
        linear_gradient(
            90.0,
            linear_color_stop(hsla(g.from), a),
            linear_color_stop(hsla(g.to), z),
        )
    };
    fill_rounded(window, inner, ir, stops(b.lower, split + half, 1.0 - half));
    if split > 0.0 {
        let mask = ContentMask {
            bounds: Bounds::new(
                inner.origin,
                size(split_x - inner.origin.x, inner.size.height),
            ),
        };
        window.with_content_mask(Some(mask), |window| {
            fill_rounded(
                window,
                inner,
                ir,
                stops(b.upper, half, (split - half).max(half)),
            );
        });
    }
}

/// Unified title/toolbar or bottom bar: highlight row, gradient body and a
/// separator row (M: chrome.unified.active, bottombar.column). `top_radius`
/// rounds the window's top corners when the bar starts the window.
pub fn chrome(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    c: &Chrome,
    top_radius: f32,
    separator_on_top: bool,
) {
    let bounds = snap_bounds(window, bounds);
    let h = hairline(window);
    let radii = Corners {
        top_left: px(top_radius),
        top_right: px(top_radius),
        bottom_left: px(0.),
        bottom_right: px(0.),
    };
    let device_rows = bounds.size.height.as_f32() * window.scale_factor();
    window.paint_quad(quad(
        bounds,
        radii,
        rows(c.fill, device_rows),
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    ));
    let (hl_y, sep_y) = if separator_on_top {
        (bounds.origin.y + h, bounds.origin.y)
    } else {
        (bounds.origin.y, bounds.bottom() - h)
    };
    let hl = Bounds::new(point(bounds.origin.x, hl_y), size(bounds.size.width, h));
    window.paint_quad(quad(
        hl,
        radii,
        hsla(c.highlight),
        px(0.),
        transparent_black(),
        BorderStyle::Solid,
    ));
    let sep = Bounds::new(point(bounds.origin.x, sep_y), size(bounds.size.width, h));
    window.paint_quad(gpui::fill(sep, hsla(c.separator)));
}

/// Text well: side border, darker top row, inner shadow row under it and a
/// lighter bottom row (M: toolbar.search.column: 848484, d8d8d8 … e1e1e1).
pub fn field(window: &mut Window, bounds: Bounds<Pixels>, radius: f32, f: &Field, enabled: bool) {
    let bounds = snap_bounds(window, bounds);
    let h = hairline(window);
    let r = px(radius.min(bounds.size.height.as_f32() / 2.0));
    fill_rounded(window, bounds, r, gpui::solid_background(hsla(f.border)));
    let inner = inset(bounds, h);
    let fill = if enabled { f.fill } else { f.fill_disabled };
    fill_rounded(
        window,
        inner,
        (r - h).max(px(0.)),
        gpui::solid_background(hsla(fill)),
    );
    // Top/bottom rows run between the corner arcs.
    let x = bounds.origin.x + r;
    let w = (bounds.size.width - r * 2.0).max(px(0.));
    let row = |y: Pixels| Bounds::new(point(x, y), size(w, h));
    window.paint_quad(gpui::fill(row(bounds.origin.y), hsla(f.border_top)));
    window.paint_quad(gpui::fill(row(bounds.bottom() - h), hsla(f.border_bottom)));
    if enabled && f.inner_shadow != fill {
        window.paint_quad(gpui::fill(row(inner.origin.y), hsla(f.inner_shadow)));
    }
}

/// Focus ring rows around a control edge (`Metrics::focus_ring`).
pub fn focus_ring(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radius: f32,
    color: Rgba8,
    profile: &[(f32, f32)],
) {
    let bounds = snap_bounds(window, bounds);
    let h = hairline(window);
    for &(offset, opacity) in profile {
        let ring = outset(bounds, h * offset);
        let r = px((radius + h.as_f32() * offset).max(0.0));
        let a = (color.a as f32 * opacity).round().clamp(0.0, 255.0) as u8;
        window.paint_quad(quad(
            ring,
            r,
            transparent_black(),
            h,
            hsla(color.alpha(a)),
            BorderStyle::Solid,
        ));
    }
}

/// Fill a closed polygon, used for measured control glyphs (disclosure
/// triangles, check marks, spinner spokes, pointed slider thumbs).
pub fn polygon(window: &mut Window, points: &[Point<Pixels>], color: Rgba8) {
    if points.len() < 3 {
        return;
    }
    let mut path = gpui::PathBuilder::fill();
    path.move_to(points[0]);
    for point in &points[1..] {
        path.line_to(*point);
    }
    path.close();
    if let Ok(path) = path.build() {
        window.paint_path(path, hsla(color));
    }
}

/// A recessed well: 1 px border, a darker top inner row and a soft bottom
/// highlight (M: groupbox.left/top; HIG pp. 361-363).
pub fn well(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    radius: f32,
    fill: Rgba8,
    border: Rgba8,
    shadow: Rgba8,
) {
    let bounds = snap_bounds(window, bounds);
    let h = hairline(window);
    let r = px(radius.min(bounds.size.height.as_f32() / 2.0));
    fill_rounded(window, bounds, r, gpui::solid_background(hsla(border)));
    let inner = inset(bounds, h);
    fill_rounded(
        window,
        inner,
        (r - h).max(px(0.)),
        gpui::solid_background(hsla(fill)),
    );
    let x = inner.origin.x + r;
    let w = (inner.size.width - r * 2.0).max(px(0.));
    window.paint_quad(gpui::fill(
        Bounds::new(point(x, inner.origin.y), size(w, h)),
        hsla(shadow),
    ));
    window.paint_quad(gpui::fill(
        Bounds::new(point(x, inner.bottom() - h), size(w, h)),
        hsla(silvestris_core::Rgba8::hex(0xffffff).alpha(70)),
    ));
}

/// Selection band: gradient body with a one-row line on top
/// (M: sidebar.selection.active: 5595d2 line over 6ca5dd→1a6ab9).
pub fn selection(window: &mut Window, bounds: Bounds<Pixels>, s: &Selection) {
    let bounds = snap_bounds(window, bounds);
    let h = hairline(window);
    let device_rows = bounds.size.height.as_f32() * window.scale_factor();
    window.paint_quad(gpui::fill(bounds, rows(s.fill, device_rows)));
    if s.line != s.fill.from {
        window.paint_quad(gpui::fill(
            Bounds::new(bounds.origin, size(bounds.size.width, h)),
            hsla(s.line),
        ));
    }
}

/// A horizontal line one device row high.
pub fn hline(window: &mut Window, x: Pixels, y: Pixels, width: Pixels, color: Rgba8) {
    let h = hairline(window);
    let x0 = snap_edge(window, x);
    let x1 = snap_edge(window, x + width);
    if x1 <= x0 {
        return;
    }
    window.paint_quad(gpui::fill(
        Bounds::new(point(x0, snap_edge(window, y)), size(x1 - x0, h)),
        hsla(color),
    ));
}

/// A vertical line one device column wide.
pub fn vline(window: &mut Window, x: Pixels, y: Pixels, height: Pixels, color: Rgba8) {
    let h = hairline(window);
    let y0 = snap_edge(window, y);
    let y1 = snap_edge(window, y + height);
    if y1 <= y0 {
        return;
    }
    window.paint_quad(gpui::fill(
        Bounds::new(point(snap_edge(window, x), y0), size(h, y1 - y0)),
        hsla(color),
    ));
}

/// Size control in the window's lower-right corner (HIG PDF p. 221;
/// M: grip.geometry, grip.bar.*, grip.body.*). `corner` is the window's
/// bottom-right point. Each line of `n` pixels runs up and to the right from
/// the bottom row; the shade is the same line one pixel left with one more
/// pixel at the top, the highlight one pixel right with one fewer.
pub fn resize_grip(window: &mut Window, corner: Point<Pixels>, g: &GripStyle) {
    let corner = point(snap_edge(window, corner.x), snap_edge(window, corner.y));
    const LENGTHS: [i32; 3] = [10, 6, 2];
    let one = px(1.0);
    let right = corner.x - one - px(g.inset); // column of the lines' upper ends
    let bottom = corner.y - one - px(g.inset); // row of the lines' lower ends
    let dot = |window: &mut Window, dx: i32, dy: i32, color: Rgba8| {
        if color.a == 0 {
            return;
        }
        let origin = point(right + px(dx as f32), bottom + px(dy as f32));
        window.paint_quad(gpui::fill(Bounds::new(origin, size(one, one)), hsla(color)));
    };
    for n in LENGTHS {
        // Pixel k of the line sits at (k - (n - 1), -k): the top end is column 0.
        let x0 = -(n - 1);
        for k in 0..=n {
            dot(window, x0 - 1 + k, -k, g.shade);
        }
        for k in 0..n - 1 {
            dot(window, x0 + 1 + k, -k, g.highlight);
        }
        for k in 0..n {
            dot(window, x0 + k, -k, g.line);
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::{div, prelude::*, Context, Render, TestAppContext, Window, WindowHandle};

    use super::*;

    struct View;

    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    fn with_scale(scale: f32) -> (TestAppContext, WindowHandle<View>) {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let window = app.add_window(|_, _| View);
        window
            .update(&mut app, |_, window, cx| {
                window.set_scale_factor(scale);
                let _ = cx;
            })
            .unwrap();
        (app, window)
    }

    #[test]
    fn a_hairline_uses_a_whole_number_of_device_pixels() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let (mut app, window) = with_scale(scale);
            window
                .update(&mut app, |_, window, _| {
                    let thickness = hairline(window);
                    let device = thickness.as_f32() * scale;
                    let expected = scale.round().max(1.0);
                    assert!(
                        (device - expected).abs() < 1e-4,
                        "scale {scale}: {thickness:?} is {device} device px, expected {expected}"
                    );
                    assert_eq!(
                        device.fract(),
                        0.0,
                        "scale {scale}: {device} is not a whole device pixel"
                    );
                })
                .unwrap();
        }
    }

    #[test]
    fn edges_snap_to_device_edges_and_strokes_to_device_centres() {
        let (mut app, window) = with_scale(1.25);
        window
            .update(&mut app, |_, window, _| {
                for (from, to) in [(10.0, 10.4), (10.1, 10.4), (10.7, 10.4), (10.8, 11.2)] {
                    assert_eq!(snap_edge(window, px(from)), px(to), "edge {from} at 1.25");
                }
                // A centred one-device stroke must sit on a device centre.
                for from in [10.0, 10.1, 10.3, 10.7] {
                    let centre = snap_stroke_center(window, px(from));
                    let device = centre.as_f32() * 1.25;
                    assert_eq!(
                        (device - 0.5).fract(),
                        0.0,
                        "stroke centre {from} -> {centre:?} ({device} device)"
                    );
                }
                let bounds = Bounds::new(point(px(10.1), px(20.2)), size(px(100.3), px(20.2)));
                let snapped = snap_bounds(window, bounds);
                assert_eq!(snapped.origin.x, px(10.4));
                assert_eq!(snapped.origin.y, px(20.0));
                for edge in [
                    snapped.origin.x,
                    snapped.origin.y,
                    snapped.origin.x + snapped.size.width,
                    snapped.origin.y + snapped.size.height,
                ] {
                    let device = edge.as_f32() * 1.25;
                    assert_eq!(device.fract(), 0.0, "{edge:?} is not a device edge");
                }
            })
            .unwrap();
    }
}
