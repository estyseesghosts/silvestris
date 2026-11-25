//! The local-disk `FileProvider`.
//!
//! Host paths are kept in this adapter, never in the UI. A `LocationId` is an
//! opaque handle (`fs-location-00000001`) mapped to an exact `PathBuf`;
//! display labels may be lossy, but IO never is. The map is shared behind an
//! `Arc<Mutex<...>>` so background listing can register child handles.
//!
//! All directory, metadata and mutation work runs on GPUI's background
//! executor. Only map lookups happen on the caller's thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use gpui::{App, SharedString, Task};

use super::provider::{
    modified_seconds, validate_structural_name, FileAncestor, FileEntry, FileError, FileKind,
    FileListing, FileMetadata, FilePlace, FileProvider, FileProviderCapabilities, MetadataUpdates,
    PlaceSection,
};
use crate::binding::ItemId;
use crate::controls::path_control::LocationId;
use crate::icons::Icon;

#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
mod platform;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(all(unix, not(target_os = "linux"), not(target_os = "macos")))]
#[path = "unix.rs"]
mod platform;

/// A sidebar destination found by the host adapter.
struct HostPlace {
    path: PathBuf,
    label: SharedString,
    icon: Icon,
    section: PlaceSection,
    eject: bool,
}

impl HostPlace {
    fn place(
        path: PathBuf,
        label: impl Into<SharedString>,
        icon: Icon,
        section: PlaceSection,
    ) -> Self {
        Self {
            path,
            label: label.into(),
            icon,
            section,
            eject: false,
        }
    }
}

/// How many cold path handles one provider keeps before reclaiming the least
/// recently used ones. Roots, places, the last listing and any location passed
/// to [`FileProvider::retain`] are pinned and never evicted.
pub const INTERN_CAPACITY: usize = 65_536;

#[derive(Default)]
struct FsState {
    paths: HashMap<LocationId, PathBuf>,
    ids: HashMap<PathBuf, LocationId>,
    used: HashMap<LocationId, u64>,
    intrinsic: std::collections::HashSet<LocationId>,
    retained: std::collections::HashSet<LocationId>,
    /// Entries of the most recently produced listing, kept alive while a view
    /// can still select or resolve them.
    listing: std::collections::HashSet<LocationId>,
    clock: u64,
    next: u64,
    capacity: usize,
}

