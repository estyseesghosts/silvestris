//! GPUI-free core of Silvestris.
//!
//! Theme identities and resolved tokens, the shell layout planner, the
//! command model, preference persistence and the window-control sprite
//! rasterizer. Nothing here depends on GPUI or on any consumer, so it is unit
//! tested without a window server.

pub mod aqua;
pub mod color;
pub mod command;
pub mod geist;
pub mod layout;
pub mod prefs;
pub mod split;
pub mod sprites;
pub mod theme;
pub mod tokens;

pub use color::{Gradient, Rgba8};
pub use theme::{Appearance, ThemeFamily, ThemeId, ThemePreference};
pub use tokens::{FontOverrides, Theme};

/// Family names registered by silvestris-fonts (checked against
/// `TextSystem::all_font_names` at startup by the GPUI crate).
pub mod fonts {
    pub const LATO: &str = "Lato";
    pub const SOURCE_CODE_PRO: &str = "Source Code Pro";
    pub const INTER: &str = "Inter";
    pub const GEIST_MONO: &str = "Geist Mono";
}

/// Resolve tokens for a theme.
pub fn theme(id: ThemeId) -> Theme {
    match (id.family, id.appearance) {
        (ThemeFamily::Aqua, Appearance::Light) => aqua::aqua_light(),
        (ThemeFamily::Aqua, Appearance::Dark) => aqua::aqua_dark(),
        (ThemeFamily::Geist, Appearance::Light) => geist::geist_light(),
        (ThemeFamily::Geist, Appearance::Dark) => geist::geist_dark(),
    }
}

/// Resolve a theme, then apply a global override and a theme-specific one.
///
/// The theme-specific value wins so an app can set one house font everywhere
/// and still give a single theme a different face.
#[must_use]
pub fn theme_with_font_overrides(
    id: ThemeId,
    global: FontOverrides,
    specific: FontOverrides,
) -> Theme {
    let mut theme = theme(id);
    theme.apply_font_overrides(global);
    theme.apply_font_overrides(specific);
    theme
}

#[cfg(test)]
mod conformance {
    //! Theme conformance (#121): every theme resolves, keeps its identity,
    //! and meets WCAG 2.x contrast for primary text (handbook 15: 4.5:1),
    //! except the listed historical values.
    use super::*;

    /// Pairs below 4.5:1 that are kept on purpose. Each needs a reason.
    const EXCEPTIONS: &[(ThemeId, &str, &str)] = &[
        // Measured 10.6 source-list heading (M: sidebar.header.text): bold
        // upper-case 11 pt, ~3.5:1. Kept for fidelity; large-text threshold 3:1.
        (ThemeId::AQUA_LIGHT, "sidebar_heading", "historical"),
        (
            ThemeId::AQUA_DARK,
            "sidebar_heading",
            "mirrors Aqua Light heading weight",
        ),
        // Measured Force Quit table selection, flat 3b77da with white 13 pt
        // text: 4.34:1. Kept for fidelity; Dark Aqua's 2f63b8 passes.
        (ThemeId::AQUA_LIGHT, "table selection", "historical"),
        (
            ThemeId::GEIST_DARK,
            "text_disabled",
            "disabled text is exempt (WCAG 1.4.3)",
        ),
    ];

    fn exempt(id: ThemeId, what: &str) -> bool {
        EXCEPTIONS.iter().any(|(i, w, _)| *i == id && *w == what)
    }

    #[test]
    fn every_theme_resolves_with_its_own_id() {
        for id in ThemeId::ALL {
            assert_eq!(theme(id).id, id);
        }
    }

    #[test]
    fn primary_text_contrast() {
        let mut failures = Vec::new();
        for id in ThemeId::ALL {
            let t = theme(id);
            let c = &t.colors;
            let checks = [
                ("text/window", c.text, c.window, 4.5),
                ("text/content", c.text, c.content, 4.5),
                ("text/sidebar", c.text, c.sidebar, 4.5),
                ("menu_text/menu", c.menu_text, c.menu, 4.5),
                ("tooltip", c.tooltip_text, c.tooltip, 4.5),
                ("sidebar_heading", c.sidebar_heading, c.sidebar, 3.0),
                ("field", t.controls.field.text, t.controls.field.fill, 4.5),
                (
                    "table selection",
                    t.lists.table_active.text,
                    t.lists.table_active.fill.at(0.5),
                    4.5,
                ),
                (
                    "source selection",
                    t.lists.source_active.text,
                    t.lists.source_active.fill.at(0.5),
                    3.0,
                ),
            ];
            for (what, fg, bg, min) in checks {
                let ratio = fg.contrast(bg);
                if ratio < min && !exempt(id, what) {
                    failures.push(format!("{} {what}: {ratio:.2} < {min}", id.label()));
                }
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn states_are_distinguishable() {
        for id in ThemeId::ALL {
            let t = theme(id);
            let p = &t.controls.push;
            assert_ne!(p.normal, p.disabled, "{id:?} push disabled");
            assert_ne!(p.normal.text, p.disabled.text, "{id:?} disabled label");
            let cb = &t.controls.checkbox;
            assert_ne!(cb.normal, cb.selected, "{id:?} checkbox on");
            assert_ne!(
                t.lists.table_active, t.lists.table_inactive,
                "{id:?} selection focus"
            );
            assert_ne!(t.chrome.active, t.chrome.inactive, "{id:?} window activity");
        }
    }

    #[test]
    fn dark_aqua_keeps_aqua_geometry_and_window_control_identity() {
        let l = theme(ThemeId::AQUA_LIGHT);
        let d = theme(ThemeId::AQUA_DARK);
        assert_eq!(l.metrics, d.metrics);
        assert_eq!(l.traffic.close, d.traffic.close);
        assert_eq!(l.traffic.zoom, d.traffic.zoom);
    }
}

pub mod source_list;
