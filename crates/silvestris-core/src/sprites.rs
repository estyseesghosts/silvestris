//! CPU rasterizer for window-control spheres.
//!
//! GPUI at the pin has two-stop linear gradients only, so the radial gloss of
//! a 10.6 window button is drawn here and uploaded once per (theme, state,
//! scale) as an image (handbook 10: "Traffic-light spheres: state-specific
//! vector/raster at 1x/2x"). The model is original analytic art whose shape
//! parameters (`SphereShape`) were fitted by tools/metrology/fit_traffic.py,
//! which this file mirrors operation for operation; no Apple pixels are used.

use crate::color::Rgba8;
use crate::tokens::{SphereShape, TrafficLight};

/// Which glyph to draw on hover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    None,
    Close,
    Minimize,
    Zoom,
}

/// RGBA8, straight alpha, row-major, `size × size`.
pub struct Sprite {
    pub size: u32,
    pub rgba: Vec<u8>,
}

type Rgb = [f32; 3];

fn rgb(c: Rgba8) -> Rgb {
    [c.r as f32, c.g as f32, c.b as f32]
}

fn lerp(a: Rgb, b: Rgb, k: f32) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * k,
        a[1] + (b[1] - a[1]) * k,
        a[2] + (b[2] - a[2]) * k,
    ]
}

/// `clip(1 - ((u/ax)^2 + ((v-cy)/ay)^2), 0, 1) ^ p`
fn ellipse(u: f32, v: f32, ax: f32, cy: f32, ay: f32, p: f32) -> f32 {
    let q = (u / ax).powi(2) + ((v - cy) / ay).powi(2);
    (1.0 - q).clamp(0.0, 1.0).powf(p)
}

/// Colour of one sample at device position (`x`, `y`) of a `size`-pixel
/// sprite, or `None` outside the disc. Mirrors `fit_traffic.render`.
fn sample(
    light: &TrafficLight,
    s: &SphereShape,
    x: f32,
    y: f32,
    size: f32,
    scale: f32,
) -> Option<Rgb> {
    let c0 = size / 2.0;
    let r = s.radius * c0;
    let (dx, dy) = (x - c0, y - c0);
    let d = (dx * dx + dy * dy).sqrt();
    if d > r {
        return None;
    }
    let (u, v) = (dx / r, dy / r);
    let t = y / size;
    let ring = lerp(
        rgb(light.ring_top),
        rgb(light.ring_bottom),
        t.powf(s.ring_pow),
    );
    let mut c = rgb(light.deep);
    // Darken toward the top ring.
    let k = ((s.top_end - v) / (s.top_end + 1.0))
        .clamp(0.0, 1.0)
        .powi(2)
        * s.top_k;
    c = lerp(c, ring, k);
    // Radial glow centred low in the sphere.
    c = lerp(
        c,
        rgb(light.glow),
        ellipse(u, v, s.glow_x, s.glow_y, s.glow_ry, s.glow_pow),
    );
    // Edge shading toward the ring.
    let k = ((d / r - s.edge_start) / (1.0 - s.edge_start))
        .clamp(0.0, 1.0)
        .powi(2)
        * s.edge_k;
    c = lerp(c, ring, k);
    if d > r - s.ring_width * scale {
        c = ring;
    }
    // Specular cap.
    c = lerp(
        c,
        rgb(light.specular),
        ellipse(u, v, s.spec_x, s.spec_y, s.spec_ry, s.spec_pow),
    );
    Some(c)
}

/// Draw one sphere of logical diameter `diameter` at `scale` device pixels per
/// logical pixel, box-filtering 4×4 samples per pixel (fit_traffic.SS).
pub fn traffic_light(
    light: &TrafficLight,
    shape: &SphereShape,
    glyph: Glyph,
    diameter: f32,
    scale: f32,
) -> Sprite {
    const SS: u32 = 4;
    let size = (diameter * scale).round().max(1.0) as u32;
    let sizef = size as f32;
    // One logical pixel in units of the nominal disc radius, for glyph strokes.
    let glyph_px = scale / sizef * 2.0;
    let mut rgba = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let mut acc = [0f32; 3];
            let mut n = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    let x = px as f32 + (sx as f32 + 0.5) / SS as f32;
                    let y = py as f32 + (sy as f32 + 0.5) / SS as f32;
                    let Some(c) = sample(light, shape, x, y, sizef, scale) else {
                        continue;
                    };
                    let half = sizef / 2.0;
                    let c = draw_glyph(
                        glyph,
                        light.glyph,
                        c,
                        (x - half) / half,
                        (y - half) / half,
                        glyph_px,
                    );
                    acc[0] += c[0];
                    acc[1] += c[1];
                    acc[2] += c[2];
                    n += 1;
                }
            }
            if n > 0 {
                let i = ((py * size + px) * 4) as usize;
                let nf = n as f32;
                rgba[i] = (acc[0] / nf).round() as u8;
                rgba[i + 1] = (acc[1] / nf).round() as u8;
                rgba[i + 2] = (acc[2] / nf).round() as u8;
                rgba[i + 3] = (nf / (SS * SS) as f32 * 255.0).round() as u8;
            }
        }
    }
    Sprite { size, rgba }
}

