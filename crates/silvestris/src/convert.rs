//! Token → GPUI conversions.

use gpui::{linear_color_stop, linear_gradient, Background, Hsla, Pixels, Rgba, Window};
use silvestris_core::{Gradient, Rgba8};

/// Exact sRGB conversion (GPUI converts to `Hsla` internally).
pub fn rgba(c: Rgba8) -> Rgba {
    Rgba {
        r: c.r as f32 / 255.0,
        g: c.g as f32 / 255.0,
        b: c.b as f32 / 255.0,
        a: c.a as f32 / 255.0,
    }
}

pub fn hsla(c: Rgba8) -> Hsla {
    rgba(c).into()
}

/// Vertical gradient whose first and last *pixel rows* are exactly `from`
/// and `to`. GPUI evaluates the gradient at pixel centres
/// (gpui_windows shaders.hlsl `t = dot(position - center, …)`), so stops are
/// placed at the centres of the first and last rows.
pub fn rows(g: Gradient, height: f32) -> Background {
    if g.from == g.to || height <= 1.0 {
        return gpui::solid_background(hsla(g.from));
    }
    let half = 0.5 / height;
    linear_gradient(
        180.0,
        linear_color_stop(hsla(g.from), half),
        linear_color_stop(hsla(g.to), 1.0 - half),
    )
}

/// Vertical gradient between two fractions of the quad height (the paint
/// stack's upper/lower gloss bands).
pub fn band(g: Gradient, start: f32, end: f32) -> Background {
    if g.from == g.to {
        return gpui::solid_background(hsla(g.from));
    }
    linear_gradient(
        180.0,
        linear_color_stop(hsla(g.from), start),
        linear_color_stop(hsla(g.to), end),
    )
}

/// Horizontal gradient (left → right), for vertical scrollers and rails.
pub fn across(g: Gradient, width: f32) -> Background {
    if g.from == g.to || width <= 1.0 {
        return gpui::solid_background(hsla(g.from));
    }
    let half = 0.5 / width;
    linear_gradient(
        90.0,
        linear_color_stop(hsla(g.from), half),
        linear_color_stop(hsla(g.to), 1.0 - half),
    )
}

/// One device pixel rounded to a whole number of device pixels, in logical
/// pixels: 1 at 1x, 1 at 2x (two device px), 4/3 at 1.5x. Keeps rings and
/// separators the same device width everywhere on screen at fractional
/// scales instead of alternating between 1 and 2 device pixels.
pub fn hairline(window: &Window) -> Pixels {
    let s = window.scale_factor();
    gpui::px(s.round().max(1.0) / s)
}
