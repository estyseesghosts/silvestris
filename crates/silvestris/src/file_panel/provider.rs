//! FileProvider (#29, core of T6): everything the panels know about files.
//!
//! The panels never touch `std::fs`. A provider answers with stable location
//! ids and `FileEntry` values; `MemoryProvider` serves tests and the gallery,
//! `StdFsProvider` (feature `std-fs`) is the local-disk adapter and does its
//! I/O on GPUI's background executor.
//!
//! `FileEntry` carries the metadata the Open and Save panels display:
//! identity, name, kind, directory/package flags, size, modification time,
//! hidden state and a semantic icon. It does not carry an `AnyElement`, so a
//! provider is plain data and can be used from a background task.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{App, SharedString, Task};

use crate::binding::ItemId;
use crate::controls::path_control::LocationId;
use crate::icons::Icon;

/// A provider failure, shown in the panel's status area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError {
    pub message: SharedString,
}

impl FileError {
    pub fn new(message: impl Into<SharedString>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for FileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// What a row is, for the Kind column and the icon default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileKind {
    Folder,
    Document,
    Application,
    Volume,
    Alias,
    #[default]
    Other,
}

impl FileKind {
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            FileKind::Folder => "Folder",
            FileKind::Document => "Document",
            FileKind::Application => "Application",
            FileKind::Volume => "Volume",
            FileKind::Alias => "Alias",
            FileKind::Other => "File",
        }
    }

    #[must_use]
    pub fn icon(self) -> Icon {
        match self {
            FileKind::Folder => Icon::Folder,
            FileKind::Document => Icon::Document,
            FileKind::Application => Icon::Application,
            FileKind::Volume => Icon::Volume,
            FileKind::Alias => Icon::Document,
            FileKind::Other => Icon::Document,
        }
    }
}

/// One entry as the provider reports it. `id` is stable within its location.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub id: ItemId,
    pub name: SharedString,
    pub kind: FileKind,
    pub is_dir: bool,
    /// A directory the Finder treats as a single document (`.app`, `.bundle`).
    pub is_package: bool,
    /// Bytes; zero for directories and unknown sizes.
    pub size: u64,
    /// Seconds since the Unix epoch; 0 means unknown.
    pub modified: u64,
    pub hidden: bool,
    pub icon: Icon,
}

impl FileEntry {
    pub fn new(
        id: impl Into<ItemId>,
        name: impl Into<SharedString>,
        kind: FileKind,
        is_dir: bool,
    ) -> Self {
        let name = name.into();
        Self {
            id: id.into(),
            name,
            kind,
            is_dir,
            is_package: false,
            size: 0,
            modified: 0,
            hidden: false,
            icon: kind.icon(),
        }
    }

    pub fn folder(id: impl Into<ItemId>, name: impl Into<SharedString>) -> Self {
        Self::new(id, name, FileKind::Folder, true)
    }

    pub fn document(id: impl Into<ItemId>, name: impl Into<SharedString>) -> Self {
        Self::new(id, name, FileKind::Document, false)
    }

    pub fn package(mut self, package: bool) -> Self {
        self.is_package = package;
        self
    }

    pub fn size(mut self, size: u64) -> Self {
        self.size = size;
        self
    }

    pub fn modified(mut self, seconds: u64) -> Self {
        self.modified = seconds;
        self
    }

    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    /// The extension without the dot, lowercased; empty when there is none.
    #[must_use]
    pub fn extension(&self) -> String {
        self.name
            .rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map(|(_, ext)| ext.to_lowercase())
            .unwrap_or_default()
    }

    /// Finder-style size text. The rounding is our own; 10.6 shows
    /// "Zero bytes" for an empty file and whole KB/MB otherwise.
    #[must_use]
    pub fn size_label(&self) -> SharedString {
        if self.is_dir {
            return "--".into();
        }
        if self.size == 0 {
            return "Zero bytes".into();
        }
        if self.size < 1024 {
            return format!("{} bytes", self.size).into();
        }
        let units = ["KB", "MB", "GB", "TB"];
        let mut value = self.size as f64 / 1024.0;
        let mut unit = 0;
        while value >= 1024.0 && unit + 1 < units.len() {
            value /= 1024.0;
            unit += 1;
        }
        if value < 10.0 {
            format!("{value:.1} {}", units[unit]).into()
        } else {
            format!("{:.0} {}", value.round(), units[unit]).into()
        }
    }

