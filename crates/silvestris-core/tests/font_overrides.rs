//! Consumer font overrides keep measured tokens and respect precedence.

use silvestris_core::{theme, theme_with_font_overrides, FontOverrides, ThemeId};

#[test]
fn a_global_override_changes_only_the_family_names() {
    let base = theme(ThemeId::GEIST_DARK);
    let overridden = theme_with_font_overrides(
        ThemeId::GEIST_DARK,
        FontOverrides::new().ui("House Sans").mono("House Mono"),
        FontOverrides::default(),
    );

    assert_eq!(overridden.type_.ui, "House Sans");
    assert_eq!(overridden.type_.mono, "House Mono");
    assert_eq!(overridden.type_.size_regular, base.type_.size_regular);
    assert_eq!(overridden.type_.size_mono, base.type_.size_mono);
    assert_eq!(overridden.colors, base.colors);
    assert_eq!(overridden.metrics, base.metrics);
}

#[test]
fn a_theme_override_wins_over_the_global_one() {
    let overridden = theme_with_font_overrides(
        ThemeId::GEIST_LIGHT,
        FontOverrides::new().ui("Global Sans"),
        FontOverrides::new().ui("Geist Theme Sans"),
    );
    assert_eq!(overridden.type_.ui, "Geist Theme Sans");
}

#[test]
fn only_the_named_family_is_replaced() {
    let base = theme(ThemeId::AQUA_LIGHT);
    let overridden = theme_with_font_overrides(
        ThemeId::AQUA_LIGHT,
        FontOverrides::new().mono("House Mono"),
        FontOverrides::default(),
    );
    assert_eq!(overridden.type_.ui, base.type_.ui);
    assert_eq!(overridden.type_.mono, "House Mono");
}

#[test]
fn an_empty_override_is_a_no_op() {
    for id in [
        ThemeId::AQUA_LIGHT,
        ThemeId::AQUA_DARK,
        ThemeId::GEIST_LIGHT,
        ThemeId::GEIST_DARK,
    ] {
        assert_eq!(
            theme_with_font_overrides(id, FontOverrides::default(), FontOverrides::default()),
            theme(id)
        );
    }
}
