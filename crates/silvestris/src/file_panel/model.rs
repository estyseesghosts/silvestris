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

use std::rc::Rc;

use gpui::{App, AppContext, Context, Entity, EventEmitter, Task};
use serde::{Deserialize, Serialize};

use crate::binding::ListController;
use crate::controls::path_control::{LocationId, PathEntry};
use crate::icons::Icon;

use super::provider::{FileAncestor, FileEntry, FileError, FileListing, FilePlace, FileProvider};

/// Icons, list or columns. The order matches the toolbar segments in the
/// 4.15.47 AM Open capture: icon, list, columns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileViewMode {
    Icons,
    #[default]
    List,
    Columns,
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