    /// UTC `YYYY-MM-DD HH:MM`. The model stores epoch seconds; the panel does
    /// not depend on a local-time crate.
    #[must_use]
    pub fn modified_label(&self) -> SharedString {
        if self.modified == 0 {
            return "--".into();
        }
        let (year, month, day, hour, minute) = civil(self.modified);
        format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}").into()
    }

    /// Hidden names are dotfiles even when a provider did not flag them.
    #[must_use]
    pub fn is_hidden(&self) -> bool {
        self.hidden || self.name.starts_with('.')
    }

    /// Whether the entry matches an allowed-extension list. Directories always
    /// match; an empty list accepts everything.
    #[must_use]
    pub fn matches_types(&self, types: &[String]) -> bool {
        if self.is_dir || types.is_empty() {
            return true;
        }
        let ext = self.extension();
        types
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(&ext))
    }
}

/// Epoch seconds to a UTC civil date and time.
fn civil(epoch: u64) -> (i64, u32, u32, u32, u32) {
    let seconds = (epoch % 86_400) as u32;
    let days = (epoch / 86_400) as i64;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, seconds / 3600, (seconds % 3600) / 60)
}

/// Where a sidebar place belongs. Search entries are not file locations but
/// the panel lists them in the same control; a provider can answer them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaceSection {
    #[default]
    Places,
    Devices,
    SearchFor,
}

/// A sidebar entry supplied by the provider: a location id, a label and an
/// icon. The panel never builds these from path strings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePlace {
    pub id: LocationId,
    pub label: SharedString,
    pub icon: Icon,
    pub section: PlaceSection,
    /// A removable device shows the eject accessory.
    pub eject: bool,
}

impl FilePlace {
    pub fn new(
        id: impl Into<LocationId>,
        label: impl Into<SharedString>,
        icon: Icon,
        section: PlaceSection,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            section,
            eject: false,
        }
    }

    pub fn eject(mut self, eject: bool) -> Self {
        self.eject = eject;
        self
    }
}

/// One ancestor of a location, from the current directory upward.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAncestor {
    pub id: LocationId,
    pub label: SharedString,
    pub icon: Icon,
}

/// The application's file access. Every method is asynchronous when it can
/// block; `children` and `create_folder` never run on the UI thread for
/// `StdFsProvider`.
pub trait FileProvider: 'static {
    /// List `location`'s visible entries. The model applies the hidden and
    /// allowed-type policy.
    fn children(
        &self,
        location: &LocationId,
        cx: &mut App,
    ) -> Task<Result<Vec<FileEntry>, FileError>>;

    /// The ancestors of `location`, nearest parent first. The current
    /// location itself is not included.
    fn ancestors(&self, location: &LocationId, cx: &App) -> Vec<FileAncestor>;

    /// DEVICES, PLACES and SEARCH FOR entries.
    fn places(&self, cx: &App) -> Vec<FilePlace>;

    /// Create `name` under `parent` and return the new location id.
    fn create_folder(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<LocationId, FileError>>;

    /// Whether `name` already exists under `parent`.
    fn exists(&self, parent: &LocationId, name: &str, cx: &mut App) -> Task<bool>;

    /// Rename an entry in place. The panel's New Folder flow starts an inline
    /// edit; providers that cannot rename return the default error.
    fn rename(
        &self,
        _parent: &LocationId,
        _id: &ItemId,
        _new_name: &str,
        _cx: &App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new("This provider cannot rename entries")))
    }

    /// Delete an entry. Providers that cannot delete return the default error.
    fn delete(
        &self,
        _parent: &LocationId,
        _id: &ItemId,
        _cx: &mut App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new("This provider cannot delete entries")))
    }

    /// A host path for a location, when the provider has one. The panel never
    /// needs it; a consumer that writes the chosen file does.
    fn resolve(&self, _location: &LocationId) -> Option<PathBuf> {
        None
    }

    /// The location id for a host path, for a "go to folder" feature.
    fn location_of(&self, _path: &std::path::Path) -> Option<LocationId> {
        None
    }

    /// The label shown in the panel's path pop-up and breadcrumb.
    fn label(&self, location: &LocationId, cx: &App) -> SharedString {
        self.ancestors(location, cx)
            .into_iter()
            .next()
            .map_or_else(|| location.clone(), |ancestor| ancestor.label)
    }

    /// The icon for a location. The default is a folder; a provider can
    /// report a volume, home or application location.
    fn location_icon(&self, _location: &LocationId) -> Icon {
        Icon::Folder
    }
}

/// Provider-side owned data; the trait methods use `&self`.
#[derive(Default)]
struct MemoryData {
    locations: std::collections::BTreeMap<LocationId, MemoryLocation>,
    places: Vec<FilePlace>,
}

