//! Silvestris: a Snow Leopard Aqua and Geist application shell for GPUI.
//!
//! Call `init` once, then open windows with `chrome::window_options` and
//! compose controls and the `shell`.

pub mod chrome;
pub mod controls;
pub mod convert;
pub mod fonts;
pub mod native;
pub mod paint;
pub mod sprites;
pub mod theme;

pub use silvestris_core as core;
pub use silvestris_core::{Appearance, ThemeFamily, ThemeId, ThemePreference};
pub use theme::{set_theme_preference, system_appearance_changed, ActiveTheme};

/// Register bundled fonts and install the theme global. Call once, before
/// opening windows.
pub fn init(preference: ThemePreference, cx: &mut gpui::App) -> anyhow::Result<()> {
    fonts::register(cx)?;
    theme::install(preference, cx);
    controls::text_field::bind_keys(cx);
    Ok(())
}

pub mod sidebar;

pub mod command;

pub mod menu;

pub mod preferences;

pub mod shell;

/// Scenario harness for downstream GPUI interaction tests. Font binary and
/// native registration checks remain separate from this harness.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
