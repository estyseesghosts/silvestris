//! FileProvider (#78, extracted from #29): everything a browser knows about
//! files.
//!
//! The panels never touch `std::fs`. A provider answers with stable, opaque
//! location ids and `FileEntry` values; `MemoryProvider` serves tests and the
//! gallery, `StdFsProvider` (feature `std-fs`) is the local-disk adapter and
//! does its I/O on GPUI's background executor.
//!
//! Identity is provider-owned. A [`FileEntry`] carries both a stable `id` for
//! selection and a `location` for navigation and IO. The panel never builds a
//! path from a display name: it asks the provider to resolve the location.
//! Display text may be lossy; the provider keeps the exact host path.
//!
//! `FileEntry` carries the metadata the Open and Save panels display:
//! identity, location, name, kind, directory/package/link flags, size,
//! modification time, hidden state and a semantic icon. It does not carry an
//! `AnyElement`, so a provider is plain data and can be used from a background
//! task.

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

/// One entry as the provider reports it.
///
/// `id` is stable within its location and is what selection and rename use.
/// `location` is the provider handle for the entry itself; it is the only
/// value the panel may hand back to the provider for navigation or
/// `resolve`. `name` is display text and may be lossy on a host filesystem.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileEntry {
    pub id: ItemId,
    pub location: LocationId,
    pub name: SharedString,
    /// Lowercased display name, computed once when the name is set. Name
    /// sorting and filename search use this key instead of lowercasing inside
    /// every comparison. It is simple Unicode lowercasing, not full case
    /// folding, which matches the previous semantics.
    pub(crate) name_key: SharedString,
    pub kind: FileKind,
    pub is_dir: bool,
    /// A directory the Finder treats as a single document (`.app`, `.bundle`).
    pub is_package: bool,
    /// A symbolic link or reparse point. Links are classified by their target
    /// when the target can be read; a broken link is an `Alias`.
    pub is_link: bool,
    /// `false` when metadata could not be read. Size and date then display as
    /// unknown rather than as zero.
    pub metadata_known: bool,
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
        let id = id.into();
        let location: LocationId = id.to_string().into();
        let name = name.into();
        let name_key: SharedString = name.to_lowercase().into();
        Self {
            id,
            location,
            name,
            name_key,
            kind,
            is_dir,
            is_package: false,
            is_link: false,
            metadata_known: true,
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

    /// Set the provider-owned identity for this entry. Providers that list a
    /// directory must set it; the default is the entry id.
    #[must_use]
    pub fn at(mut self, location: impl Into<LocationId>) -> Self {
        self.location = location.into();
        self
    }

    pub fn package(mut self, package: bool) -> Self {
        self.is_package = package;
        self
    }

    pub fn link(mut self, link: bool) -> Self {
        self.is_link = link;
        self
    }

    pub fn metadata_known(mut self, known: bool) -> Self {
        self.metadata_known = known;
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

    /// The lowercased name used for sorting and search. Display text stays in
    /// [`name`](Self::name).
    #[must_use]
    pub fn name_key(&self) -> &SharedString {
        &self.name_key
    }

    /// Replace the display name and its normalized key together. Providers
    /// that rename an entry use this so the key never diverges from `name`.
    pub fn set_name(&mut self, name: impl Into<SharedString>) {
        self.name = name.into();
        self.name_key = self.name.to_lowercase().into();
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
        if self.is_dir || !self.metadata_known {
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
        if self.modified == 0 || !self.metadata_known {
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

/// A directory listing plus the non-fatal failures found while reading it.
/// One unreadable entry is a warning; an unreadable directory is an error.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileListing {
    pub entries: Vec<FileEntry>,
    pub warnings: Vec<FileError>,
}

impl FileListing {
    #[must_use]
    pub fn new(entries: Vec<FileEntry>) -> Self {
        Self {
            entries,
            warnings: Vec::new(),
        }
    }
}

/// One entry's metadata, for providers that enumerate names first and fill in
/// size, date, link target and host attributes afterwards.
///
/// Enrichment is addressed by id, so a late answer can never replace a
/// different entry that now has the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileMetadata {
    pub kind: FileKind,
    pub is_dir: bool,
    pub is_package: bool,
    pub is_link: bool,
    pub metadata_known: bool,
    pub size: u64,
    pub modified: u64,
    pub hidden: bool,
    pub icon: Icon,
}

/// The answer to [`FileProvider::metadata_enrichment`]: one metadata record per
/// entry id that was enriched.
pub type MetadataUpdates = Vec<(ItemId, FileMetadata)>;

impl FileMetadata {
    /// Apply this metadata to an entry, preserving its identity, location and
    /// display name.
    pub fn apply(self, entry: &mut FileEntry) {
        entry.kind = self.kind;
        entry.is_dir = self.is_dir;
        entry.is_package = self.is_package;
        entry.is_link = self.is_link;
        entry.metadata_known = self.metadata_known;
        entry.size = self.size;
        entry.modified = self.modified;
        entry.hidden = self.hidden;
        entry.icon = self.icon;
    }
}

/// Where a sidebar place belongs.
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

/// What a provider can actually do. The panel builds its context menus and
/// New Folder button from these flags, so it never advertises an operation the
/// provider would refuse.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FileProviderCapabilities {
    pub create_folder: bool,
    pub rename: bool,
    pub copy_path: bool,
    /// Move to the platform trash/recycle bin.
    pub trash: bool,
    /// Permanently destroy an entry. Only a provider that owns the storage
    /// policy should set this; the panel asks for confirmation first.
    pub delete_permanently: bool,
    /// Eject or unmount a removable location.
    pub eject: bool,
}

impl FileProviderCapabilities {
    #[must_use]
    pub const fn all() -> Self {
        Self {
            create_folder: true,
            rename: true,
            copy_path: true,
            trash: true,
            delete_permanently: true,
            eject: true,
        }
    }
}

/// The application's file access.
///
/// Methods callable from rendering (`ancestors`, `label`, `location_icon`,
/// `validate_name`) must not perform blocking filesystem IO. `places`,
/// `children`, `create_folder`, `exists`, `rename` and `resolve` are the IO
/// boundary; the local-disk adapter runs all of them except `resolve` on the
/// background executor.
pub trait FileProvider: 'static {
    /// List `location`'s entries. The model applies the hidden policy;
    /// extension/choose policy belongs to the panel. Entry locations must be
    /// set by the provider.
    fn children(&self, location: &LocationId, cx: &mut App)
        -> Task<Result<FileListing, FileError>>;

    /// Fill in metadata that [`children`](Self::children) deliberately left
    /// out. A provider that can enumerate names cheaply overrides this; the
    /// model applies the answer to the same ids when it arrives. `None` means
    /// the listing is already complete.
    ///
    /// The result must stay ordered as `entries` is: the model matches by id,
    /// but a stable order keeps diagnostics and tests deterministic.
    fn metadata_enrichment(
        &self,
        _entries: &[FileEntry],
        _cx: &mut App,
    ) -> Option<Task<Result<MetadataUpdates, FileError>>> {
        None
    }

    /// The ancestors of `location`, nearest parent first. The current
    /// location itself is not included.
    fn ancestors(&self, location: &LocationId, cx: &App) -> Vec<FileAncestor>;

    /// DEVICES, PLACES and SEARCH FOR entries. Discovery that touches the host
    /// belongs on the background executor, not in the caller's thread.
    fn places(&self, cx: &mut App) -> Task<Result<Vec<FilePlace>, FileError>>;

    /// Create `name` under `parent` and return the new location id.
    fn create_folder(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<LocationId, FileError>>;

    /// Whether `name` already exists under `parent`.
    ///
    /// An error means the provider could not determine the answer. The Save
    /// panel must treat that as an error, never as "the target is missing".
    fn exists(
        &self,
        parent: &LocationId,
        name: &str,
        cx: &mut App,
    ) -> Task<Result<bool, FileError>>;

    /// Check a name before it is created or renamed. This is pure and must not
    /// block. The default rejects empty names, `.`, `..`, path separators and
    /// NUL; a real host filesystem applies its own rules in its adapter.
    fn validate_name(&self, _parent: &LocationId, name: &str, _cx: &App) -> Result<(), FileError> {
        validate_structural_name(name)
    }

    /// The location a child called `name` would have under `parent`, including
    /// a target that does not exist yet. The Save panel uses this for its
    /// outcome. The default works for a path-backed provider; a provider with
    /// its own child scheme overrides it.
    fn child_location(&self, parent: &LocationId, name: &str) -> Option<LocationId> {
        if validate_structural_name(name).is_err() {
            return None;
        }
        let parent = self.resolve(parent)?;
        self.location_of(&parent.join(name))
    }

    /// Rename an entry in place. Providers that cannot rename return the
    /// default error; the panel only offers the command when `capabilities`
    /// says it is available.
    fn rename(
        &self,
        _parent: &LocationId,
        _id: &ItemId,
        _new_name: &str,
        _cx: &App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new("This provider cannot rename entries")))
    }

    /// Move an entry to the platform trash. No confirmation is required: the
    /// operation is reversible and the platform owns the recovery UI.
    fn trash(
        &self,
        _parent: &LocationId,
        _id: &ItemId,
        _cx: &mut App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new(
            "This provider cannot move entries to the trash",
        )))
    }

    /// Permanently destroy an entry.
    ///
    /// This is the seam for a file-manager application that owns its own
    /// storage policy. The panel calls it only after the user confirms a
    /// destructive alert. A picker adapter should normally leave the
    /// capability off and offer [`trash`](Self::trash) instead.
    fn delete_permanently(
        &self,
        _parent: &LocationId,
        _id: &ItemId,
        _cx: &mut App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new("This provider cannot delete entries")))
    }

    /// What the panel may offer in its menus.
    fn capabilities(&self) -> FileProviderCapabilities {
        FileProviderCapabilities::default()
    }

    /// Keep these handles alive. Providers with a bounded handle table call
    /// this with the current location and history so navigation can return to
    /// them; a provider that never reclaims handles ignores it.
    fn retain(&self, _locations: &[LocationId], _cx: &mut App) {}

    /// Eject or unmount a removable location. Only offered when
    /// [`capabilities`](Self::capabilities) says `eject`.
    fn eject(&self, _location: &LocationId, _cx: &mut App) -> Task<Result<(), FileError>> {
        Task::ready(Err(FileError::new("This provider cannot eject locations")))
    }

    /// A host path for a location, when the provider has one. The panel never
    /// needs it; a consumer that writes the chosen file does.
    fn resolve(&self, _location: &LocationId) -> Option<PathBuf> {
        None
    }

    /// The location id for a host path, for a "go to folder" feature. Like the
    /// other ids, the result belongs to this provider instance.
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

