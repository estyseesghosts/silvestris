//! Eject is a provider capability, not a decorative row glyph. A provider that
//! cannot eject shows no control; a failing eject keeps the device and reports
//! the error.

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, Context, Entity, Modifiers, Task, TestAppContext, VisualTestContext,
    Window,
};
use silvestris::file_panel::{
    FileAncestor, FileEntry, FileError, FileListing, FilePanel, FilePlace, FileProvider,
    FileProviderCapabilities, LocationId, MemoryProvider, OpenPanel, PlaceSection,
};
use silvestris::icons::Icon;
use silvestris::{ThemeId, ThemePreference};

struct Body;

impl gpui::Render for Body {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

struct Owner {
    body: Entity<Body>,
}

impl gpui::Render for Owner {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        silvestris::chrome::WindowFrame::new("Document").child(self.body.clone())
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn provider() -> MemoryProvider {
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        vec![FileEntry::document("note", "note.txt")],
    );
    provider.set_places(vec![FilePlace::new(
        "/",
        "Computer",
        Icon::Volume,
        PlaceSection::Devices,
    )
    .eject(true)]);
    provider
}

fn present(provider: Rc<dyn FileProvider>) -> (Entity<FilePanel>, VisualTestContext) {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let mut cx = cx.clone();
    let panel = cx.update(|window, cx| {
        OpenPanel::new(provider)
            .directory("/")
            .begin_sheet(owner, window, cx)
    });
    draw(&mut cx);
    (panel, cx)
}

/// A provider that advertises (or refuses) ejection on top of the memory
/// provider's data.
struct Ejecting {
    inner: MemoryProvider,
    supported: bool,
    failure: Option<&'static str>,
}

impl FileProvider for Ejecting {
    fn children(
        &self,
        location: &LocationId,
        cx: &mut App,
    ) -> Task<Result<FileListing, FileError>> {
        self.inner.children(location, cx)
    }

    fn ancestors(&self, location: &LocationId, cx: &App) -> Vec<FileAncestor> {
        self.inner.ancestors(location, cx)
    }

    fn places(&self, cx: &mut App) -> Task<Result<Vec<FilePlace>, FileError>> {
        self.inner.places(cx)
    }

    fn create_folder(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<LocationId, FileError>> {
        self.inner.create_folder(parent, name, cx)
    }

    fn exists(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<bool, FileError>> {
        self.inner.exists(parent, name, cx)
    }

    fn capabilities(&self) -> FileProviderCapabilities {
        FileProviderCapabilities {
            eject: self.supported,
            ..self.inner.capabilities()
        }
    }

    fn eject(&self, location: &LocationId, cx: &mut App) -> Task<Result<(), FileError>> {
        match self.failure {
            Some(message) => Task::ready(Err(FileError::new(message))),
            None => self.inner.eject(location, cx),
        }
    }
}

fn ejecting(supported: bool, failure: Option<&'static str>) -> Rc<dyn FileProvider> {
    Rc::new(Ejecting {
        inner: provider(),
        supported,
        failure,
    })
}

#[test]
fn a_provider_without_eject_shows_no_row_accessory() {
    let (_panel, mut cx) = present(ejecting(false, None));
    assert!(
        cx.debug_bounds("acc-accessories-/-eject-/").is_none(),
        "a provider that cannot eject must not promise it"
    );
}

#[test]
fn clicking_eject_calls_the_provider_without_navigating() {
    let (panel, mut cx) = present(Rc::new(provider()));
    let accessory = cx
        .debug_bounds("acc-accessories-/-eject-/")
        .expect("eject accessory");
    cx.simulate_click(accessory.center(), Modifiers::default());
    draw(&mut cx);
    assert_eq!(
        cx.read(|cx| panel.read(cx).model().read(cx).location().to_string()),
        "/",
        "eject does not select or navigate into the volume"
    );
    assert!(
        cx.debug_bounds("source-row-/").is_none(),
        "the ejected place leaves the sidebar"
    );
}

#[test]
fn a_failed_eject_keeps_the_device_and_reports_the_error() {
    let (panel, mut cx) = present(ejecting(true, Some("The device is busy.")));
    let accessory = cx
        .debug_bounds("acc-accessories-/-eject-/")
        .expect("eject accessory");
    cx.simulate_click(accessory.center(), Modifiers::default());
    draw(&mut cx);
    assert!(
        cx.debug_bounds("source-row-/").is_some(),
        "the device remains after a failed eject"
    );
    assert_eq!(
        cx.read(|cx| panel
            .read(cx)
            .status_error()
            .map(|error| error.message.to_string())),
        Some("The device is busy.".to_string()),
        "the provider's error is visible in the panel"
    );
}
