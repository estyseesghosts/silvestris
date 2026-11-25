//! FileBrowserModel (#29 core): the one owner of the current location,
//! history, listing, filters and view mode for both panels.
//!
//! Controls render the model; they never navigate or touch a provider. A
//! sidebar click, a path control choice and a column-browser path all call
//! `navigate`, so the three representations cannot disagree.
//!
//! Navigation is asynchronous and cancellable. Every load carries a
//! generation; a result from an older generation is dropped, so a slow
//! directory can never overwrite the location the user just chose.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

use gpui::{App, AppContext, Context, Entity, EventEmitter, Task};
use serde::{Deserialize, Serialize};

use crate::binding::ListController;
use crate::controls::path_control::{LocationId, PathEntry};
use crate::controls::SortDirection;
use crate::icons::Icon;

use super::provider::{
    FileAncestor, FileEntry, FileError, FileListing, FileMetadata, FilePlace, FileProvider,
};

/// Icons, list or columns. The order matches the toolbar segments in the
/// 4.15.47 AM Open capture: icon, list, columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileViewMode {
    Icons,
    #[default]
    List,
    Columns,
}

/// The column a panel sort uses. Every view consumes the same key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FileSortKey {
    #[default]
    Name,
    Kind,
    Modified,
    Size,
}

impl FileSortKey {
    /// The direction the context menu uses the first time a key is chosen.
    /// Finder shows the newest and largest first on those columns while name
    /// and kind read naturally ascending.
    #[must_use]
    pub fn default_direction(self) -> SortDirection {
        match self {
            FileSortKey::Name | FileSortKey::Kind => SortDirection::Ascending,
            FileSortKey::Modified | FileSortKey::Size => SortDirection::Descending,
        }
    }

    /// The table column that presents this key.
    #[must_use]
    pub fn column_id(self) -> &'static str {
        match self {
            FileSortKey::Name => "name",
            FileSortKey::Kind => "kind",
            FileSortKey::Modified => "modified",
            FileSortKey::Size => "size",
        }
    }

    /// The key a table column id stands for, when it has one.
    #[must_use]
    pub fn from_column_id(column: &str) -> Option<Self> {
        match column {
            "name" => Some(FileSortKey::Name),
            "kind" => Some(FileSortKey::Kind),
            "modified" => Some(FileSortKey::Modified),
            "size" => Some(FileSortKey::Size),
            _ => None,
        }
    }
}

/// The panel's one sort policy: key plus direction. The list, icon and column
/// views all order through [`FileSort::compare`], so switching views cannot
/// change the ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileSort {
    pub key: FileSortKey,
    pub direction: SortDirection,
}

impl Default for FileSort {
    fn default() -> Self {
        Self {
            key: FileSortKey::Name,
            direction: SortDirection::Ascending,
        }
    }
}

impl FileSort {
    #[must_use]
    pub fn new(key: FileSortKey, direction: SortDirection) -> Self {
        Self { key, direction }
    }

    /// Folders stay above files in both directions, as in Finder. Only the
    /// key comparison reverses.
    #[must_use]
    pub fn compare(&self, a: &FileEntry, b: &FileEntry) -> Ordering {
        let folders = b.is_dir.cmp(&a.is_dir);
        if folders != Ordering::Equal {
            return folders;
        }
        let order = match self.key {
            FileSortKey::Name => a.name_key().cmp(b.name_key()),
            FileSortKey::Kind => a.kind.cmp(&b.kind),
            FileSortKey::Modified => a.modified.cmp(&b.modified),
            FileSortKey::Size => a.size.cmp(&b.size),
        };
        match self.direction {
            SortDirection::Ascending => order,
            SortDirection::Descending => order.reverse(),
        }
    }

