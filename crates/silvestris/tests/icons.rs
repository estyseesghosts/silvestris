//! Typed artwork vocabularies, role separation and fixed template-image slots.
//!
//! The actual colour rendering is checked by the `open-dialog-*` and
//! `save-sheet-*` goldens: `TestPlatform` cannot read back a frame, and the
//! capture binary is the supported readback path.

use silvestris::icons::{ConsumerArtwork, FileArtwork, TemplateImage, TemplateImageSlot};

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
