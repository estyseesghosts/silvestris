//! Full-colour file icons (#48 subset): the designed vocabulary and role
//! table shared by sidebars, list cells, grid cells and toolbar glyphs.
//!
//! The actual colour rendering is checked by the `open-dialog-*` and
//! `save-sheet-*` goldens: `TestPlatform` cannot read back a frame, and the
//! capture binary is the supported readback path.

use silvestris::icons::Icon;

#[test]
fn every_panel_role_resolves_including_the_colour_icons() {
    for icon in [
        Icon::Folder,
        Icon::Home,
        Icon::Desktop,
        Icon::Documents,
        Icon::Volume,
        Icon::Network,
        Icon::SmartSearch,
        Icon::Document,
        Icon::Application,
        Icon::Back,
        Icon::Forward,
        Icon::IconView,
        Icon::List,
        Icon::Columns,
        Icon::Eject,
        Icon::Search,
    ] {
        assert_eq!(Icon::from_role(icon.role()), Some(icon), "{icon:?}");
    }
}
