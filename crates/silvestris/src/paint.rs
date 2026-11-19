//! Material painter: the handbook 10 paint stack expressed as GPUI quads.
//!
//! Painters take resolved tokens and bounds, never theme names, so Aqua and
//! Geist share one code path and differ only in data (#121).

use gpui::{point, px, quad, size, transparent_black, BorderStyle, Bounds, ContentMask, Corners, Pixels, Point, Window};
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

/// Round a logical coordinate to the device-pixel grid.
fn snap(window: &Window, v: Pixels) -> Pixels {
    let s = window.scale_factor();
    px((v.as_f32() * s).round() / s)
}

fn fill_rounded(window: &mut Window, b: Bounds<Pixels>, r: Pixels, bg: gpui::Background) {
    window.paint_quad(quad(b, r, bg, px(0.), transparent_black(), BorderStyle::Solid));
}

/// Paint a bezel: emboss line under it, border ring (top→bottom gradient),
/// then the lower gloss band and the upper band clipped above the split
/// (M: button.push.normal and siblings; handbook 10 "layered gloss").
///
/// Both bands are painted as the full inner rounded rectangle so capsule
/// ends stay round; the upper band is clipped with a content mask, and the
/// lower band holds `lower.from` above its first stop.
pub fn bevel(window: &mut Window, bounds: Bounds<Pixels>, radius: f32, b: &Bevel) {
    let h = hairline(window);
    let r = px(radius.min(bounds.size.height.as_f32() / 2.0));
    if b.emboss.a > 0 {
        let mut e = bounds;
        e.origin.y += h;
        fill_rounded(window, e, r, gpui::solid_background(hsla(b.emboss)));
    }
    let scale = window.scale_factor();
    fill_rounded(window, bounds, r, rows(b.border, bounds.size.height.as_f32() * scale));
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
            bounds: Bounds::new(inner.origin, size(inner.size.width, split_y - inner.origin.y)),
        };
        window.with_content_mask(Some(mask), |window| {
            fill_rounded(window, inner, ir, band(b.upper, half, (split - half).max(half)));
        });
    }
}

/// Unified title/toolbar or bottom bar: highlight row, gradient body and a
/// separator row (M: chrome.unified.active, bottombar.column). `top_radius`
/// rounds the window's top corners when the bar starts the window.
pub fn chrome(window: &mut Window, bounds: Bounds<Pixels>, c: &Chrome, top_radius: f32, separator_on_top: bool) {
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
    window.paint_quad(quad(hl, radii, hsla(c.highlight), px(0.), transparent_black(), BorderStyle::Solid));
    let sep = Bounds::new(point(bounds.origin.x, sep_y), size(bounds.size.width, h));
    window.paint_quad(gpui::fill(sep, hsla(c.separator)));
}

/// Text well: side border, darker top row, inner shadow row under it and a
/// lighter bottom row (M: toolbar.search.column: 848484, d8d8d8 … e1e1e1).
pub fn field(window: &mut Window, bounds: Bounds<Pixels>, radius: f32, f: &Field, enabled: bool) {
    let h = hairline(window);
    let r = px(radius.min(bounds.size.height.as_f32() / 2.0));
    fill_rounded(window, bounds, r, gpui::solid_background(hsla(f.border)));
    let inner = inset(bounds, h);
    let fill = if enabled { f.fill } else { f.fill_disabled };
    fill_rounded(window, inner, (r - h).max(px(0.)), gpui::solid_background(hsla(fill)));
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
pub fn focus_ring(window: &mut Window, bounds: Bounds<Pixels>, radius: f32, color: Rgba8, profile: &[(f32, f32)]) {
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

/// Selection band: gradient body with a one-row line on top
/// (M: sidebar.selection.active: 5595d2 line over 6ca5dd→1a6ab9).
pub fn selection(window: &mut Window, bounds: Bounds<Pixels>, s: &Selection) {
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
    window.paint_quad(gpui::fill(Bounds::new(point(x, y), size(width, h)), hsla(color)));
}

/// A vertical line one device column wide.
pub fn vline(window: &mut Window, x: Pixels, y: Pixels, height: Pixels, color: Rgba8) {
    let h = hairline(window);
    window.paint_quad(gpui::fill(Bounds::new(point(x, y), size(h, height)), hsla(color)));
}

/// Size control in the window's lower-right corner (HIG PDF p. 221;
/// M: grip.geometry, grip.bar.*, grip.body.*). `corner` is the window's
/// bottom-right point. Each line of `n` pixels runs up and to the right from
/// the bottom row; the shade is the same line one pixel left with one more
/// pixel at the top, the highlight one pixel right with one fewer.
pub fn resize_grip(window: &mut Window, corner: Point<Pixels>, g: &GripStyle) {
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