impl FsState {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            ..Self::default()
        }
    }

    fn intern(&mut self, path: PathBuf) -> LocationId {
        if let Some(id) = self.ids.get(&path) {
            let id = id.clone();
            self.touch(&id);
            return id;
        }
        self.next = self.next.saturating_add(1);
        let id: LocationId = format!("fs-location-{:08}", self.next).into();
        self.paths.insert(id.clone(), path.clone());
        self.ids.insert(path, id.clone());
        self.touch(&id);
        self.evict();
        id
    }

    /// A path handle pinned for the provider's lifetime: home, roots and
    /// discovered places.
    fn intern_intrinsic(&mut self, path: PathBuf) -> LocationId {
        let id = self.intern(path);
        self.intrinsic.insert(id.clone());
        id
    }

    fn touch(&mut self, id: &LocationId) {
        self.clock = self.clock.saturating_add(1);
        self.used.insert(id.clone(), self.clock);
    }

    fn path(&mut self, location: &LocationId) -> Option<PathBuf> {
        let path = self.paths.get(location).cloned()?;
        self.touch(location);
        Some(path)
    }

    fn set_retained(&mut self, locations: &[LocationId]) {
        self.retained.clear();
        for location in locations {
            if self.paths.contains_key(location) {
                self.retained.insert(location.clone());
                self.touch(location);
            }
        }
    }

    fn pin_place(&mut self, id: LocationId) {
        self.intrinsic.insert(id);
    }

    fn interned_count(&self) -> usize {
        self.paths.len()
    }

    fn is_pinned(&self, id: &LocationId) -> bool {
        self.intrinsic.contains(id) || self.retained.contains(id) || self.listing.contains(id)
    }

    fn evict(&mut self) {
        if self.paths.len() <= self.capacity {
            return;
        }
        let mut cold: Vec<(u64, LocationId)> = self
            .paths
            .keys()
            .filter(|id| !self.is_pinned(id))
            .map(|id| (self.used.get(id).copied().unwrap_or(0), id.clone()))
            .collect();
        cold.sort_by_key(|(used, _)| *used);
        let over = self.paths.len() - self.capacity;
        for (_, id) in cold.into_iter().take(over) {
            self.remove(&id);
        }
    }

    fn remove(&mut self, id: &LocationId) {
        if let Some(path) = self.paths.remove(id) {
            self.ids.remove(&path);
        }
        self.used.remove(id);
        self.intrinsic.remove(id);
        self.retained.remove(id);
        self.listing.remove(id);
    }

    /// Preserve `old`'s handle when the entry moves to `new`, including every
    /// already-interned descendant of a renamed directory. The exact host path
    /// changes; the identity the panel selected does not.
    fn rename_path(&mut self, old: &Path, new: &Path) -> Option<LocationId> {
        let id = match self.ids.remove(old) {
            Some(id) => id,
            None => return Some(self.intern(new.to_path_buf())),
        };
        self.paths.insert(id.clone(), new.to_path_buf());
        self.ids.insert(new.to_path_buf(), id.clone());
        let descendants: Vec<(LocationId, PathBuf)> = self
            .paths
            .iter()
            .filter(|(other, path)| *other != &id && path.starts_with(old))
            .map(|(other, path)| (other.clone(), path.clone()))
            .collect();
        for (descendant, path) in descendants {
            let Ok(relative) = path.strip_prefix(old) else {
                continue;
            };
            let moved = new.join(relative);
            self.ids.remove(&path);
            self.paths.insert(descendant.clone(), moved.clone());
            self.ids.insert(moved, descendant);
        }
        self.touch(&id);
        Some(id)
    }
}

/// The local disk, behind feature `std-fs`, so the core library keeps no
/// filesystem dependency.
///
/// A provider instance owns its location handles. Keep one instance for the
/// lifetime of the application (or at least for a panel and the consumer code
/// that acts on its outcome); use [`resolve`](FileProvider::resolve) when a
/// durable `PathBuf` is needed.
#[derive(Clone)]
pub struct StdFsProvider {
    state: Arc<Mutex<FsState>>,
    home: Option<PathBuf>,
}

impl std::fmt::Debug for StdFsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StdFsProvider")
            .field("home", &self.home)
            .finish_non_exhaustive()
    }
}

impl Default for StdFsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl StdFsProvider {
    #[must_use]
    pub fn new() -> Self {
        Self::with_intern_capacity(INTERN_CAPACITY)
    }