/// Glyph strokes in unit-disc coordinates; `px` is one logical pixel in those
/// units. designed: the hover glyphs were not captured (aqua.rs `glyph`).
fn draw_glyph(glyph: Glyph, ink: Rgba8, under: Rgb, x: f32, y: f32, px: f32) -> Rgb {
    let half = 0.75 * px; // 1.5 px stroke
    let len = 0.42;
    let hit = match glyph {
        Glyph::None => false,
        Glyph::Minimize => x.abs() <= len && y.abs() <= half,
        Glyph::Zoom => (x.abs() <= len && y.abs() <= half) || (y.abs() <= len && x.abs() <= half),
        Glyph::Close => {
            let d1 = (x - y).abs() / std::f32::consts::SQRT_2;
            let d2 = (x + y).abs() / std::f32::consts::SQRT_2;
            let r = (x * x + y * y).sqrt();
            r <= len * 0.95 && (d1 <= half || d2 <= half)
        }
    };
    if hit {
        lerp(under, rgb(ink), ink.a as f32 / 255.0)
    } else {
        under
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aqua::aqua_light;

    #[test]
    fn corners_are_transparent_and_centre_opaque() {
        let t = aqua_light();
        let s = traffic_light(&t.traffic.close, &t.traffic.shape, Glyph::None, 14.0, 1.0);
        assert_eq!(s.size, 14);
        assert_eq!(s.rgba[3], 0, "top-left corner must be clear");
        let centre = ((7 * 14 + 7) * 4) as usize;
        assert_eq!(s.rgba[centre + 3], 255);
    }

    #[test]
    fn scales_to_device_pixels() {
        let t = aqua_light();
        let shape = &t.traffic.shape;
        assert_eq!(
            traffic_light(&t.traffic.zoom, shape, Glyph::Zoom, 14.0, 1.5).size,
            21
        );
        assert_eq!(
            traffic_light(&t.traffic.zoom, shape, Glyph::Zoom, 14.0, 2.0).size,
            28
        );
    }

    /// Pixels of the same model rendered by fit_traffic.render (straight
    /// colour, alpha = coverage), so the Rust and Python stay in step.
    #[test]
    fn matches_the_fitted_python_model() {
        let t = aqua_light();
        let s = traffic_light(&t.traffic.close, &t.traffic.shape, Glyph::None, 14.0, 1.0);
        for &(row, col, want) in PYTHON_CLOSE {
            let i = ((row * 14 + col) * 4) as usize;
            let got = [s.rgba[i], s.rgba[i + 1], s.rgba[i + 2], s.rgba[i + 3]];
            for k in 0..4 {
                assert!(
                    (got[k] as i32 - want[k] as i32).abs() <= 1,
                    "pixel ({row},{col}): rust {got:?} python {want:?}"
                );
            }
        }
    }

    // Output of `python tools/metrology/fit_traffic.py --rust-check`.
    const PYTHON_CLOSE: &[(u32, u32, [u8; 4])] = &[
        (0, 7, [115, 58, 55, 255]),
        (2, 7, [237, 203, 202, 255]),
        (4, 2, [217, 71, 63, 255]),
        (5, 7, [235, 73, 64, 255]),
        (7, 0, [141, 86, 83, 255]),
        (7, 7, [251, 121, 114, 255]),
        (9, 3, [252, 135, 129, 255]),
        (11, 7, [253, 187, 185, 255]),
        (12, 11, [183, 132, 130, 128]),
        (13, 7, [197, 147, 145, 255]),
    ];
}
