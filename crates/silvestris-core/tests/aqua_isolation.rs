//! Aqua token isolation.
//!
//! The fixtures were captured from the clean baseline commit before any Geist
//! change. They are literal files, not values computed from the implementation
//! under test, so a Geist or shared-constructor regression fails this suite.

use silvestris_core::{theme, theme_with_font_overrides, FontOverrides, ThemeId};

#[path = "support/snapshot.rs"]
mod snapshot;

use snapshot::Snapshot;

const AQUA_LIGHT: &str = include_str!("aqua_light.tokens");
const AQUA_DARK: &str = include_str!("aqua_dark.tokens");

fn rendered(id: ThemeId) -> String {
    let theme = theme(id);
    format!("{:#?}\n", Snapshot::from(&theme))
}

fn assert_matches_fixture(id: ThemeId, fixture: &str) {
    assert_eq!(rendered(id), fixture, "{id:?} resolved tokens changed");
}

#[test]
fn aqua_light_keeps_its_baseline_tokens() {
    assert_matches_fixture(ThemeId::AQUA_LIGHT, AQUA_LIGHT);
}

#[test]
fn aqua_dark_keeps_its_baseline_tokens() {
    assert_matches_fixture(ThemeId::AQUA_DARK, AQUA_DARK);
}

#[test]
fn every_theme_resolves_and_keeps_its_identity_after_resolving_other_themes() {
    for id in ThemeId::ALL {
        let expected = theme(id);
        assert_eq!(expected.id, id);
        for other in ThemeId::ALL {
            let _ = theme(other);
        }
        assert_eq!(theme(id), expected, "{id:?} changed after other themes");
    }
}

#[test]
fn switching_through_geist_returns_aqua_to_the_baseline() {
    let before = theme(ThemeId::AQUA_LIGHT);
    for id in [
        ThemeId::GEIST_DARK,
        ThemeId::GEIST_LIGHT,
        ThemeId::AQUA_DARK,
        ThemeId::AQUA_LIGHT,
    ] {
        let _ = theme(id);
    }
    assert_eq!(theme(ThemeId::AQUA_LIGHT), before);
    assert_matches_fixture(ThemeId::AQUA_LIGHT, AQUA_LIGHT);
}

#[test]
fn global_and_theme_font_overrides_have_the_same_precedence_for_all_themes() {
    let global = FontOverrides::new().ui("House Sans").mono("House Mono");
    let specific = FontOverrides::new().ui("Theme Sans");
    for id in ThemeId::ALL {
        let global_only = theme_with_font_overrides(id, global, FontOverrides::default());
        assert_eq!(global_only.type_.ui, "House Sans", "{id:?} global ui");
        assert_eq!(global_only.type_.mono, "House Mono", "{id:?} global mono");
        assert_eq!(global_only.type_.size_regular, theme(id).type_.size_regular);

        let both = theme_with_font_overrides(id, global, specific);
        assert_eq!(both.type_.ui, "Theme Sans", "{id:?} specific ui must win");
        assert_eq!(both.type_.mono, "House Mono", "{id:?} global mono survives");
        assert_eq!(both.colors, theme(id).colors, "{id:?} colours changed");
        assert_eq!(both.metrics, theme(id).metrics, "{id:?} metrics changed");
    }
}

#[test]
fn overrides_leave_the_aqua_fixture_untouched_when_cleared() {
    let overrides = FontOverrides::new().ui("House Sans").mono("House Mono");
    for id in [ThemeId::AQUA_LIGHT, ThemeId::AQUA_DARK] {
        let overridden = theme_with_font_overrides(id, overrides, FontOverrides::default());
        assert_eq!(overridden.type_.ui, "House Sans");
        assert_eq!(overridden.type_.mono, "House Mono");
    }
    assert_matches_fixture(ThemeId::AQUA_LIGHT, AQUA_LIGHT);
    assert_matches_fixture(ThemeId::AQUA_DARK, AQUA_DARK);
}
