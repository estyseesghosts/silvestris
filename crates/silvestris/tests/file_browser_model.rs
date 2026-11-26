//! FileBrowserModel and FileProvider (#78): listing, hidden filtering,
//! history, selection by id, view switching and cancellation through the
//! neutral `file_browser` layer.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    div, prelude::*, App, Context, Entity, Render, Task, TestAppContext, VisualTestContext, Window,
};
use silvestris::controls::path_control::LocationId;
use silvestris::file_browser::{
    FileAncestor, FileBrowserEvent, FileBrowserModel, FileEntry, FileError, FileListing, FilePlace,
    FileProvider, FileSort, FileSortKey, FileViewHost, FileViewMode, MemoryProvider, PlaceSection,
};
use silvestris::icons::Icon;
use silvestris::{ThemeId, ThemePreference};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn memory() -> MemoryProvider {
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        vec![
            FileEntry::folder("docs", "Documents"),
            FileEntry::document("report", "report.txt")
                .size(2048)
                .modified(1_700_000_000),
            FileEntry::document("secret", ".secret").hidden(true),
        ],
    );
    provider.add_location(
        "/docs",
        "Documents",
        Icon::Documents,
        Some("/".into()),
        vec![FileEntry::document("note", "note.txt")],
    );
    provider.set_places(vec![
        FilePlace::new("/", "Computer", Icon::Volume, PlaceSection::Devices).eject(true),
        FilePlace::new("/docs", "Documents", Icon::Documents, PlaceSection::Places),
    ]);
    provider
}

fn open(app: &mut TestAppContext) -> (Entity<FileBrowserModel>, VisualTestContext) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let model = app.update(|cx| cx.new(|cx| FileBrowserModel::new(Rc::new(memory()), "/", cx)));
    let window = app.add_window(|_, _| Empty);
    let cx = VisualTestContext::from_window(*window, app);
    cx.run_until_parked();
    (model, cx)
}

fn visible(model: &Entity<FileBrowserModel>, cx: &mut VisualTestContext) -> Vec<String> {
    let model = model.clone();
    cx.read(|cx| {
        model
            .read(cx)
            .controller()
            .read(cx)
            .visible()
            .map(|entry| entry.name.to_string())
            .collect()
    })
}

#[test]
fn a_new_model_lists_the_location_and_reports_loaded() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    let events = Rc::new(std::cell::RefCell::new(Vec::new()));
    let sink = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&model, move |_, event: &FileBrowserEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
    });
    cx.run_until_parked();
    cx.update(|_, cx| model.update(cx, |model, cx| model.reload(cx)));
    cx.run_until_parked();
    assert_eq!(visible(&model, &mut cx), ["Documents", "report.txt"]);
    assert!(events
        .borrow()
        .iter()
        .any(|event| matches!(event, FileBrowserEvent::Loaded { count: 2, .. })));
}

#[test]
fn hidden_files_appear_only_when_enabled() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    assert_eq!(visible(&model, &mut cx), ["Documents", "report.txt"]);
    cx.update(|_, cx| model.update(cx, |model, cx| model.set_show_hidden(true, cx)));
    cx.run_until_parked();
    assert_eq!(
        visible(&model, &mut cx),
        ["Documents", "report.txt", ".secret"]
    );
    cx.update(|_, cx| model.update(cx, |model, cx| model.set_show_hidden(false, cx)));
    cx.run_until_parked();
    assert_eq!(visible(&model, &mut cx), ["Documents", "report.txt"]);
}

#[test]
fn panel_extension_policy_matches_directories_and_extensions() {
    let types = vec!["txt".to_string()];
    let documents = FileEntry::folder("documents", "Documents");
    let report = FileEntry::document("report", "report.txt");
    let png = FileEntry::document("png", "photo.png");
    assert!(silvestris::file_panel::matches_types(
        &documents, &types, false
    ));
    assert!(silvestris::file_panel::matches_types(
        &report, &types, false
    ));
    assert!(!silvestris::file_panel::matches_types(&png, &types, false));
    assert!(silvestris::file_panel::matches_types(&png, &types, true));
    assert!(silvestris::file_panel::matches_types(&png, &[], false));
}

