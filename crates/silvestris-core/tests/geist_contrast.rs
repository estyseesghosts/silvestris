//! WCAG 2.x contrast checks for every common Geist control state.
//!
//! Primary text targets 4.5:1. Disabled text is exempt from WCAG 1.4.3 but
//! must still be visibly above its fill.

use silvestris_core::{theme, ThemeId};

fn ratio(foreground: silvestris_core::Rgba8, background: silvestris_core::Rgba8) -> f32 {
    foreground.contrast(background.over(silvestris_core::Rgba8::hex(0x000000)))
}

#[test]
fn geist_text_contrast_covers_normal_hover_pressed_and_selected() {
    for id in [ThemeId::GEIST_LIGHT, ThemeId::GEIST_DARK] {
        let theme = theme(id);
        let states = theme.controls.push;
        let pairs = [
            ("normal", states.normal.text, states.normal.upper.from),
            (
                "hover",
                theme.controls.hover.text,
                theme.controls.hover.upper.from,
            ),
            ("pressed", states.pressed.text, states.pressed.upper.from),
            ("selected", states.selected.text, states.selected.upper.from),
            (
                "field",
                theme.controls.field.text,
                theme.controls.field.fill,
            ),
            ("menu", theme.colors.menu_text, theme.colors.menu),
            (
                "table selection",
                theme.lists.table_active.text,
                theme.lists.table_active.fill.from,
            ),
            (
                "primary action",
                theme.controls.push_default.text,
                theme.controls.push_default.upper.from,
            ),
        ];
        for (state, foreground, background) in pairs {
            let actual = ratio(foreground, background);
            assert!(
                actual >= 4.5,
                "{id:?} {state}: {actual:.2} < 4.5 for {foreground:?} on {background:?}"
            );
        }
        let disabled = ratio(states.disabled.text, states.disabled.upper.from);
        assert!(
            disabled >= 1.5,
            "{id:?} disabled text is illegible: {disabled:.2}"
        );
    }
}
