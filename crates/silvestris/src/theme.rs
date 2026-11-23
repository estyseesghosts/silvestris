//! The active theme as a GPUI global, with live switching (#121).
//!
//! One `ThemeState` per app: every window renders from it, and a change
//! refreshes all windows without rebuilding view state, so focus handles and
//! selections survive a switch (handbook 15 "A switch between Aqua
//! Light/Dark must not reset the underlying focus handle").

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, Global, WindowAppearance};
use silvestris_core::{Appearance, FontOverrides, Theme, ThemeId, ThemePreference};

pub struct ThemeState {
    preference: ThemePreference,
    system: Appearance,
    theme: Arc<Theme>,
    /// Bumped on every change; caches key on it.
    revision: u64,
    /// Family substitutions applied to every theme.
    global_fonts: FontOverrides,
    /// Family substitutions for one theme, taking precedence over `global_fonts`.
    theme_fonts: HashMap<ThemeId, FontOverrides>,
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
        theme: Arc::new(silvestris_core::theme_with_font_overrides(
            id,
            FontOverrides::default(),
            FontOverrides::default(),
        )),
        revision: 0,
        global_fonts: FontOverrides::default(),
        theme_fonts: HashMap::new(),
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
        rebuild_theme(state, id);
        cx.refresh_windows();
    }
}

fn rebuild_theme(state: &mut ThemeState, id: ThemeId) {
    let specific = state.theme_fonts.get(&id).copied().unwrap_or_default();
    state.theme = Arc::new(silvestris_core::theme_with_font_overrides(
        id,
        state.global_fonts,
        specific,
    ));
    state.revision = state.revision.wrapping_add(1);
}

/// Replace the UI and/or monospace family for every theme.
///
/// This is a consumer integration point: an application can adopt its house
/// font without forking a theme. Sizes, colours and metrics are untouched, and
/// a per-theme override set with [`set_font_overrides`] still wins.
pub fn set_global_font_overrides(overrides: FontOverrides, cx: &mut App) {
    let current = cx.global::<ThemeState>().theme.id;
    {
        let state = cx.global_mut::<ThemeState>();
        state.global_fonts = overrides;
        rebuild_theme(state, current);
    }
    cx.refresh_windows();
}

/// Replace the UI and/or monospace family for one theme.
///
/// The override survives appearance switches and is reapplied whenever that
/// theme becomes active. It takes precedence over the global override.
pub fn set_font_overrides(id: ThemeId, overrides: FontOverrides, cx: &mut App) {
    let current = cx.global::<ThemeState>().theme.id;
    {
        let state = cx.global_mut::<ThemeState>();
        state.theme_fonts.insert(id, overrides);
        if id == current {
            rebuild_theme(state, current);
        }
    }
    if id == current {
        cx.refresh_windows();
    }
}

/// Remove every global and per-theme font override.
pub fn clear_font_overrides(cx: &mut App) {
    let current = cx.global::<ThemeState>().theme.id;
    {
        let state = cx.global_mut::<ThemeState>();
        state.global_fonts = FontOverrides::default();
        state.theme_fonts.clear();
        rebuild_theme(state, current);
    }
    cx.refresh_windows();
}

/// The overrides currently registered for a theme, for diagnostics and tests.
#[must_use]
pub fn font_overrides(cx: &App, id: ThemeId) -> (FontOverrides, FontOverrides) {
    let state = cx.global::<ThemeState>();
    (
        state.global_fonts,
        state.theme_fonts.get(&id).copied().unwrap_or_default(),
    )
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