#[test]
fn the_view_host_switches_presentation_without_losing_browser_state() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    let host = cx.update(|_, cx| cx.new(|_| FileViewHost::new(model.clone())));
    cx.update(|_, cx| {
        model.update(cx, |model, cx| {
            model.set_sort(
                Some(FileSort::new(
                    FileSortKey::Name,
                    silvestris::controls::SortDirection::Descending,
                )),
                cx,
            );
            model.controller().update(cx, |controller, cx| {
                controller.select("report");
                cx.notify();
            });
        });
        host.update(cx, |host, cx| host.show_icons(cx));
    });
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| host.read(cx).view(cx)), FileViewMode::Icons);
    assert_eq!(cx.read(|cx| host.read(cx).location(cx).to_string()), "/");
    assert_eq!(
        cx.read(|cx| host
            .read(cx)
            .selected_ids(cx)
            .into_iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()),
        ["report"]
    );
    assert_eq!(
        cx.read(|cx| host.read(cx).sort(cx)),
        Some(FileSort::new(
            FileSortKey::Name,
            silvestris::controls::SortDirection::Descending
        ))
    );
    cx.update(|_, cx| host.update(cx, |host, cx| host.show_columns(cx)));
    assert_eq!(cx.read(|cx| host.read(cx).view(cx)), FileViewMode::Columns);
    assert_eq!(cx.read(|cx| host.read(cx).location(cx).to_string()), "/");
}

#[test]
fn history_back_and_forward_move_the_model_but_never_the_selection() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    cx.update(|_, cx| {
        let controller = model.read(cx).controller();
        controller.update(cx, |controller, cx| {
            controller.select("report");
            cx.notify();
        });
        model.update(cx, |model, cx| model.navigate("/docs", cx));
    });
    cx.run_until_parked();
    assert_eq!(visible(&model, &mut cx), ["note.txt"]);
    assert!(cx.read(|cx| model.read(cx).can_go_back()));
    assert!(cx.read(|cx| model
        .read(cx)
        .controller()
        .read(cx)
        .selected_ids()
        .is_empty()));
    cx.update(|_, cx| model.update(cx, |model, cx| model.go_back(cx)));
    cx.run_until_parked();
    assert_eq!(visible(&model, &mut cx), ["Documents", "report.txt"]);
    assert!(cx.read(|cx| model.read(cx).can_go_forward()));
    cx.update(|_, cx| model.update(cx, |model, cx| model.go_forward(cx)));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| model.read(cx).location().to_string()), "/docs");
}

#[test]
fn reloading_keeps_the_selection_by_id() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    cx.update(|_, cx| {
        let controller = model.read(cx).controller();
        controller.update(cx, |controller, cx| {
            controller.select("report");
            cx.notify();
        });
        model.update(cx, |model, cx| model.reload(cx));
    });
    cx.run_until_parked();
    assert_eq!(
        cx.read(|cx| model
            .read(cx)
            .controller()
            .read(cx)
            .selected_ids()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()),
        ["report"]
    );
}

#[test]
fn switching_the_view_mode_keeps_location_and_selection() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    cx.update(|_, cx| {
        let controller = model.read(cx).controller();
        controller.update(cx, |controller, cx| {
            controller.select("docs");
            cx.notify();
        });
        model.update(cx, |model, cx| model.set_view(FileViewMode::Columns, cx));
    });
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| model.read(cx).view()), FileViewMode::Columns);
    assert_eq!(cx.read(|cx| model.read(cx).location().to_string()), "/");
    assert_eq!(
        cx.read(|cx| model.read(cx).controller().read(cx).selected_ids().to_vec()),
        ["docs"]
    );
}

#[test]
fn an_older_location_result_never_overwrites_a_newer_one() {
    struct Delayed {
        inner: MemoryProvider,
        root_delay: Duration,
    }

    impl FileProvider for Delayed {
        fn children(
            &self,
            location: &LocationId,
            cx: &mut App,
        ) -> Task<Result<FileListing, FileError>> {
            let entries = self.inner.entries(location);
            let delay = if location.as_ref() == "/" {
                self.root_delay
            } else {
                Duration::ZERO
            };
            let executor = cx.background_executor().clone();
            cx.background_executor().spawn(async move {
                executor.timer(delay).await;
                Ok(FileListing::new(entries))
            })
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
    }

    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::default(), cx));
    let provider = Delayed {
        inner: memory(),
        root_delay: Duration::from_millis(100),
    };
    let model = app.update(|cx| cx.new(|cx| FileBrowserModel::new(Rc::new(provider), "/", cx)));
    let window = app.add_window(|_, _| Empty);
    let mut cx = VisualTestContext::from_window(*window, &app);
    cx.update(|_, cx| model.update(cx, |model, cx| model.navigate("/docs", cx)));
    cx.run_until_parked();
    assert_eq!(cx.read(|cx| model.read(cx).location().to_string()), "/docs");
    assert_eq!(visible(&model, &mut cx), ["note.txt"]);
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    assert_eq!(
        cx.read(|cx| model.read(cx).location().to_string()),
        "/docs",
        "the older root listing cannot overwrite the newer location"
    );
    assert_eq!(visible(&model, &mut cx), ["note.txt"]);
}