    /// A provider with a specific cold-handle budget. Tests use a small budget
    /// to prove the maps stay bounded; production uses
    /// [`INTERN_CAPACITY`].
    #[must_use]
    pub fn with_intern_capacity(capacity: usize) -> Self {
        let home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    std::env::current_dir()
                        .map(|cwd| cwd.join(&path))
                        .unwrap_or(path)
                }
            });
        let provider = Self {
            state: Arc::new(Mutex::new(FsState::with_capacity(capacity))),
            home,
        };
        if let Some(home) = provider.home.clone() {
            provider.intern_intrinsic(&home);
        }
        provider
    }

    /// The user's home directory as a location, when the host has one.
    #[must_use]
    pub fn home(&self) -> Option<LocationId> {
        self.home.as_ref().map(|path| self.intern(path))
    }

    /// The filesystem root of the home directory, or `/` when no home exists.
    #[must_use]
    pub fn root(&self) -> LocationId {
        let root = self
            .home
            .as_ref()
            .and_then(|home| home.ancestors().last())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/"));
        self.intern(&root)
    }

    /// The location handle for a host path. The path is used exactly as given
    /// (made absolute if necessary), never round-tripped through display text.
    #[must_use]
    pub fn location_for(&self, path: impl AsRef<Path>) -> Option<LocationId> {
        self.absolute(path.as_ref()).map(|path| self.intern(&path))
    }

    fn intern(&self, path: &Path) -> LocationId {
        self.state().intern(path.to_path_buf())
    }

    fn intern_intrinsic(&self, path: &Path) -> LocationId {
        self.state().intern_intrinsic(path.to_path_buf())
    }

    fn state(&self) -> MutexGuard<'_, FsState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// How many path handles the provider currently holds. Bounded by
    /// [`INTERN_CAPACITY`] plus the pinned roots, places, listing and retained
    /// history.
    #[must_use]
    pub fn interned_count(&self) -> usize {
        self.state().interned_count()
    }

    fn absolute(&self, path: &Path) -> Option<PathBuf> {
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            std::env::current_dir().ok().map(|cwd| cwd.join(path))
        }
    }

    fn path_of(&self, location: &LocationId) -> Option<PathBuf> {
        let mut state = self.state();
        state.path(location)
    }

    fn label_for(path: &Path) -> SharedString {
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| path.display().to_string())
            .into()
    }

    fn location_icon_for(&self, path: &Path) -> Icon {
        if self.home.as_deref() == Some(path) {
            return Icon::Home;
        }
        match path.file_name().and_then(|name| name.to_str()) {
            Some("Desktop") => Icon::Desktop,
            Some("Documents" | "Downloads" | "Pictures" | "Music" | "Videos") => Icon::Documents,
            Some("Applications") => Icon::Application,
            _ if path.parent().is_none() => Icon::Volume,
            _ => Icon::Folder,
        }
    }
}

impl FileProvider for StdFsProvider {
    fn children(
        &self,
        location: &LocationId,
        cx: &mut App,
    ) -> Task<Result<FileListing, FileError>> {
        let Some(path) = self.path_of(location) else {
            return Task::ready(Err(FileError::new(
                "This location does not belong to this provider",
            )));
        };
        let state = self.state.clone();
        cx.background_executor()
            .spawn(async move { list_directory(&path, &state) })
    }

    fn metadata_enrichment(
        &self,
        entries: &[FileEntry],
        cx: &mut App,
    ) -> Option<Task<Result<MetadataUpdates, FileError>>> {
        // Cheap listings mark ordinary entries unknown. Symlink targets were
        // resolved while listing because navigation depends on them.
        if entries
            .iter()
            .all(|entry| entry.metadata_known && entry.is_link)
        {
            return None;
        }
        let targets: Vec<(ItemId, PathBuf)> = entries
            .iter()
            .filter_map(|entry| {
                self.path_of(&entry.location)
                    .map(|path| (entry.id.clone(), path))
            })
            .collect();
        Some(cx.background_executor().spawn(async move {
            let mut out = Vec::with_capacity(targets.len());
            for (id, path) in targets {
                let name = path
                    .file_name()
                    .map_or_else(std::ffi::OsString::new, std::ffi::OsStr::to_os_string);
                out.push((id, read_metadata(&path, &name)));
            }
            Ok(out)
        }))
    }

    fn ancestors(&self, location: &LocationId, _: &App) -> Vec<FileAncestor> {
        let Some(path) = self.path_of(location) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut current = path.parent();
        while let Some(parent) = current {
            out.push(FileAncestor {
                id: self.intern(parent),
                label: Self::label_for(parent),
                icon: self.location_icon_for(parent),
            });
            current = parent.parent();
        }
        out
    }

