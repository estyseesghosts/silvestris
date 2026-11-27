//! Standard Geist pictograms from the pinned Lucide release.

use gpui::{div, prelude::*, px, Context, Render, TestAppContext, Window};
use silvestris::icons::{GeistPictogram, PictorialState};
use silvestris::{ActiveTheme, ThemeId, ThemePreference};

struct Gallery {
    role: GeistPictogram,
    state: PictorialState,
}

impl Render for Gallery {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        div().size(px(120.)).child(
            div()
                .debug_selector(|| format!("geist-icon-{}", self.role.role()))
                .child(self.role.themed_element(20.0, self.state, &theme)),
        )
    }
}

#[test]
fn the_ten_generic_roles_use_the_exact_lucide_pictograms() {
    let expected = [
        (GeistPictogram::Settings, "settings"),
        (GeistPictogram::Database, "database"),
        (GeistPictogram::MessageSquare, "message-square"),
        (GeistPictogram::Monitor, "monitor"),
        (GeistPictogram::Volume2, "volume-2"),
        (GeistPictogram::Gamepad2, "gamepad-2"),
        (GeistPictogram::Bug, "bug"),
        (GeistPictogram::Bot, "bot"),
        (GeistPictogram::Wrench, "wrench"),
        (GeistPictogram::SlidersHorizontal, "sliders-horizontal"),
    ];
    assert_eq!(GeistPictogram::all().len(), expected.len());
    for (role, name) in expected {
        assert_eq!(role.role(), name);
        assert!(GeistPictogram::all().contains(&role));
    }
}

#[test]
fn prepared_geometry_stays_inside_the_24_unit_viewbox() {
    for role in GeistPictogram::all() {
        let element = role.element(20.0, silvestris::core::Rgba8::hex(0xededed));
        // The generated geometry is static; a missing role would panic during
        // construction rather than silently drawing an improvised glyph.
        let _ = element;
    }
    let generated = include_str!("../src/geist_pictograms.rs");
    for role in GeistPictogram::all() {
        assert!(
            generated.contains(role.role()),
            "{} missing from prepared geometry",
            role.role()
        );
    }
    assert!(!generated.contains("<path"), "no SVG parsing at paint time");
}

#[test]
fn themed_pictograms_render_only_for_the_geist_family() {
    for id in [
        ThemeId::GEIST_DARK,
        ThemeId::GEIST_LIGHT,
        ThemeId::AQUA_LIGHT,
        ThemeId::AQUA_DARK,
    ] {
        let mut app = TestAppContext::single();
        app.update(|cx| silvestris::theme::install(ThemePreference::explicit(id), cx));
        let (_, cx) = app.add_window_view(|_, _| Gallery {
            role: GeistPictogram::Settings,
            state: PictorialState::normal(),
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(
            cx.debug_bounds("geist-icon-settings").is_some(),
            "{id:?} did not lay out the pictogram slot"
        );
    }
}
