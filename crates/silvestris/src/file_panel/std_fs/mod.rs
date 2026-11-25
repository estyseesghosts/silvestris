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
    FileListing, FilePlace, FileProvider, FileProviderCapabilities, PlaceSection,
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

    fn eject(mut self, eject: bool) -> Self {
        self.eject = eject;
        self
    }
}

#[derive(Default)]
struct FsState {
    paths: HashMap<LocationId, PathBuf>,
    ids: HashMap<PathBuf, LocationId>,
    next: u64,
}

impl FsState {
    fn intern(&mut self, path: PathBuf) -> LocationId {
        if let Some(id) = self.ids.get(&path) {
            return id.clone();
        }
        self.next = self.next.saturating_add(1);
        let id: LocationId = format!("fs-location-{:08}", self.next).into();
        self.paths.insert(id.clone(), path.clone());
        self.ids.insert(path, id.clone());
        id
    }

    fn path(&self, location: &LocationId) -> Option<PathBuf> {
        self.paths.get(location).cloned()
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
            state: Arc::new(Mutex::new(FsState::default())),
            home,
        };
        if let Some(home) = provider.home.clone() {
            provider.intern(&home);
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

    fn state(&self) -> MutexGuard<'_, FsState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn absolute(&self, path: &Path) -> Option<PathBuf> {
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            std::env::current_dir().ok().map(|cwd| cwd.join(path))
        }
    }

    fn path_of(&self, location: &LocationId) -> Option<PathBuf> {
        self.state().path(location)
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
                .map(|place| FilePlace {
                    id: state.intern(place.path),
                    label: place.label,
                    icon: place.icon,
                    section: place.section,
                    eject: place.eject,
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
        cx.background_executor().spawn(async move {
            match new_path.try_exists() {
                Ok(true) => {
                    return Err(FileError::new(format!(
                        "\u{201C}{}\u{201D} already exists",
                        new_path.display()
                    )));
                }
                Ok(false) => {}
                Err(error) => {
                    return Err(FileError::new(format!("{}: {error}", new_path.display())));
                }
            }
            std::fs::rename(&old_path, &new_path)
                .map_err(|error| FileError::new(format!("{}: {error}", new_path.display())))
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
        }
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
        let mut is_dir = file_type
            .as_ref()
            .is_some_and(|file_type| file_type.is_dir());
        let mut metadata = item.metadata();
        if symlink {
            match std::fs::metadata(&child) {
                Ok(target) => {
                    is_dir = target.is_dir();
                    metadata = Ok(target);
                }
                Err(error) => {
                    warnings.push(FileError::new(format!("{}: {error}", child.display())));
                    metadata = Err(error);
                }
            }
        }
        let metadata_known = metadata.is_ok();
        let modified = match &metadata {
            Ok(metadata) => match metadata.modified() {
                Ok(time) => modified_seconds(time),
                Err(error) => {
                    warnings.push(FileError::new(format!("{}: {error}", child.display())));
                    0
                }
            },
            Err(_) => 0,
        };
        let extension = child
            .extension()
            .map(|extension| extension.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let package = is_dir && matches!(extension.as_str(), "app" | "bundle" | "pkg");
        let kind = if package {
            FileKind::Application
        } else if is_dir {
            FileKind::Folder
        } else if symlink && !metadata_known {
            FileKind::Alias
        } else if matches!(extension.as_str(), "app" | "exe") {
            FileKind::Application
        } else if extension == "lnk" {
            FileKind::Alias
        } else {
            FileKind::Document
        };
        let hidden = hidden_name(&name_os, metadata.as_ref().ok());
        let size = if is_dir {
            0
        } else {
            metadata.as_ref().map_or(0, std::fs::Metadata::len)
        };
        rows.push((
            child,
            FileEntry {
                id: SharedString::default(),
                location: LocationId::default(),
                name: name.into(),
                kind,
                is_dir,
                is_package: package,
                is_link: symlink,
                metadata_known,
                size,
                modified,
                hidden,
                icon: kind.icon(),
            },
        ));
    }
    let mut state = state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut entries: Vec<FileEntry> = rows
        .into_iter()
        .map(|(child, mut entry)| {
            let location = state.intern(child);
            entry.id = location.clone();
            entry.location = location;
            entry
        })
        .collect();
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(FileListing { entries, warnings })
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
