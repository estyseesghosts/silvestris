//! Cheap listings put names on screen first; metadata is applied later by id,
//! under the same generation guard as the listing itself.

use std::rc::Rc;
use std::time::Duration;

use gpui::{
    div, prelude::*, App, Context, Entity, Render, Task, TestAppContext, VisualTestContext, Window,
};
use silvestris::file_panel::{
    FileAncestor, FileEntry, FileError, FileKind, FileListing, FileMetadata, FilePlace,
    FileProvider, FileSort, FileSortKey, FileViewMode, LocationId, MemoryProvider,
};
use silvestris::icons::Icon;
use silvestris::{ThemeId, ThemePreference};

struct Empty;

impl Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

struct SlowMetadata {
    inner: MemoryProvider,
    delay: Duration,
}

impl FileProvider for SlowMetadata {
    fn children(
        &self,
        location: &LocationId,
        cx: &mut App,
    ) -> Task<Result<FileListing, FileError>> {
        let listing = self.inner.children(location, cx);
        // The inner provider is ready; strip metadata to model a cheap pass.
        let task = cx.background_executor().spawn(async move {
            let mut listing = listing.await?;
            for entry in &mut listing.entries {
                entry.metadata_known = false;
                entry.size = 0;
                entry.modified = 0;
            }
            Ok(listing)
        });
        task
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

    fn metadata_enrichment(
        &self,
        entries: &[FileEntry],
        cx: &mut App,
    ) -> Option<Task<Result<Vec<(silvestris::binding::ItemId, FileMetadata)>, FileError>>> {
        let delay = self.delay;
        let out: Vec<_> = entries
            .iter()
            .map(|entry| {
                let size = if entry.name == "alpha.txt" { 20 } else { 10 };
                let hidden = entry.name == "secret.txt";
                let metadata = FileMetadata {
                    kind: FileKind::Document,
                    is_dir: false,
                    is_package: false,
                    is_link: false,
                    metadata_known: true,
                    size,
                    modified: 100 + size,
                    hidden,
                    icon: Icon::Document,
                };
                (entry.id.clone(), metadata)
            })
            .collect();
        let executor = cx.background_executor().clone();
        Some(cx.background_executor().spawn(async move {
            executor.timer(delay).await;
            Ok(out)
        }))
    }
}

fn provider() -> SlowMetadata {
    let inner = MemoryProvider::new();
    inner.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        vec![
            FileEntry::document("b", "beta.txt"),
            FileEntry::document("a", "alpha.txt"),
            FileEntry::document("s", "secret.txt"),
        ],
    );
    SlowMetadata {
        inner,
        delay: Duration::from_millis(50),
    }
}

fn open(
    app: &mut TestAppContext,
) -> (
    Entity<silvestris::file_panel::FileBrowserModel>,
    VisualTestContext,
) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let model = app.update(|cx| {
        cx.new(|cx| silvestris::file_panel::FileBrowserModel::new(Rc::new(provider()), "/", cx))
    });
    let window = app.add_window(|_, _| Empty);
    let cx = VisualTestContext::from_window(*window, app);
    cx.run_until_parked();
    (model, cx)
}

fn rows(
    model: &Entity<silvestris::file_panel::FileBrowserModel>,
    cx: &mut VisualTestContext,
) -> Vec<(String, bool, u64)> {
    let model = model.clone();
    cx.read(|cx| {
        model
            .read(cx)
            .controller()
            .read(cx)
            .visible()
            .map(|entry| (entry.name.to_string(), entry.metadata_known, entry.size))
            .collect()
    })
}

#[test]
fn names_arrive_before_metadata_and_the_answer_is_applied_by_id() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    assert_eq!(
        rows(&model, &mut cx),
        [
            ("beta.txt".to_string(), false, 0),
            ("alpha.txt".to_string(), false, 0),
            ("secret.txt".to_string(), false, 0),
        ],
        "the cheap listing appears without waiting for stat"
    );

    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(
        rows(&model, &mut cx),
        [
            ("beta.txt".to_string(), true, 10),
            ("alpha.txt".to_string(), true, 20),
        ],
        "metadata fills in for the same ids; a host-hidden entry disappears"
    );

    // With Show Hidden Files, the same late metadata keeps it on screen.
    cx.update(|_, cx| {
        model.update(cx, |model, cx| {
            model.set_show_hidden(true, cx);
        })
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert!(rows(&model, &mut cx)
        .iter()
        .any(|(name, _, _)| name == "secret.txt"));
    cx.update(|_, cx| {
        model.update(cx, |model, cx| {
            model.set_show_hidden(false, cx);
        })
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert!(!rows(&model, &mut cx)
        .iter()
        .any(|(name, _, _)| name == "secret.txt"));
}

#[test]
fn a_size_sort_reorders_once_after_enrichment_and_keeps_the_selection() {
    let mut app = TestAppContext::single();
    let (model, mut cx) = open(&mut app);
    cx.update(|_, cx| {
        model.update(cx, |model, cx| {
            model.set_view(FileViewMode::List, cx);
            model.set_sort(
                Some(FileSort::new(
                    FileSortKey::Size,
                    silvestris::controls::SortDirection::Descending,
                )),
                cx,
            );
            model.controller().update(cx, |controller, cx| {
                controller.select("a");
                cx.notify();
            });
        });
    });
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    assert_eq!(
        rows(&model, &mut cx),
        [
            ("alpha.txt".to_string(), true, 20),
            ("beta.txt".to_string(), true, 10),
        ],
        "metadata changes the order exactly once the answer arrives"
    );
    assert_eq!(
        cx.read(|cx| model
            .read(cx)
            .controller()
            .read(cx)
            .selection()
            .map(ToString::to_string)),
        Some("a".to_string()),
        "selection survives enrichment and the re-sort"
    );
}
