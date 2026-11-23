//! Interaction test harness for GPUI dispatch.
//!
//! The harness proves focus, key bindings, click handlers, timers and window
//! lifecycle through GPUI's production dispatch path. It does not deliver
//! native OS input or window-manager behaviour; that belongs to the platform
//! matrix (#50).
//!
//! Downstream consumers enable the `test-support` feature. Unit tests compile
//! the module automatically.

pub mod scenario;

/// Install the theme and text-field key bindings for a test app.
///
/// Font registration and native platform checks stay outside this harness.
pub fn initialize(preference: crate::ThemePreference, cx: &mut gpui::App) {
    crate::theme::install(preference, cx);
    crate::controls::text_field::bind_keys(cx);
}
