//! Register the bundled fonts with GPUI and prove they resolved.

use std::borrow::Cow;

use anyhow::{bail, Context as _};
use gpui::App;

/// Add every face in `silvestris_fonts::FACES` and check that each required
/// family is now known to the text system. Fails loudly instead of letting
/// GPUI fall back to a system font (handbook 11: "Do not let silent Segoe UI
/// or macOS system-font fallback create an apparently passing screenshot").
pub fn register(cx: &mut App) -> anyhow::Result<()> {
    let faces: Vec<Cow<'static, [u8]>> = silvestris_fonts::FACES
        .iter()
        .map(|f| Cow::Borrowed(f.bytes))
        .collect();
    cx.text_system()
        .add_fonts(faces)
        .context("registering bundled fonts")?;
    let names = cx.text_system().all_font_names();
    let missing: Vec<&str> = silvestris_fonts::FAMILIES
        .iter()
        .copied()
        .filter(|family| !names.iter().any(|n| n == family))
        .collect();
    if !missing.is_empty() {
        bail!(
            "bundled font families not registered: {missing:?}. The bytes in silvestris-fonts are \
             corrupt or the platform text system rejected them (see fonts/manifest.json)"
        );
    }
    Ok(())
}