struct MemoryLocation {
    parent: Option<LocationId>,
    label: SharedString,
    icon: Icon,
    entries: Vec<FileEntry>,
}

/// The provider's own parent/name location id, matching
/// `FileBrowserModel::child_location`.
fn join_location(parent: &LocationId, name: &str) -> LocationId {
    if parent.ends_with('/') || parent.ends_with('\\') {
        format!("{parent}{name}").into()
    } else {
        format!("{parent}/{name}").into()
    }
}

/// Deterministic provider for tests, the gallery and the capture samples.
#[derive(Clone, Default)]
pub struct MemoryProvider {
    data: Rc<RefCell<MemoryData>>,
}

impl MemoryProvider {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add or replace a location. `parent` makes it reachable from the path
    /// pop-up and from `ancestors`.
    pub fn add_location(
        &self,
        id: impl Into<LocationId>,
        label: impl Into<SharedString>,
        icon: Icon,
        parent: Option<LocationId>,
        entries: Vec<FileEntry>,
    ) {
        self.data.borrow_mut().locations.insert(
            id.into(),
            MemoryLocation {
                parent,
                label: label.into(),
                icon,
                entries,
            },
        );
    }

    pub fn set_places(&self, places: Vec<FilePlace>) {
        self.data.borrow_mut().places = places;
    }

    /// Append one entry to a location without replacing the rest.
    pub fn push_entry(&self, location: &LocationId, entry: FileEntry) {
        if let Some(location) = self.data.borrow_mut().locations.get_mut(location) {
            location.entries.push(entry);
        }
    }

    /// Replace a location's entries, preserving its label and parent.
    pub fn set_entries(&self, location: &LocationId, entries: Vec<FileEntry>) {
        if let Some(found) = self.data.borrow_mut().locations.get_mut(location) {
            found.entries = entries;
        }
    }

    /// The entries a location currently holds, for test assertions.
    #[must_use]
    pub fn entries(&self, location: &LocationId) -> Vec<FileEntry> {
        self.data
            .borrow()
            .locations
            .get(location)
            .map(|found| found.entries.clone())
            .unwrap_or_default()
    }

    fn has_name(&self, parent: &LocationId, name: &str) -> bool {
        self.data
            .borrow()
            .locations
            .get(parent)
            .is_some_and(|found| found.entries.iter().any(|entry| entry.name == name))
    }
}

impl FileProvider for MemoryProvider {
    fn children(
        &self,
        location: &LocationId,
        _: &mut App,
    ) -> Task<Result<Vec<FileEntry>, FileError>> {
        let result = self
            .data
            .borrow()
            .locations
            .get(location)
            .map(|found| found.entries.clone())
            .ok_or_else(|| FileError::new(format!("No such location: {location}")));
        Task::ready(result)
    }

    fn ancestors(&self, location: &LocationId, _: &App) -> Vec<FileAncestor> {
        let data = self.data.borrow();
        let mut out = Vec::new();
        let mut current = data
            .locations
            .get(location)
            .and_then(|found| found.parent.clone());
        while let Some(id) = current {
            let Some(found) = data.locations.get(&id) else {
                break;
            };
            out.push(FileAncestor {
                id: id.clone(),
                label: found.label.clone(),
                icon: found.icon,
            });
            current = found.parent.clone();
        }
        out
    }

    fn places(&self, _: &App) -> Vec<FilePlace> {
        self.data.borrow().places.clone()
    }

    fn label(&self, location: &LocationId, _: &App) -> SharedString {
        self.data
            .borrow()
            .locations
            .get(location)
            .map_or_else(|| location.clone(), |found| found.label.clone())
    }

    fn location_icon(&self, location: &LocationId) -> Icon {
        self.data
            .borrow()
            .locations
            .get(location)
            .map_or(Icon::Folder, |found| found.icon)
    }

    fn create_folder(
        &self,
        parent: &LocationId,
        name: &str,
        _: &mut App,
    ) -> Task<Result<LocationId, FileError>> {
        if name.trim().is_empty() || name.contains('/') || name.contains('\\') {
            return Task::ready(Err(FileError::new("Invalid folder name")));
        }
        if self.has_name(parent, name) {
            return Task::ready(Err(FileError::new(format!(
                "\u{201C}{name}\u{201D} already exists"
            ))));
        }
        let id = join_location(parent, name);
        {
            let mut data = self.data.borrow_mut();
            let Some(location) = data.locations.get_mut(parent) else {
                return Task::ready(Err(FileError::new("No such location")));
            };
            location
                .entries
                .push(FileEntry::folder(name.to_string(), name.to_string()));
            data.locations.insert(
                id.clone(),
                MemoryLocation {
                    parent: Some(parent.clone()),
                    label: name.to_string().into(),
                    icon: Icon::Folder,
                    entries: Vec::new(),
                },
            );
        }
        Task::ready(Ok(id))
    }