/// The name rules every provider shares: a name is one path component. The
/// provider's own `validate_name` adds host rules; Unix allows `\` in a
/// filename, so the shared check does not reject it.
pub fn validate_structural_name(name: &str) -> Result<(), FileError> {
    if name.is_empty() {
        return Err(FileError::new("The name cannot be empty."));
    }
    if name == "." || name == ".." {
        return Err(FileError::new("That name is reserved."));
    }
    if name.contains('/') || name.contains('\0') {
        return Err(FileError::new("Names cannot contain a path separator."));
    }
    Ok(())
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

/// The provider's own parent/name location id, matching the memory adapter's
/// child convention. Real adapters keep opaque ids in a map instead.
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

    /// Remove one entry and, for a memory folder, its registered descendants.
    fn remove(&self, parent: &LocationId, id: &ItemId) -> Result<(), FileError> {
        let mut data = self.data.borrow_mut();
        let Some(location) = data.locations.get_mut(parent) else {
            return Err(FileError::new("No such location"));
        };
        let Some(index) = location.entries.iter().position(|entry| entry.id == *id) else {
            return Err(FileError::new("No such entry"));
        };
        let entry = location.entries.remove(index);
        if entry.is_dir && !entry.is_package {
            let mut stack = vec![entry.location.clone()];
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
        Ok(())
    }
}

impl FileProvider for MemoryProvider {
    fn children(&self, location: &LocationId, _: &mut App) -> Task<Result<FileListing, FileError>> {
        let result = self
            .data
            .borrow()
            .locations
            .get(location)
            .map(|found| {
                let mut entries = found.entries.clone();
                for entry in &mut entries {
                    entry.location = join_location(location, entry.id.as_ref());
                }
                FileListing::new(entries)
            })
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

    fn places(&self, _: &mut App) -> Task<Result<Vec<FilePlace>, FileError>> {
        Task::ready(Ok(self.data.borrow().places.clone()))
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
        cx: &mut App,
    ) -> Task<Result<LocationId, FileError>> {
        if let Err(error) = self.validate_name(parent, name, cx) {
            return Task::ready(Err(error));
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
                .push(FileEntry::folder(name.to_string(), name.to_string()).at(id.clone()));
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

    fn exists(
        &self,
        parent: &LocationId,
        name: &str,
        _: &mut App,
    ) -> Task<Result<bool, FileError>> {
        if let Err(error) = validate_structural_name(name) {
            return Task::ready(Err(error));
        }
        Task::ready(Ok(self.has_name(parent, name)))
    }

    fn child_location(&self, parent: &LocationId, name: &str) -> Option<LocationId> {
        if validate_structural_name(name).is_err() {
            return None;
        }
        Some(join_location(parent, name))
    }

    fn validate_name(&self, _parent: &LocationId, name: &str, _: &App) -> Result<(), FileError> {
        validate_structural_name(name)
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
        let mut data = self.data.borrow_mut();
        let old_location = {
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
            let old_location = entry.location.clone();
            let renamed_kind = entry.kind;
            entry.set_name(new_name.to_string());
            entry.id = new_name.to_string().into();
            entry.location = join_location(parent, new_name);
            entry.icon = renamed_kind.icon();
            old_location
        };
        if old_location != join_location(parent, new_name) {
            let new_child = join_location(parent, new_name);
            if let Some(mut child) = data.locations.remove(&old_location) {
                child.label = new_name.to_string().into();
                data.locations.insert(new_child.clone(), child);
            }
            for location in data.locations.values_mut() {
                if location.parent.as_ref() == Some(&old_location) {
                    location.parent = Some(new_child.clone());
                }
            }
        }
        Task::ready(Ok(()))
    }

    fn trash(&self, parent: &LocationId, id: &ItemId, _: &mut App) -> Task<Result<(), FileError>> {
        Task::ready(self.remove(parent, id))
    }

    fn delete_permanently(
        &self,
        parent: &LocationId,
        id: &ItemId,
        _: &mut App,
    ) -> Task<Result<(), FileError>> {
        Task::ready(self.remove(parent, id))
    }

    fn capabilities(&self) -> FileProviderCapabilities {
        FileProviderCapabilities::all()
    }

    fn eject(&self, location: &LocationId, _: &mut App) -> Task<Result<(), FileError>> {
        let mut data = self.data.borrow_mut();
        let before = data.places.len();
        data.places.retain(|place| &place.id != location);
        if data.places.len() == before {
            return Task::ready(Err(FileError::new("No such removable place")));
        }
        Task::ready(Ok(()))
    }
}

/// The modification time of `path`, or 0. Used by providers that build entries
/// from metadata rather than `FileEntry::modified`.
#[must_use]
pub fn modified_seconds(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(feature = "std-fs")]
pub use super::std_fs::StdFsProvider;
