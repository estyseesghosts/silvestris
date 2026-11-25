//! The local-disk provider against a real temporary directory.
//!
//! These tests exercise the adapter that a consumer actually ships, not the
//! deterministic memory provider: opaque ids, exact host paths, platform name
//! rules, fallible existence, rename, folder creation and trash.

#![cfg(feature = "std-fs")]

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{div, prelude::*, Task, TestAppContext, VisualTestContext, Window};
use silvestris::file_panel::{FileError, FileKind, FileProvider, LocationId, StdFsProvider};

struct Empty;

impl gpui::Render for Empty {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

fn scratch(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let path =
        std::env::temp_dir().join(format!("silvestris-{label}-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&path).expect("scratch directory");
    path
}

fn app() -> (TestAppContext, VisualTestContext) {
    let mut app = TestAppContext::single();
    let window = app.add_window(|_, _| Empty);
    let cx = VisualTestContext::from_window(*window, &app);
    (app, cx)
}

fn wait<T: 'static>(
    cx: &mut VisualTestContext,
    task: Task<Result<T, FileError>>,
) -> Result<T, FileError> {
    let slot = Rc::new(RefCell::new(None));
    let sink = slot.clone();
    cx.spawn(move |_| async move {
        *sink.borrow_mut() = Some(task.await);
    })
    .detach();
    cx.run_until_parked();
    let result = slot.borrow_mut().take();
    result.expect("the provider task completed")
}

/// A listing arrives in two stages: names and kinds first, metadata second.
fn enrich(
    cx: &mut VisualTestContext,
    provider: &StdFsProvider,
    listing: &mut silvestris::file_panel::FileListing,
) {
    let Some(task) = cx.update(|_, cx| provider.metadata_enrichment(&listing.entries, cx)) else {
        return;
    };
    let updates = wait(cx, task).expect("metadata enrichment");
    let by_id: std::collections::HashMap<_, _> = updates.into_iter().collect();
    for entry in &mut listing.entries {
        if let Some(metadata) = by_id.get(&entry.id) {
            metadata.apply(entry);
        }
    }
}

#[test]
fn listing_returns_opaque_handles_not_display_paths() {
    let root = scratch("listing");
    std::fs::create_dir(root.join("docs")).unwrap();
    std::fs::write(root.join("report.txt"), b"hello").unwrap();
    std::fs::write(root.join("caf\u{e9}.txt"), b"accent").unwrap();

    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).expect("root location");
    assert!(
        location.starts_with("fs-location-"),
        "the public id is opaque: {location}"
    );
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let mut listing = wait(&mut cx, task).expect("directory listing");
    // The cheap pass reports names immediately; metadata arrives separately.
    assert!(listing.entries.iter().all(|entry| !entry.metadata_known));
    enrich(&mut cx, &provider, &mut listing);
    assert!(listing.warnings.is_empty(), "{:?}", listing.warnings);
    assert_eq!(listing.entries.len(), 3);

    for name in ["docs", "report.txt", "caf\u{e9}.txt"] {
        let entry = listing
            .entries
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} listed"));
        assert!(
            entry.location.starts_with("fs-location-"),
            "{name} has an opaque location"
        );
        assert_ne!(
            entry.location, entry.name,
            "the identity is not the display name"
        );
        assert_eq!(
            provider.resolve(&entry.location),
            Some(root.join(name)),
            "the provider keeps the exact host path"
        );
    }
    let docs = listing
        .entries
        .iter()
        .find(|entry| entry.name == "docs")
        .unwrap();
    assert!(docs.is_dir);
    assert_eq!(docs.kind, FileKind::Folder);
    let report = listing
        .entries
        .iter()
        .find(|entry| entry.name == "report.txt")
        .unwrap();
    assert_eq!(report.kind, FileKind::Document);
    assert_eq!(report.size, 5);

    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn a_non_utf8_name_keeps_its_exact_path() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = scratch("non-utf8");
    let raw = OsString::from_vec(vec![b'n', 0xff, b'a', b'm', b'e', b'.', b't', b'x', b't']);
    let exact = root.join(&raw);
    std::fs::write(&exact, b"bytes").unwrap();

    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let listing = wait(&mut cx, task).expect("directory listing");