    #[must_use]
    pub fn toggled(self) -> Self {
        Self {
            direction: match self.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            },
            ..self
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FileBrowserEvent {
    Loading {
        location: LocationId,
    },
    Loaded {
        location: LocationId,
        count: usize,
    },
    Failed {
        location: LocationId,
        error: FileError,
    },
    LocationChanged {
        location: LocationId,
    },
    /// The view mode changed; the panel swaps presentation without touching
    /// the location or the selection.
    ViewChanged {
        mode: FileViewMode,
    },
}

pub struct FileBrowserModel {
    provider: Rc<dyn FileProvider>,
    location: LocationId,
    back: Vec<LocationId>,
    forward: Vec<LocationId>,
    controller: Entity<ListController<FileEntry>>,
    loading: bool,
    error: Option<FileError>,
    warnings: Vec<FileError>,
    show_hidden: bool,
    allowed_types: Vec<String>,
    allows_other_types: bool,
    view: FileViewMode,
    /// One sort policy for list, icons and columns. `None` keeps provider
    /// listing order.
    sort: Option<FileSort>,
    generation: u64,
    task: Option<Task<()>>,
}

impl EventEmitter<FileBrowserEvent> for FileBrowserModel {}

impl FileBrowserModel {
    pub fn new(
        provider: Rc<dyn FileProvider>,
        location: impl Into<LocationId>,
        cx: &mut Context<Self>,
    ) -> Self {
        let controller = cx.new(|_| ListController::new(|entry: &FileEntry| entry.id.clone()));
        let mut model = Self {
            provider,
            location: location.into(),
            back: Vec::new(),
            forward: Vec::new(),
            controller,
            loading: false,
            error: None,
            warnings: Vec::new(),
            show_hidden: false,
            allowed_types: Vec::new(),
            allows_other_types: true,
            view: FileViewMode::List,
            sort: None,
            generation: 0,
            task: None,
        };
        model.load(false, cx);
        model
    }

    #[must_use]
    pub fn provider(&self) -> &Rc<dyn FileProvider> {
        &self.provider
    }

    #[must_use]
    pub fn location(&self) -> &LocationId {
        &self.location
    }

    #[must_use]
    pub fn controller(&self) -> Entity<ListController<FileEntry>> {
        self.controller.clone()
    }

    #[must_use]
    pub fn view(&self) -> FileViewMode {
        self.view
    }

    /// The panel's sort policy. Every view uses it.
    #[must_use]
    pub fn sort(&self) -> Option<FileSort> {
        self.sort
    }

    /// Replace the sort policy and re-sort the controller. Selection survives
    /// by id. `None` restores provider order.
    pub fn set_sort(&mut self, sort: Option<FileSort>, cx: &mut Context<Self>) {
        self.sort = sort;
        let controller = self.controller.clone();
        controller.update(cx, |controller, cx| {
            match sort {
                Some(sort) => {
                    controller
                        .set_sort(Some(move |a: &FileEntry, b: &FileEntry| sort.compare(a, b)));
                }
                None => controller.set_sort(None::<fn(&FileEntry, &FileEntry) -> Ordering>),
            }
            cx.notify();
        });
        cx.notify();
    }

    /// Sort `entries` with the panel policy, for a view that owns a separate
    /// list (the column browser's child columns).
    pub fn sort_entries(&self, entries: &mut [FileEntry]) {
        if let Some(sort) = self.sort {
            entries.sort_by(|a, b| sort.compare(a, b));
        }
    }

    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    #[must_use]
    pub fn error(&self) -> Option<&FileError> {
        self.error.as_ref()
    }

    /// Non-fatal failures from the last listing: an entry vanished or its
    /// metadata could not be read while the directory itself loaded.
    #[must_use]
    pub fn warnings(&self) -> &[FileError] {
        &self.warnings
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    #[must_use]
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    #[must_use]
    pub fn show_hidden(&self) -> bool {
        self.show_hidden
    }

    #[must_use]
    pub fn allowed_types(&self) -> &[String] {
        &self.allowed_types
    }

    #[must_use]
    pub fn allows_other_types(&self) -> bool {
        self.allows_other_types
    }

    /// Move to `location`, recording history. Navigating away clears the
    /// forward stack and the selection.
    pub fn navigate(&mut self, location: impl Into<LocationId>, cx: &mut Context<Self>) {
        let location = location.into();
        if location == self.location {
            self.reload(cx);
            return;
        }
        self.back.push(self.location.clone());
        self.forward.clear();
        self.location = location;
        cx.emit(FileBrowserEvent::LocationChanged {
            location: self.location.clone(),
        });
        self.load(false, cx);
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        let Some(previous) = self.back.pop() else {
            return;
        };
        self.forward.push(self.location.clone());
        self.location = previous;
        cx.emit(FileBrowserEvent::LocationChanged {
            location: self.location.clone(),
        });
        self.load(false, cx);
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        let Some(next) = self.forward.pop() else {
            return;
        };
        self.back.push(self.location.clone());
        self.location = next;
        cx.emit(FileBrowserEvent::LocationChanged {
            location: self.location.clone(),
        });
        self.load(false, cx);
    }

    /// Re-list the current location. The selection survives by id.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.load(true, cx);
    }

    pub fn set_show_hidden(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_hidden != show {
            self.show_hidden = show;
            self.load(true, cx);
        }
    }

    /// Extensions without the dot. Directories always match.
    pub fn set_allowed_types(&mut self, types: impl IntoIterator<Item = String>) {
        self.allowed_types = types.into_iter().map(|s| s.to_lowercase()).collect();
    }

    pub fn set_allows_other_types(&mut self, allows: bool) {
        self.allows_other_types = allows;
    }

    /// Whether the entry passes the allowed-type policy. Save dims a
    /// non-matching file; Open disables it. Directories always pass.
    #[must_use]
    pub fn matches_types(&self, entry: &FileEntry) -> bool {
        entry.is_dir || self.allows_other_types || entry.matches_types(&self.allowed_types)
    }

    /// Switch presentation. The location, controller and selection stay.
    pub fn set_view(&mut self, view: FileViewMode, cx: &mut Context<Self>) {
        if self.view != view {
            self.view = view;
            cx.emit(FileBrowserEvent::ViewChanged { mode: view });
            cx.notify();
        }
    }

    pub fn set_location_entry(&mut self, location: impl Into<LocationId>, cx: &mut Context<Self>) {
        self.navigate(location, cx);
    }

    /// The current location as a path-control entry.
    #[must_use]
    pub fn current_entry(&self, cx: &App) -> PathEntry {
        PathEntry::new(
            self.location.clone(),
            self.provider.label(&self.location, cx),
            self.provider.location_icon(&self.location),
        )
    }

    /// Ancestors, nearest parent first.
    #[must_use]
    pub fn ancestors(&self, cx: &App) -> Vec<FileAncestor> {
        self.provider.ancestors(&self.location, cx)
    }

    /// Ancestors converted to the path control's entries.
    #[must_use]
    pub fn path_ancestors(&self, cx: &App) -> Vec<PathEntry> {
        self.ancestors(cx)
            .into_iter()
            .map(|ancestor| PathEntry::new(ancestor.id, ancestor.label, ancestor.icon))
            .collect()
    }

    #[must_use]
    pub fn selected_entries(&self, cx: &App) -> Vec<FileEntry> {
        self.controller
            .read(cx)
            .selected_items()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Selected entries as host paths, when the provider can resolve them.
    #[must_use]
    pub fn selected_paths(&self, cx: &App) -> Vec<std::path::PathBuf> {
        self.selected_entries(cx)
            .into_iter()
            .filter_map(|entry| self.provider.resolve(&entry.location))
            .collect()
    }

    /// The candidate path for `name` in the current location.
    #[must_use]
    pub fn path_for(&self, name: &str) -> Option<std::path::PathBuf> {
        self.provider.resolve(&self.child_location(name)?)
    }

    /// The provider handle a child called `name` would have, including a
    /// target that does not exist yet.
    #[must_use]
    pub fn child_location(&self, name: &str) -> Option<LocationId> {
        self.provider.child_location(&self.location, name)
    }

    /// Start a folder creation in the current location. The caller selects the
    /// result after reloading.
    pub fn create_folder(&self, name: &str, cx: &mut App) -> Task<Result<LocationId, FileError>> {
        self.provider.create_folder(&self.location, name, cx)
    }

    /// The provider-owned location for a listed entry.
    #[must_use]
    pub fn location_of_entry(&self, entry: &FileEntry, cx: &App) -> LocationId {
        self.location_of_child(&self.location, entry, cx)
    }

    /// Like [`location_of_entry`](Self::location_of_entry), for an entry that
    /// belongs to `parent` rather than the panel's current location. The
    /// entry's location comes from the provider; the parent is not used to
    /// rebuild a path from display text.
    #[must_use]
    pub fn location_of_child(
        &self,
        _parent: &LocationId,
        entry: &FileEntry,
        _: &App,
    ) -> LocationId {
        entry.location.clone()
    }

    /// The host path for a child entry, when the provider resolves paths.
    #[must_use]
    pub fn path_of_entry(&self, entry: &FileEntry) -> Option<std::path::PathBuf> {
        self.provider.resolve(&entry.location)
    }

    /// Like [`path_of_entry`](Self::path_of_entry), for a specific parent. The
    /// entry's provider-owned location is authoritative.
    #[must_use]
    pub fn path_of_child(
        &self,
        _parent: &LocationId,
        entry: &FileEntry,
    ) -> Option<std::path::PathBuf> {
        self.provider.resolve(&entry.location)
    }

    pub fn exists(&self, name: &str, cx: &mut App) -> Task<Result<bool, FileError>> {
        self.provider.exists(&self.location, name, cx)
    }

    /// Ask the provider whether a name is legal here. Pure and non-blocking.
    pub fn validate_name(&self, name: &str, cx: &App) -> Result<(), FileError> {
        self.provider.validate_name(&self.location, name, cx)
    }

    fn load(&mut self, keep_selection: bool, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.loading = true;
        self.error = None;
        cx.emit(FileBrowserEvent::Loading {
            location: self.location.clone(),
        });
        let location = self.location.clone();
        // Navigation history and the current location are live handles. A
        // provider with a bounded handle table must not reclaim them while the
        // user can still go back or forward to them.
        let mut live = Vec::with_capacity(self.back.len() + self.forward.len() + 1);
        live.push(location.clone());
        live.extend(self.back.iter().cloned());
        live.extend(self.forward.iter().cloned());
        self.provider.retain(&live, cx);
        let pending = self.provider.children(&location, cx);
        let task = cx.spawn(async move |this, cx| {
            let result = pending.await;
            let _ = this.update(cx, |model, cx| {
                if model.generation == generation {
                    model.apply(location, result, keep_selection, cx);
                }
            });
        });
        // Dropping the previous task cancels a slow earlier listing.
        self.task = Some(task);
        cx.notify();
    }

    fn apply(
        &mut self,
        location: LocationId,
        result: Result<FileListing, FileError>,
        keep_selection: bool,
        cx: &mut Context<Self>,
    ) {
        if location != self.location {
            return;
        }
        self.loading = false;
        match result {
            Ok(listing) => {
                let FileListing { entries, warnings } = listing;
                self.warnings = warnings;
                let entries: Vec<FileEntry> = entries
                    .into_iter()
                    .filter(|entry| self.show_hidden || !entry.is_hidden())
                    .collect();
                let count = entries.len();
                // Cheap listings can fill in size/date/link metadata after the
                // names are visible. The answer is matched by id and guarded by
                // the load generation.
                let enrichment = self.provider.metadata_enrichment(&entries, cx);
                let controller = self.controller.clone();
                if !keep_selection {
                    controller.update(cx, |controller, cx| {
                        controller.clear_selection();
                        cx.notify();
                    });
                }
                controller.update(cx, |controller, cx| {
                    controller.set_items(entries);
                    cx.notify();
                });
                if let Some(task) = enrichment {
                    let controller = self.controller.clone();
                    let show_hidden = self.show_hidden;
                    let generation = self.generation;
                    cx.spawn(async move |this, cx| {
                        let result = task.await;
                        let _ = this.update(cx, |model, cx| {
                            if model.generation != generation {
                                return;
                            }
                            match result {
                                Ok(updates) => {
                                    let metadata: HashMap<_, FileMetadata> =
                                        updates.into_iter().collect();
                                    controller.update(cx, |controller, cx| {
                                        controller.update_items(|entry| {
                                            if let Some(metadata) = metadata.get(&entry.id) {
                                                metadata.apply(entry);
                                            }
                                        });
                                        if !show_hidden {
                                            controller.retain_items(|entry| !entry.is_hidden());
                                        }
                                        cx.notify();
                                    });
                                }
                                Err(error) => {
                                    model.warnings.push(error);
                                    cx.notify();
                                }
                            }
                        });
                    })
                    .detach();
                }
                cx.emit(FileBrowserEvent::Loaded { location, count });
            }
            Err(error) => {
                self.error = Some(error.clone());
                self.warnings.clear();
                let controller = self.controller.clone();
                controller.update(cx, |controller, cx| {
                    controller.clear_selection();
                    controller.set_items(Vec::new());
                    cx.notify();
                });
                cx.emit(FileBrowserEvent::Failed { location, error });
            }
        }
        cx.notify();
    }
}

/// Whether the entry can be chosen by an Open panel: a folder when
/// `directories` is set, a file when `files` is set, and a package counts as
/// a file.
#[must_use]
pub fn choosable(entry: &FileEntry, files: bool, directories: bool) -> bool {
    if entry.is_dir && !entry.is_package {
        directories
    } else {
        files
    }
}

/// Icon for a location's place, for the path pop-up's current row.
#[must_use]
pub fn place_icon(places: &[FilePlace], location: &LocationId) -> Icon {
    places
        .iter()
        .find(|place| &place.id == location)
        .map_or(Icon::Folder, |place| place.icon)
}

/// Kind ordering helper for a column that sorts folders first.
#[must_use]
pub fn compare_kind_folders_first(a: &FileEntry, b: &FileEntry) -> std::cmp::Ordering {
    b.is_dir.cmp(&a.is_dir).then_with(|| a.kind.cmp(&b.kind))
}
