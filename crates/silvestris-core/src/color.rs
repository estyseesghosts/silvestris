//! Plain sRGB colours and two-stop gradients, independent of GPUI.

use serde::{Deserialize, Serialize};

/// 8-bit sRGB colour with straight (non-premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    pub const TRANSPARENT: Rgba8 = Rgba8::rgba(0, 0, 0, 0);

    /// Opaque colour from `0xRRGGBB`.
    pub const fn hex(rgb: u32) -> Self {
        Self {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
            a: 255,
        }
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Same colour with alpha `a` (0-255).
    pub const fn alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Linear interpolation in sRGB space, which is what Quartz used for
    /// interface gradients and what GPUI's default `ColorSpace::Srgb` does.
    pub fn mix(self, other: Rgba8, t: f32) -> Rgba8 {
        let t = t.clamp(0.0, 1.0);
        let lerp = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgba8 {
            r: lerp(self.r, other.r),
            g: lerp(self.g, other.g),
            b: lerp(self.b, other.b),
            a: lerp(self.a, other.a),
        }
    }

    /// Composite `self` over an opaque `background`.
    pub fn over(self, background: Rgba8) -> Rgba8 {
        let a = self.a as f32 / 255.0;
        let c = |f: u8, b: u8| (f as f32 * a + b as f32 * (1.0 - a)).round() as u8;
        Rgba8::rgba(
            c(self.r, background.r),
            c(self.g, background.g),
            c(self.b, background.b),
            255,
        )
    }

    /// WCAG 2.x relative luminance.
    pub fn luminance(self) -> f32 {
        let channel = |v: u8| {
            let c = v as f32 / 255.0;
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }

    /// WCAG 2.x contrast ratio between two opaque colours.
    pub fn contrast(self, other: Rgba8) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

/// A two-stop gradient along one axis (top→bottom unless a painter says
/// otherwise). GPUI's `linear_gradient` at the pinned revision takes exactly
/// two stops (gpui/src/color.rs `Background::colors: [LinearColorStop; 2]`),
/// so multi-stop materials are expressed as several bands of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gradient {
    pub from: Rgba8,
    pub to: Rgba8,
}

impl Gradient {
    pub const fn new(from: Rgba8, to: Rgba8) -> Self {
        Self { from, to }
    }
    pub const fn flat(color: Rgba8) -> Self {
        Self {
            from: color,
            to: color,
        }
    }
    pub fn at(&self, t: f32) -> Rgba8 {
        self.from.mix(self.to, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_of_black_and_white_is_21() {
        let c = Rgba8::hex(0x000000).contrast(Rgba8::hex(0xffffff));
        assert!((c - 21.0).abs() < 0.01, "{c}");
    }

    #[test]
    fn over_composites() {
        let half_black = Rgba8::rgba(0, 0, 0, 128);
        assert_eq!(half_black.over(Rgba8::hex(0xffffff)), Rgba8::hex(0x7f7f7f));
    }
}
