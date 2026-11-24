//! Persistent file-panel presentation state.
//!
//! HIG p. 245 says the Open dialog remembers its last size; pp. 247-248 say the
//! Save panel remembers its expanded state and Hide extension. The panel does
//! not own a global: the consumer supplies a [`PanelStateStore`], normally an
//! adapter over the application's preferences entity. A memory store serves
//! tests and the gallery.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{App, Entity};
use serde::{Deserialize, Serialize};
use silvestris_core::prefs::Versioned;

use super::model::FileViewMode;
use crate::prefs::Preferences;

/// The presentation choices a panel remembers between invocations.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelState {
    pub view: FileViewMode,
    pub expanded: bool,
    pub hide_extension: bool,
    pub sidebar_width: f32,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            // 10.6 Open opens in column view (4.15.47 AM capture).
            view: FileViewMode::Columns,
            expanded: false,
            hide_extension: false,
            // Measured sidebar + scroller width (M: panel.geometry).
            sidebar_width: 160.0,
        }
    }
}

/// The application's store for panel presentation state.
pub trait PanelStateStore {
    fn load(&self, cx: &App) -> PanelState;
    fn save(&self, state: PanelState, cx: &mut App);
}

/// A store for tests and the gallery. Share it through `Rc` across panel
/// invocations to exercise the persistence rules.
#[derive(Default)]
pub struct MemoryPanelStateStore {
    state: Cell<PanelState>,
}

impl MemoryPanelStateStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn state(&self) -> PanelState {
        self.state.get()
    }
}

impl PanelStateStore for MemoryPanelStateStore {
    fn load(&self, _: &App) -> PanelState {
        self.state.get()
    }

    fn save(&self, state: PanelState, _: &mut App) {
        self.state.set(state);
    }
}

impl<T: PanelStateStore + ?Sized> PanelStateStore for Rc<T> {
    fn load(&self, cx: &App) -> PanelState {
        (**self).load(cx)
    }

    fn save(&self, state: PanelState, cx: &mut App) {
        (**self).save(state, cx)
    }
}

/// Adapter over the application's preferences entity. `state` and
/// `state_mut` select the panel state field, so the panel state lives in the
/// app's own versioned document rather than in a Silvestris global.
pub struct PreferencePanelStates<T: Versioned> {
    preferences: Entity<Preferences<T>>,
    state: fn(&T) -> &PanelState,
    state_mut: fn(&mut T) -> &mut PanelState,
}

impl<T: Versioned> PreferencePanelStates<T> {
    #[must_use]
    pub fn new(
        preferences: Entity<Preferences<T>>,
        state: fn(&T) -> &PanelState,
        state_mut: fn(&mut T) -> &mut PanelState,
    ) -> Self {
        Self {
            preferences,
            state,
            state_mut,
        }
    }
}

impl<T> PanelStateStore for PreferencePanelStates<T>
where
    T: Versioned + Clone + PartialEq + 'static,
{
    fn load(&self, cx: &App) -> PanelState {
        *(self.state)(self.preferences.read(cx).value())
    }

    fn save(&self, state: PanelState, cx: &mut App) {
        let state_mut = self.state_mut;
        self.preferences.update(cx, |preferences, cx| {
            let _ = preferences.set(
                "file-panel.state",
                |value| {
                    *state_mut(value) = state;
                },
                cx,
            );
        });
    }
}