    fn exists(&self, parent: &LocationId, name: &str, _: &mut App) -> Task<bool> {
        Task::ready(self.has_name(parent, name))
    }

    fn rename(
        &self,
        parent: &LocationId,
        id: &ItemId,
        new_name: &str,
        _: &App,
    ) -> Task<Result<(), FileError>> {
        if new_name.trim().is_empty() || new_name.contains('/') || new_name.contains('\\') {
            return Task::ready(Err(FileError::new("Invalid file name")));
        }
        let mut data = self.data.borrow_mut();
        let mut old_names = Vec::new();
        {
            let Some(location) = data.locations.get_mut(parent) else {
                return Task::ready(Err(FileError::new("No such location")));
            };
            if location
                .entries
                .iter()
                .any(|entry| entry.name == new_name && entry.id != *id)
            {
                return Task::ready(Err(FileError::new(format!(
                    "\u{201C}{new_name}\u{201D} already exists"
                ))));
            }
            let Some(entry) = location.entries.iter_mut().find(|entry| entry.id == *id) else {
                return Task::ready(Err(FileError::new("No such entry")));
            };
            let renamed_kind = entry.kind;
            let was_dir = entry.is_dir && !entry.is_package;
            let old_name = entry.name.to_string();
            entry.name = new_name.to_string().into();
            entry.id = new_name.to_string().into();
            entry.icon = renamed_kind.icon();
            old_names.push((was_dir, old_name));
        }
        for (was_dir, old_name) in old_names {
            let old_child = join_location(parent, &old_name);
            if was_dir {
                let new_child = join_location(parent, new_name);
                if let Some(mut child) = data.locations.remove(&old_child) {
                    child.label = new_name.to_string().into();
                    data.locations.insert(new_child.clone(), child);
                }
                for location in data.locations.values_mut() {
                    if location.parent.as_ref() == Some(&old_child) {
                        location.parent = Some(new_child.clone());
                    }
                }
            }
        }
        Task::ready(Ok(()))
    }

    fn delete(&self, parent: &LocationId, id: &ItemId, _: &mut App) -> Task<Result<(), FileError>> {
        let mut data = self.data.borrow_mut();
        let Some(location) = data.locations.get_mut(parent) else {
            return Task::ready(Err(FileError::new("No such location")));
        };
        let Some(index) = location.entries.iter().position(|entry| entry.id == *id) else {
            return Task::ready(Err(FileError::new("No such entry")));
        };
        let entry = location.entries.remove(index);
        if entry.is_dir && !entry.is_package {
            let child = join_location(parent, &entry.name);
            let mut stack = vec![child];
            let mut descendants = Vec::new();
            while let Some(id) = stack.pop() {
                descendants.push(id.clone());
                let children: Vec<LocationId> = data
                    .locations
                    .iter()
                    .filter(|(_, location)| location.parent.as_ref() == Some(&id))
                    .map(|(id, _)| id.clone())
                    .collect();
                stack.extend(children);
            }
            for id in descendants {
                data.locations.remove(&id);
            }
        }
        Task::ready(Ok(()))
    }
}

/// The local disk, behind feature `std-fs`, so the core library keeps no
/// filesystem dependency. Location ids are display paths (`/a/b` or
/// `C:\a\b`); `resolve` returns the host path.
#[cfg(feature = "std-fs")]
#[derive(Clone, Debug, Default)]
pub struct StdFsProvider {
    home: Option<PathBuf>,
}

#[cfg(feature = "std-fs")]
impl StdFsProvider {
    #[must_use]
    pub fn new() -> Self {
        Self {
            home: std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .map(PathBuf::from),
        }
    }

    fn path_of(location: &LocationId) -> PathBuf {
        PathBuf::from(location.as_ref())
    }

    fn label_for(path: &std::path::Path) -> SharedString {
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| path.display().to_string())
            .into()
    }
}

