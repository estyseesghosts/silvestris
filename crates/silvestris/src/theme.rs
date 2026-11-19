//! The active theme as a GPUI global, with live switching (#121).
//!
//! One `ThemeState` per app: every window renders from it, and a change
//! refreshes all windows without rebuilding view state, so focus handles and
//! selections survive a switch (handbook 15 "A switch between Aqua
//! Light/Dark must not reset the underlying focus handle").

use std::sync::Arc;

use gpui::{App, Global, WindowAppearance};
use silvestris_core::{Appearance, Theme, ThemeId, ThemePreference};

pub struct ThemeState {
    preference: ThemePreference,
    system: Appearance,
    theme: Arc<Theme>,
    /// Bumped on every change; caches key on it.
    revision: u64,
}

impl Global for ThemeState {}

pub fn appearance_of(a: WindowAppearance) -> Appearance {
    match a {
        WindowAppearance::Dark | WindowAppearance::VibrantDark => Appearance::Dark,
        WindowAppearance::Light | WindowAppearance::VibrantLight => Appearance::Light,
    }
}

/// Install the theme global. Called by `silvestris::init`.
pub(crate) fn install(preference: ThemePreference, cx: &mut App) {
    let system = appearance_of(cx.window_appearance());
    let id = preference.resolve(system);
    cx.set_global(ThemeState {
        preference,
        system,
        theme: Arc::new(silvestris_core::theme(id)),
        revision: 0,
    });
}

/// Read access to the active theme from any context that derefs to `App`.
pub trait ActiveTheme {
    fn theme(&self) -> &Arc<Theme>;
    fn theme_preference(&self) -> ThemePreference;
    fn theme_revision(&self) -> u64;
}

impl ActiveTheme for App {
    fn theme(&self) -> &Arc<Theme> {
        &self.global::<ThemeState>().theme
    }
    fn theme_preference(&self) -> ThemePreference {
        self.global::<ThemeState>().preference
    }
    fn theme_revision(&self) -> u64 {
        self.global::<ThemeState>().revision
    }
}

fn apply(cx: &mut App, preference: ThemePreference, system: Appearance) {
    let id: ThemeId = preference.resolve(system);
    let state = cx.global_mut::<ThemeState>();
    state.preference = preference;
    state.system = system;
    if state.theme.id != id {
        state.theme = Arc::new(silvestris_core::theme(id));
        state.revision += 1;
        cx.refresh_windows();
    }
}

/// Change the user's theme choice; all windows repaint.
pub fn set_theme_preference(preference: ThemePreference, cx: &mut App) {
    let system = cx.global::<ThemeState>().system;
    apply(cx, preference, system);
}

/// Feed an OS appearance change (from `Window::observe_window_appearance`).
/// Only matters when the preference follows the system.
pub fn system_appearance_changed(appearance: WindowAppearance, cx: &mut App) {
    let preference = cx.global::<ThemeState>().preference;
    apply(cx, preference, appearance_of(appearance));
}
