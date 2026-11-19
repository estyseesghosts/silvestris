//! GPU images for window-control spheres, cached per (theme, light, glyph,
//! scale) and released on theme change (handbook 03: caches "are bounded,
//! invalidated on theme/scale change").

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Global, RenderImage, Window};
use image::{Frame, RgbaImage};
use silvestris_core::sprites::{traffic_light, Glyph};
use silvestris_core::ThemeId;
use smallvec::SmallVec;

use crate::theme::ActiveTheme;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Light {
    Close,
    Minimize,
    Zoom,
    /// Inactive window or disabled button.
    Idle,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    theme: ThemeId,
    light: Light,
    glyph: Glyph,
    /// Scale factor in thousandths.
    scale: u32,
}

#[derive(Default)]
struct SpriteCache {
    revision: u64,
    images: HashMap<Key, Arc<RenderImage>>,
}

impl Global for SpriteCache {}

/// Every light × glyph at three scales. Reaching it means keys are leaking
/// (e.g. a window dragged across many scale factors); the cache restarts.
const LIMIT: usize = 4 * 4 * 3;

/// The sphere image for `light` with `glyph`, at the window's scale. Paint
/// it into `Metrics::traffic_diameter` logical pixels.
pub fn traffic(light: Light, glyph: Glyph, window: &mut Window, cx: &mut App) -> Arc<RenderImage> {
    let theme = cx.theme().clone();
    let revision = cx.theme_revision();
    let scale = window.scale_factor();
    let key = Key {
        theme: theme.id,
        light,
        glyph,
        scale: (scale * 1000.0).round() as u32,
    };
    let stale: Vec<Arc<RenderImage>> = {
        let cache = cx.default_global::<SpriteCache>();
        if cache.revision != revision || cache.images.len() >= LIMIT {
            cache.revision = revision;
            cache.images.drain().map(|(_, v)| v).collect()
        } else {
            Vec::new()
        }
    };
    for image in stale {
        cx.drop_image(image, Some(window));
    }
    if let Some(image) = cx.global::<SpriteCache>().images.get(&key) {
        return image.clone();
    }
    let params = match light {
        Light::Close => &theme.traffic.close,
        Light::Minimize => &theme.traffic.minimize,
        Light::Zoom => &theme.traffic.zoom,
        Light::Idle => &theme.traffic.idle,
    };
    let sprite = traffic_light(params, &theme.traffic.shape, glyph, theme.metrics.traffic_diameter, scale);
    // RenderImage holds straight-alpha BGRA (gpui platform.rs
    // decode_static_image_from_decoder swaps RGBA → BGRA the same way).
    let mut bgra = sprite.rgba;
    for p in bgra.as_chunks_mut::<4>().0 {
        p.swap(0, 2);
    }
    let buffer = RgbaImage::from_raw(sprite.size, sprite.size, bgra).expect("sprite buffer is size²×4");
    let image = Arc::new(RenderImage::new(SmallVec::from_elem(Frame::new(buffer), 1)));
    cx.global_mut::<SpriteCache>().images.insert(key, image.clone());
    image
}
