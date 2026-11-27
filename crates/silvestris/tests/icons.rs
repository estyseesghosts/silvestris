//! Typed artwork vocabularies, role separation and fixed template-image slots.
//!
//! The actual colour rendering is checked by the `open-dialog-*` and
//! `save-sheet-*` goldens: `TestPlatform` cannot read back a frame, and the
//! capture binary is the supported readback path.

use silvestris::icons::{
    ConsumerArtwork, ConsumerTemplateImage, FileArtwork, PictorialImage, PictorialState,
    TemplateImage, TemplateImageSlot,
};

#[test]
fn every_file_artwork_role_resolves() {
    for icon in [
        FileArtwork::Folder,
        FileArtwork::Home,
        FileArtwork::Desktop,
        FileArtwork::Documents,
        FileArtwork::Volume,
        FileArtwork::Network,
        FileArtwork::SmartSearch,
        FileArtwork::Document,
        FileArtwork::Application,
    ] {
        assert_eq!(FileArtwork::from_role(icon.role()), Some(icon), "{icon:?}");
    }
}

#[test]
fn template_images_have_distinct_roles_and_fixed_size_families() {
    for image in [
        TemplateImage::Back,
        TemplateImage::Forward,
        TemplateImage::IconView,
        TemplateImage::ListView,
        TemplateImage::ColumnView,
        TemplateImage::Search,
        TemplateImage::Stop,
        TemplateImage::Refresh,
        TemplateImage::Add,
        TemplateImage::Remove,
        TemplateImage::Action,
        TemplateImage::Info,
        TemplateImage::Sidebar,
        TemplateImage::Inspector,
        TemplateImage::Eject,
    ] {
        assert_eq!(
            TemplateImage::from_role(image.role()),
            Some(image),
            "{image:?}"
        );
        assert_eq!(
            FileArtwork::from_role(image.role()),
            None,
            "{image:?} must resolve only in its own artwork vocabulary"
        );
    }

    assert_eq!(TemplateImageSlot::Toolbar.logical_size(), 19.0);
    assert_eq!(TemplateImageSlot::Segmented.logical_size(), 13.0);
}

#[test]
fn consumer_artwork_keeps_its_external_accessibility_label() {
    let artwork = ConsumerArtwork::new("New Chat", |_, _| gpui::div());

    assert_eq!(artwork.label().as_ref(), "New Chat");
}

#[test]
fn pictorial_images_have_distinct_roles_and_stay_out_of_the_other_vocabularies() {
    for image in [
        PictorialImage::New,
        PictorialImage::Open,
        PictorialImage::Save,
        PictorialImage::Preferences,
    ] {
        assert!(!image.role().is_empty());
        assert_eq!(PictorialImage::SLOT, 32.0);
        assert_eq!(FileArtwork::from_role(image.role()), None);
        assert_eq!(TemplateImage::from_role(image.role()), None, "{image:?}");
    }
}

#[test]
fn consumer_template_images_keep_their_label_and_fixed_slot_families() {
    let glyph = ConsumerTemplateImage::new("Details", |_, _, _| {});

    assert_eq!(glyph.label().as_ref(), "Details");
    assert_eq!(TemplateImageSlot::Toolbar.logical_size(), 19.0);
    assert_eq!(TemplateImageSlot::Segmented.logical_size(), 13.0);
}

#[test]
fn pictorial_state_helpers_describe_the_common_treatment() {
    assert_eq!(PictorialState::default(), PictorialState::normal());
    assert!(!PictorialState::disabled().enabled);
    assert!(!PictorialState::inactive().active_window);
    assert!(PictorialState::normal().enabled);
}

#[test]
fn stateful_consumer_artwork_retains_its_label() {
    let artwork = ConsumerArtwork::with_state("Primary", |state, _, _| {
        assert!(!state.pressed);
        gpui::div()
    });

    assert_eq!(artwork.label().as_ref(), "Primary");
}
