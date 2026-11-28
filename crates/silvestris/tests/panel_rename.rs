//! A failed inline rename reports the provider error and leaves the row and
//! selection coherent instead of silently snapping the name back.

use std::rc::Rc;

use gpui::{
    div, prelude::*, App, Context, Entity, Modifiers, MouseButton, Task, TestAppContext,
    VisualTestContext, Window,
};
use silvestris::file_panel::{
    FileAncestor, FileEntry, FileError, FileListing, FilePlace, FileProvider, LocationId,
    MemoryProvider, OpenPanel, PlaceSection,
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

struct FailingRename {
    inner: MemoryProvider,
}

impl FileProvider for FailingRename {
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

    fn capabilities(&self) -> silvestris::file_panel::FileProviderCapabilities {
        self.inner.capabilities()
    }

    fn rename(
        &self,
        _: &LocationId,
        _: &silvestris::binding::ItemId,
        _: &str,
        _: &App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new("Permission denied.")))
    }
}

fn provider() -> FailingRename {
    let inner = MemoryProvider::new();
    inner.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        vec![FileEntry::document("report", "report.txt")],
    );
    inner.set_places(vec![FilePlace::new(
        "/",
        "Computer",
        Icon::Volume,
        PlaceSection::Devices,
    )]);
    FailingRename { inner }
}

#[test]
fn a_failed_rename_surfaces_the_error_and_keeps_the_row() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(silvestris::controls::text_field::bind_keys);
    app.update(|cx| cx.set_reduce_motion(true));
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let mut cx = cx.clone();
    let panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider()))
            .directory("/")
            .begin_sheet(owner, window, cx)
    });
    draw(&mut cx);
    let list = cx.debug_bounds("segment-list").expect("list segment");
    cx.simulate_click(list.center(), Modifiers::default());
    draw(&mut cx);

    let row = cx.debug_bounds("table-row-report").expect("report row");
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(row.center(), MouseButton::Right, Modifiers::default());
    draw(&mut cx);
    let rename = cx.debug_bounds("menu-item-0-2").expect("Rename row");
    cx.simulate_click(rename.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.read(|cx| panel.read(cx).is_editing(cx)));

    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("notes.txt");
    cx.simulate_keystrokes("enter");
    draw(&mut cx);

    assert!(!cx.read(|cx| panel.read(cx).is_editing(cx)));
    assert_eq!(
        cx.read(|cx| panel
            .read(cx)
            .status_error()
            .map(|error| error.message.to_string())),
        Some("Permission denied.".to_string())
    );
    assert_eq!(
        cx.read(|cx| panel
            .read(cx)
            .model()
            .read(cx)
            .controller()
            .read(cx)
            .selection()
            .map(ToString::to_string)),
        Some("report".to_string()),
        "the row keeps its identity after the failure"
    );
    assert_eq!(
        cx.read(|cx| panel
            .read(cx)
            .model()
            .read(cx)
            .controller()
            .read(cx)
            .visible()
            .map(|entry| entry.name.to_string())
            .collect::<Vec<_>>()),
        ["report.txt"],
        "the failed name is not pretended to have succeeded"
    );
}