#[cfg(feature = "std-fs")]
impl FileProvider for StdFsProvider {
    fn children(
        &self,
        location: &LocationId,
        cx: &mut App,
    ) -> Task<Result<Vec<FileEntry>, FileError>> {
        let path = Self::path_of(location);
        cx.background_executor().spawn(async move {
            let read = std::fs::read_dir(&path)
                .map_err(|error| FileError::new(format!("{}: {error}", path.display())))?;
            let mut entries = Vec::new();
            for item in read.flatten() {
                let path = item.path();
                let name = item.file_name().to_string_lossy().to_string();
                let metadata = item.metadata().ok();
                let is_dir = metadata.as_ref().is_some_and(|m| m.is_dir());
                let extension = path
                    .extension()
                    .map(|ext| ext.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                let package = is_dir && matches!(extension.as_str(), "app" | "bundle" | "pkg");
                let kind = if package {
                    FileKind::Application
                } else if is_dir {
                    FileKind::Folder
                } else if matches!(extension.as_str(), "app" | "exe") {
                    FileKind::Application
                } else {
                    FileKind::Document
                };
                let modified = metadata
                    .as_ref()
                    .and_then(|m| m.modified().ok())
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |duration| duration.as_secs());
                let size = metadata.as_ref().map_or(0, |m| m.len());
                let hidden = name.starts_with('.');
                entries.push(FileEntry {
                    id: name.clone().into(),
                    name: name.into(),
                    kind,
                    is_dir,
                    is_package: package,
                    size,
                    modified,
                    hidden,
                    icon: kind.icon(),
                });
            }
            entries.sort_by(|a, b| {
                b.is_dir
                    .cmp(&a.is_dir)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            Ok(entries)
        })
    }

    fn ancestors(&self, location: &LocationId, _: &App) -> Vec<FileAncestor> {
        let path = Self::path_of(location);
        let mut out = Vec::new();
        let mut current = path.parent().map(std::path::Path::to_path_buf);
        while let Some(parent) = current {
            out.push(FileAncestor {
                id: parent.display().to_string().into(),
                label: Self::label_for(&parent),
                icon: Icon::Folder,
            });
            current = parent.parent().map(std::path::Path::to_path_buf);
        }
        out
    }

    fn places(&self, _: &App) -> Vec<FilePlace> {
        let mut out = Vec::new();
        if let Some(home) = &self.home {
            out.push(FilePlace::new(
                home.display().to_string(),
                "terezi",
                Icon::Home,
                PlaceSection::Places,
            ));
            for (name, folder, icon) in [
                ("Desktop", "Desktop", Icon::Desktop),
                ("Documents", "Documents", Icon::Documents),
                ("Downloads", "Downloads", Icon::Documents),
                ("Applications", "Applications", Icon::Application),
            ] {
                let path = home.join(folder);
                if path.exists() {
                    out.push(FilePlace::new(
                        path.display().to_string(),
                        name,
                        icon,
                        PlaceSection::Places,
                    ));
                }
            }
        }
        let root = self
            .home
            .as_ref()
            .and_then(|home| home.ancestors().last())
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/"));
        out.push(FilePlace::new(
            root.display().to_string(),
            "Computer",
            Icon::Volume,
            PlaceSection::Devices,
        ));
        for (id, label) in [
            ("search:today", "Today"),
            ("search:yesterday", "Yesterday"),
            ("search:week", "Past Week"),
        ] {
            out.push(FilePlace::new(
                id,
                label,
                Icon::SmartSearch,
                PlaceSection::SearchFor,
            ));
        }
        out
    }

    fn create_folder(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<LocationId, FileError>> {
        let parent = Self::path_of(parent);
        let name = name.to_string();
        cx.background_executor().spawn(async move {
            let path = parent.join(&name);
            std::fs::create_dir(&path)
                .map_err(|error| FileError::new(format!("{}: {error}", path.display())))?;
            Ok(path.display().to_string().into())
        })
    }

    fn exists(&self, parent: &LocationId, name: &str, cx: &mut App) -> Task<bool> {
        let path = Self::path_of(parent).join(name);
        cx.background_executor().spawn(async move { path.exists() })
    }

    fn delete(
        &self,
        parent: &LocationId,
        id: &ItemId,
        cx: &mut App,
    ) -> Task<Result<(), FileError>> {
        let path = Self::path_of(parent).join(id.as_ref());
        cx.background_executor().spawn(async move {
            let is_dir = path.is_dir();
            let result = if is_dir {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            result.map_err(|error| FileError::new(format!("{}: {error}", path.display())))
        })
    }

    fn resolve(&self, location: &LocationId) -> Option<PathBuf> {
        Some(Self::path_of(location))
    }

    fn location_of(&self, path: &std::path::Path) -> Option<LocationId> {
        Some(path.display().to_string().into())
    }

    fn label(&self, location: &LocationId, _: &App) -> SharedString {
        Self::label_for(&Self::path_of(location))
    }
}

/// The modification time of `path`, or 0. Used by providers that build entries
/// from metadata rather than `FileEntry::modified`.
#[must_use]
pub fn modified_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