    fn places(&self, cx: &mut App) -> Task<Result<Vec<FilePlace>, FileError>> {
        let state = self.state.clone();
        let home = self.home.clone();
        cx.background_executor().spawn(async move {
            let discovered = platform::host_places(home.as_deref());
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let places = discovered
                .into_iter()
                .map(|place| {
                    let id = state.intern(place.path);
                    state.pin_place(id.clone());
                    FilePlace {
                        id,
                        label: place.label,
                        icon: place.icon,
                        section: place.section,
                        eject: place.eject,
                    }
                })
                .collect();
            Ok(places)
        })
    }

    fn create_folder(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<LocationId, FileError>> {
        if let Err(error) = self.validate_name(parent, name, cx) {
            return Task::ready(Err(error));
        }
        let Some(parent_path) = self.path_of(parent) else {
            return Task::ready(Err(FileError::new("No such location")));
        };
        let path = parent_path.join(name);
        let state = self.state.clone();
        cx.background_executor().spawn(async move {
            std::fs::create_dir(&path)
                .map_err(|error| FileError::new(format!("{}: {error}", path.display())))?;
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            Ok(state.intern(path))
        })
    }

    fn exists(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<bool, FileError>> {
        if let Err(error) = self.validate_name(parent, name, cx) {
            return Task::ready(Err(error));
        }
        let Some(parent_path) = self.path_of(parent) else {
            return Task::ready(Err(FileError::new("No such location")));
        };
        let path = parent_path.join(name);
        cx.background_executor().spawn(async move {
            path.try_exists()
                .map_err(|error| FileError::new(format!("{}: {error}", path.display())))
        })
    }

    fn validate_name(&self, _parent: &LocationId, name: &str, _: &App) -> Result<(), FileError> {
        validate_structural_name(name)?;
        platform::validate_name(name)
    }

    fn child_location(&self, parent: &LocationId, name: &str) -> Option<LocationId> {
        if validate_structural_name(name).is_err() {
            return None;
        }
        let parent = self.path_of(parent)?;
        Some(self.intern(&parent.join(name)))
    }

    fn rename(
        &self,
        parent: &LocationId,
        id: &ItemId,
        new_name: &str,
        cx: &App,
    ) -> Task<Result<(), FileError>> {
        if let Err(error) = self.validate_name(parent, new_name, cx) {
            return Task::ready(Err(error));
        }
        let Some(parent_path) = self.path_of(parent) else {
            return Task::ready(Err(FileError::new("No such location")));
        };
        let Some(old_path) = self.path_of(id) else {
            return Task::ready(Err(FileError::new("No such entry")));
        };
        let new_path = parent_path.join(new_name);
        if old_path == new_path {
            return Task::ready(Ok(()));
        }
        // On a case-insensitive host, renaming `Report.txt` to `report.txt`
        // legitimately "collides" with the entry itself. Only a differently
        // spelled existing target is a collision.
        let case_only = case_only_rename(&old_path, &new_path);
        let state = self.state.clone();
        cx.background_executor().spawn(async move {
            match new_path.try_exists() {
                Ok(true) if !case_only => {
                    return Err(FileError::new(format!(
                        "\u{201C}{}\u{201D} already exists",
                        new_path.display()
                    )));
                }
                Ok(_) => {}
                Err(error) => {
                    return Err(FileError::new(format!("{}: {error}", new_path.display())));
                }
            }
            std::fs::rename(&old_path, &new_path)
                .map_err(|error| FileError::new(format!("{}: {error}", new_path.display())))?;
            // Preserve the handle the panel selected. External changes can
            // still appear as remove/add, but a Silvestris rename keeps the
            // identity of the file (and of its interned descendants).
            let mut state = state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.rename_path(&old_path, &new_path);
            Ok(())
        })
    }

    fn trash(&self, parent: &LocationId, id: &ItemId, cx: &mut App) -> Task<Result<(), FileError>> {
        let Some(_parent_path) = self.path_of(parent) else {
            return Task::ready(Err(FileError::new("No such location")));
        };
        let Some(path) = self.path_of(id) else {
            return Task::ready(Err(FileError::new("No such entry")));
        };
        let home = self.home.clone();
        cx.background_executor().spawn(async move {
            platform::move_to_trash(&path, home.as_deref())
                .map_err(|error| FileError::new(format!("{}: {error}", path.display())))
        })
    }

    fn capabilities(&self) -> FileProviderCapabilities {
        FileProviderCapabilities {
            create_folder: true,
            rename: true,
            copy_path: true,
            trash: true,
            delete_permanently: false,
            // Unmounting a volume is a platform capability this adapter does
            // not own yet, so it must not advertise the eject glyph.
            eject: false,
        }
    }

    fn retain(&self, locations: &[LocationId], _: &mut App) {
        self.state().set_retained(locations);
    }

    fn resolve(&self, location: &LocationId) -> Option<PathBuf> {
        self.path_of(location)
    }

    fn location_of(&self, path: &Path) -> Option<LocationId> {
        self.location_for(path)
    }

    fn label(&self, location: &LocationId, _: &App) -> SharedString {
        self.path_of(location)
            .map_or_else(|| location.clone(), |path| Self::label_for(&path))
    }

    fn location_icon(&self, location: &LocationId) -> Icon {
        self.path_of(location)
            .map_or(Icon::Folder, |path| self.location_icon_for(&path))
    }
}

fn list_directory(path: &Path, state: &Arc<Mutex<FsState>>) -> Result<FileListing, FileError> {
    let read = std::fs::read_dir(path)
        .map_err(|error| FileError::new(format!("{}: {error}", path.display())))?;
    let mut rows = Vec::new();
    let mut warnings = Vec::new();
    for item in read {
        let item = match item {
            Ok(item) => item,
            Err(error) => {
                warnings.push(FileError::new(format!("{}: {error}", path.display())));
                continue;
            }
        };
        let child = item.path();
        let name_os = item.file_name();
        let name = name_os.to_string_lossy().into_owned();
        let file_type = match item.file_type() {
            Ok(file_type) => Some(file_type),
            Err(error) => {
                warnings.push(FileError::new(format!("{}: {error}", child.display())));
                None
            }
        };
        let symlink = file_type
            .as_ref()
            .is_some_and(|file_type| file_type.is_symlink());
        // Names, kinds and navigation-critical flags arrive first. A symlink
        // still resolves its target here: whether it is a directory decides
        // navigation, and a broken link is a warning, not a deferred detail.
        let metadata = if symlink {
            match std::fs::metadata(&child) {
                Ok(target) => FileMetadata {
                    kind: FileKind::Folder,
                    is_dir: target.is_dir(),
                    is_package: false,
                    is_link: true,
                    metadata_known: true,
                    size: target.len(),
                    modified: target.modified().map_or(0, modified_seconds),
                    hidden: hidden_name(&name_os, Some(&target)),
                    icon: Icon::Folder,
                },
                Err(error) => {
                    warnings.push(FileError::new(format!("{}: {error}", child.display())));
                    FileMetadata {
                        kind: FileKind::Alias,
                        is_dir: false,
                        is_package: false,
                        is_link: true,
                        metadata_known: false,
                        size: 0,
                        modified: 0,
                        hidden: hidden_name(&name_os, None),
                        icon: FileKind::Alias.icon(),
                    }
                }
            }
        } else {
            let is_dir = file_type
                .as_ref()
                .is_some_and(|file_type| file_type.is_dir());
            cheap_metadata(&name_os, is_dir, false, false)
        };
        let metadata = metadata.with_classification(&name_os);
        let name_key = name.to_lowercase();
        rows.push((
            child,
            FileEntry {
                id: SharedString::default(),
                location: LocationId::default(),
                name: name.into(),
                name_key: name_key.into(),
                kind: metadata.kind,
                is_dir: metadata.is_dir,
                is_package: metadata.is_package,
                is_link: metadata.is_link,
                metadata_known: metadata.metadata_known,
                size: metadata.size,
                modified: metadata.modified,
                hidden: metadata.hidden,
                icon: metadata.icon,
            },
        ));
    }
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.listing.clear();
    let mut entries: Vec<FileEntry> = rows
        .into_iter()
        .map(|(child, mut entry)| {
            let location = state.intern(child);
            state.listing.insert(location.clone());
            entry.id = location.clone();
            entry.location = location;
            entry
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name_key.cmp(&b.name_key))
    });
    Ok(FileListing { entries, warnings })
}

/// Metadata that needs only the directory entry's type and name.
fn cheap_metadata(
    name: &std::ffi::OsStr,
    is_dir: bool,
    is_link: bool,
    known: bool,
) -> FileMetadata {
    let classification = classify(name, is_dir, is_link, known);
    FileMetadata {
        kind: classification.kind,
        is_dir,
        is_package: classification.package,
        is_link,
        metadata_known: known,
        size: 0,
        modified: 0,
        hidden: hidden_name(name, None),
        icon: classification.kind.icon(),
    }
}

impl FileMetadata {
    /// Fill kind, package flag and icon from the entry name and target type.
    fn with_classification(mut self, name: &std::ffi::OsStr) -> Self {
        if self.is_dir {
            self.size = 0;
        }
        let classification = classify(name, self.is_dir, self.is_link, self.metadata_known);
        self.kind = classification.kind;
        self.is_package = classification.package;
        self.icon = self.kind.icon();
        self
    }
}

struct Classification {
    kind: FileKind,
    package: bool,
}

fn classify(
    name: &std::ffi::OsStr,
    is_dir: bool,
    is_link: bool,
    metadata_known: bool,
) -> Classification {
    let extension = Path::new(name)
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let package = is_dir && matches!(extension.as_str(), "app" | "bundle" | "pkg");
    let kind = if package {
        FileKind::Application
    } else if is_dir {
        FileKind::Folder
    } else if is_link && !metadata_known {
        FileKind::Alias
    } else if matches!(extension.as_str(), "app" | "exe") {
        FileKind::Application
    } else if extension == "lnk" {
        FileKind::Alias
    } else {
        FileKind::Document
    };
    Classification { kind, package }
}

fn read_metadata(path: &Path, name: &std::ffi::OsStr) -> FileMetadata {
    let link_metadata = std::fs::symlink_metadata(path);
    let is_link = link_metadata
        .as_ref()
        .ok()
        .is_some_and(|metadata| metadata.file_type().is_symlink());
    let metadata = if is_link {
        std::fs::metadata(path).ok()
    } else {
        link_metadata.ok()
    };
    let known = metadata.is_some();
    let is_dir = metadata.as_ref().is_some_and(std::fs::Metadata::is_dir);
    let mut out = cheap_metadata(name, is_dir, is_link, known);
    if is_dir {
        out.size = 0;
    } else {
        out.size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
    }
    out.modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .map_or(0, modified_seconds);
    out.hidden = hidden_name(name, metadata.as_ref());
    out.with_classification(name)
}

/// Whether `old` and `new` name the same entry with different casing. Only a
/// case-insensitive host can answer yes; elsewhere both names can exist.
fn case_only_rename(old: &Path, new: &Path) -> bool {
    #[cfg(any(windows, target_os = "macos"))]
    {
        old != new
            && old
                .to_string_lossy()
                .to_lowercase()
                .eq(&new.to_string_lossy().to_lowercase())
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (old, new);
        false
    }
}

fn hidden_name(name: &std::ffi::OsStr, metadata: Option<&std::fs::Metadata>) -> bool {
    if name.to_string_lossy().starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if metadata.is_some_and(|metadata| metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0)
        {
            return true;
        }
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
    }
    false
}