    let entry = listing
        .entries
        .iter()
        .find(|entry| provider.resolve(&entry.location).as_deref() == Some(exact.as_path()))
        .expect("the non-UTF-8 entry is listed");
    assert!(
        entry.name.contains('\u{fffd}'),
        "display text is lossy, as expected: {}",
        entry.name
    );
    assert_eq!(provider.resolve(&entry.location), Some(exact));

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn existence_is_fallible_and_unknown_locations_are_errors() {
    let root = scratch("exists");
    std::fs::write(root.join("here.txt"), b"x").unwrap();
    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();

    let task = cx.update(|_, cx| provider.exists(&location, "here.txt", cx));
    assert_eq!(wait(&mut cx, task), Ok(true));
    let task = cx.update(|_, cx| provider.exists(&location, "missing.txt", cx));
    assert_eq!(wait(&mut cx, task), Ok(false));

    let unknown = LocationId::from("fs-location-99999999");
    let task = cx.update(|_, cx| provider.exists(&unknown, "here.txt", cx));
    assert!(
        wait(&mut cx, task).is_err(),
        "an unresolved location is an error, not a false answer"
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn rename_refuses_a_collision_and_create_folder_is_real() {
    let root = scratch("mutate");
    std::fs::write(root.join("report.txt"), b"x").unwrap();
    std::fs::write(root.join("taken.txt"), b"x").unwrap();

    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let listing = wait(&mut cx, task).unwrap();
    let report = listing
        .entries
        .iter()
        .find(|entry| entry.name == "report.txt")
        .unwrap()
        .clone();

    let task = cx.update(|_, cx| provider.rename(&location, &report.id, "taken.txt", cx));
    assert!(
        wait(&mut cx, task).is_err(),
        "an existing target is refused"
    );
    assert!(root.join("report.txt").exists());

    let task = cx.update(|_, cx| provider.rename(&location, &report.id, "notes.txt", cx));
    assert_eq!(wait(&mut cx, task), Ok(()));
    assert!(!root.join("report.txt").exists());
    assert!(root.join("notes.txt").exists());

    let task = cx.update(|_, cx| provider.create_folder(&location, "new folder", cx));
    let folder = wait(&mut cx, task).expect("folder location");
    assert!(provider
        .resolve(&folder)
        .is_some_and(|path| path == root.join("new folder")));
    assert!(root.join("new folder").is_dir());

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn platform_name_rules_are_applied_by_the_provider() {
    let root = scratch("names");
    let (_app, _cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();

    #[cfg(windows)]
    {
        let cx = _cx;
        for name in ["CON", "con.txt", "LPT9.log", "bad<name", "trail.", "trail "] {
            assert!(
                cx.read(|cx| provider.validate_name(&location, name, cx))
                    .is_err(),
                "{name} is invalid on Windows"
            );
        }
        assert!(cx
            .read(|cx| provider.validate_name(&location, "report.txt", cx))
            .is_ok());
    }
    #[cfg(unix)]
    {
        let cx = _cx;
        assert!(
            cx.read(|cx| provider.validate_name(&location, "report:2024.txt", cx))
                .is_ok(),
            "a colon is a legal Unix name"
        );
        assert!(cx
            .read(|cx| provider.validate_name(&location, "bad/name", cx))
            .is_err());
    }

    std::fs::remove_dir_all(root).ok();
}

#[cfg(windows)]
#[test]
fn trash_removes_the_original_path() {
    let root = scratch("trash");
    let path = root.join("recycle-me.txt");
    std::fs::write(&path, b"x").unwrap();
    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let listing = wait(&mut cx, task).unwrap();
    let entry = listing
        .entries
        .iter()
        .find(|entry| entry.name == "recycle-me.txt")
        .unwrap()
        .clone();

    let task = cx.update(|_, cx| provider.trash(&location, &entry.id, cx));
    wait(&mut cx, task).expect("move to the Recycle Bin");
    assert!(!path.exists(), "the original path is gone");
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn rename_preserves_the_provider_handle_including_descendants() {
    let root = scratch("rename-identity");
    std::fs::create_dir_all(root.join("alpha/sub")).unwrap();
    std::fs::write(root.join("alpha/sub/leaf.txt"), b"x").unwrap();

    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let listing = wait(&mut cx, task).unwrap();
    let alpha = listing
        .entries
        .iter()
        .find(|entry| entry.name == "alpha")
        .unwrap()
        .clone();
    let sub_location = provider.child_location(&alpha.location, "sub").unwrap();
    assert_eq!(
        provider.resolve(&sub_location),
        Some(root.join("alpha/sub"))
    );

    let task = cx.update(|_, cx| provider.rename(&location, &alpha.id, "beta", cx));
    assert_eq!(wait(&mut cx, task), Ok(()));
    assert_eq!(
        provider.resolve(&alpha.location),
        Some(root.join("beta")),
        "the selected handle follows the rename"
    );
    assert_eq!(
        provider.resolve(&sub_location),
        Some(root.join("beta/sub")),
        "an already-interned descendant follows its directory"
    );

    std::fs::remove_dir_all(root).ok();
}

#[cfg(any(windows, target_os = "macos"))]
#[test]
fn a_case_only_rename_is_not_a_collision() {
    let root = scratch("case-rename");
    std::fs::write(root.join("Report.txt"), b"x").unwrap();

    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let listing = wait(&mut cx, task).unwrap();
    let report = listing.entries.first().unwrap().clone();
    let task = cx.update(|_, cx| provider.rename(&location, &report.id, "report.txt", cx));
    assert_eq!(wait(&mut cx, task), Ok(()));
    assert_eq!(
        provider.resolve(&report.location),
        Some(root.join("report.txt"))
    );

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn interned_handles_stay_bounded_while_many_directories_are_visited() {
    let root = scratch("intern-budget");
    for directory in 0..200 {
        let path = root.join(format!("dir-{directory}"));
        std::fs::create_dir(&path).unwrap();
        for file in 0..8 {
            std::fs::write(path.join(format!("file-{file}.txt")), b"x").unwrap();
        }
    }

    let (_app, mut cx) = app();
    let provider = StdFsProvider::with_intern_capacity(64);
    let root_location = provider.location_for(&root).unwrap();
    cx.update(|_, cx| provider.retain(std::slice::from_ref(&root_location), cx));
    for directory in 0..200 {
        let path = root.join(format!("dir-{directory}"));
        let location = provider.location_for(&path).unwrap();
        let task = cx.update(|_, cx| provider.children(&location, cx));
        wait(&mut cx, task).expect("listing");
    }
    assert!(
        provider.interned_count() <= 64 + 16,
        "cold path handles are reclaimed: {}",
        provider.interned_count()
    );
    assert!(
        provider.resolve(&root_location).is_some(),
        "the retained root survived eviction"
    );

    std::fs::remove_dir_all(root).ok();
}

#[cfg(unix)]
#[test]
fn symlinks_are_classified_by_their_target() {
    use std::os::unix::fs::symlink;

    let root = scratch("links");
    std::fs::create_dir(root.join("real-dir")).unwrap();
    std::fs::write(root.join("real-file.txt"), b"x").unwrap();
    symlink(root.join("real-dir"), root.join("dir-link")).unwrap();
    symlink(root.join("missing"), root.join("broken-link")).unwrap();

    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    let listing = wait(&mut cx, task).expect("directory listing");

    let dir_link = listing
        .entries
        .iter()
        .find(|entry| entry.name == "dir-link")
        .unwrap();
    assert!(dir_link.is_link);
    assert!(dir_link.is_dir);
    assert_eq!(dir_link.kind, FileKind::Folder);

    let broken = listing
        .entries
        .iter()
        .find(|entry| entry.name == "broken-link")
        .unwrap();
    assert!(broken.is_link);
    assert!(!broken.is_dir);
    assert!(!broken.metadata_known);
    assert_eq!(broken.kind, FileKind::Alias);
    assert!(!listing.warnings.is_empty());

    std::fs::remove_dir_all(root).ok();
}

#[test]
fn a_missing_directory_is_a_fatal_error() {
    let root = scratch("missing");
    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let location = provider.location_for(&root).unwrap();
    std::fs::remove_dir_all(&root).unwrap();
    let task = cx.update(|_, cx| provider.children(&location, cx));
    assert!(
        wait(&mut cx, task).is_err(),
        "a directory that cannot be opened is an error"
    );
}

#[test]
fn places_are_discovered_without_touching_display_paths() {
    let (_app, mut cx) = app();
    let provider = StdFsProvider::new();
    let task = cx.update(|_, cx| provider.places(cx));
    let places = wait(&mut cx, task).expect("host places");
    assert!(
        !places.is_empty(),
        "a real host has at least a home or a volume"
    );
    for place in &places {
        assert!(
            place.id.starts_with("fs-location-"),
            "place ids are provider handles: {}",
            place.id
        );
        assert!(provider.resolve(&place.id).is_some());
    }
    assert!(
        places.iter().all(|place| !place.id.starts_with("search:")),
        "the local provider must not invent search locations"
    );
}
