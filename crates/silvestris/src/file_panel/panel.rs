//! Shared Open/Save panel view (#29/T7).
//!
//! The panel composes the generic controls: `SourceList` sidebar, `TableView`
//! list or `ColumnBrowser` columns, `PathPopUp`, `TextField` search and name
//! fields, the accessory slot and the button row. All file data comes from
//! `FileBrowserModel` / `FileProvider`; the panel owns policy only.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use std::time::Duration;

use gpui::{
    deferred, div, prelude::*, px, Animation, AnimationExt, AnyElement, AnyWindowHandle, App,
    ClipboardItem, Context, Entity, FocusHandle, Focusable, KeyDownEvent, Point, SharedString,
    Subscription, Task, WeakEntity, Window,
};

use crate::binding::ItemId;
use crate::chrome::WindowFrame;
use crate::command::CommandRegistry;
use crate::controls::column_browser::{BrowserEvent, BrowserNode, ColumnBrowser};
use crate::controls::icon_grid::{IconGrid, IconGridEvent};
use crate::controls::path_control::{LocationId, PathEntry, PathPopUp};
use crate::controls::popup_button::PopUpButton;
use crate::controls::source_list::{RowAccessory, SourceList, SourceListEvent, SourceListStyle};
use crate::controls::table_view::{SortDirection, TableColumn, TableEvent, TableView};
use crate::controls::text_field::{TextField, TextFieldEvent};
use crate::controls::{
    Alert, AlertButton, AlertResult, AlertRole, Button, CheckState, Checkbox, ControlSize,
    Disclosure, DisclosureStyle,
};
use crate::convert::hsla;
use crate::file_panel::chrome::{self, PanelToolbar};
use crate::file_panel::model::{
    choosable, FileBrowserEvent, FileBrowserModel, FileSort, FileSortKey, FileViewMode,
};
use crate::file_panel::provider::{
    FileEntry, FileError, FileListing, FilePlace, FileProvider, FileProviderCapabilities,
    PlaceSection,
};
use crate::file_panel::state::{PanelState, PanelStateStore};
use crate::icons;
use crate::menu::PopupMenu;
use crate::modal::{self, ModalPhase, ModalResult, ModalScope, ModalSession};
use crate::sheet;
use crate::window_role::{open_role, WindowRole};
use crate::ActiveTheme;
use silvestris_core::command::{CommandDescriptor, MenuEntry, MenuModel};

/// Which panel is being shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelMode {
    Save,
    Open,
}

/// What the consumer receives when the panel closes. `result` is `Accepted`
/// for Save/Open and `Rejected`/`Cancelled` otherwise. A target is reported as
/// a host path when the provider resolves one, and always as a location id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelOutcome {
    pub result: ModalResult,
    pub paths: Vec<PathBuf>,
    pub locations: Vec<LocationId>,
    pub hide_extension: bool,
    /// For an accepted Save: whether the user authorized replacing an
    /// existing target. `None` for Open and for a cancelled Save.
    pub disposition: Option<SaveDisposition>,
}

/// How an accepted Save relates to the target that existed when the panel
/// checked it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDisposition {
    /// The provider reported that the target did not exist.
    New,
    /// The user saw the replace-existing alert and chose Replace.
    ReplaceConfirmed,
}

impl PanelOutcome {
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.result == ModalResult::Accepted
    }

    #[must_use]
    pub fn first_path(&self) -> Option<&PathBuf> {
        self.paths.first()
    }

    #[must_use]
    pub fn first_location(&self) -> Option<&LocationId> {
        self.locations.first()
    }
}

type FinishFn = Rc<dyn Fn(&PanelOutcome, &mut App)>;
type AccessoryFn = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

/// How the Save panel treats the typed extension.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ExtensionPolicy {
    /// Keep exactly what the user typed. `allowed_types` only filters and
    /// dims the browser; it never adds or replaces an extension.
    #[default]
    Any,
    /// Append the first allowed extension when the name has none; a name with
    /// a different extension is invalid.
    Required,
    /// Force this extension (without the dot), replacing a different one.
    Fixed(String),
}

/// One entry in the Save panel's Format pop-up. `extensions` are without the
/// dot; choosing a format applies them to the allowed-type policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormatOption {
    pub label: SharedString,
    pub extensions: Vec<String>,
}

impl FormatOption {
    pub fn new(
        label: impl Into<SharedString>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            label: label.into(),
            extensions: extensions.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone)]
pub struct PanelOptions {
    pub mode: PanelMode,
    pub location: LocationId,
    pub name: String,
    pub message: Option<SharedString>,
    pub allowed_types: Vec<String>,
    pub allows_other_types: bool,
    pub hide_extension: bool,
    pub hide_extension_set: bool,
    pub expanded: bool,
    pub expanded_set: bool,
    pub allows_multiple: bool,
    pub can_choose_files: bool,
    pub can_choose_directories: bool,
    pub formats: Vec<FormatOption>,
    pub extension_policy: ExtensionPolicy,
    pub state_store: Option<Rc<dyn PanelStateStore>>,
    pub frame_store: Option<Rc<dyn crate::window_role::FrameStore>>,
    pub accessory: Option<AccessoryFn>,
    pub on_finish: Option<FinishFn>,
}

impl PanelOptions {
    pub fn new(mode: PanelMode, location: LocationId) -> Self {
        Self {
            mode,
            location,
            name: "Untitled".into(),
            message: None,
            allowed_types: Vec::new(),
            allows_other_types: true,
            hide_extension: false,
            hide_extension_set: false,
            expanded: mode == PanelMode::Open,
            expanded_set: false,
            allows_multiple: false,
            can_choose_files: true,
            can_choose_directories: false,
            formats: Vec::new(),
            extension_policy: ExtensionPolicy::Any,
            state_store: None,
            frame_store: None,
            accessory: None,
            on_finish: None,
        }
    }
}

/// Column-browser rows, by id, with the location whose column produced them.
/// The browser only carries the id and label, but activation still has to
/// honour `allowed_types` and the choose-files/directories policy, and the
/// child location has to be resolved against its own column rather than the
/// now-current directory.
type BrowserEntries = HashMap<ItemId, (LocationId, FileEntry)>;

/// Cold column entries a panel retains. Ids still shown in a column are never
/// dropped, even when the displayed set is larger than this budget.
const MAX_BROWSER_ENTRIES: usize = 8192;

/// One panel, shared by Save and Open.
pub struct FilePanel {
    mode: PanelMode,
    model: Entity<FileBrowserModel>,
    session: Entity<ModalSession>,
    sidebar: Entity<SourceList>,
    table: Entity<TableView<FileEntry>>,
    icons: Option<Entity<IconGrid<FileEntry>>>,
    browser: Option<Entity<ColumnBrowser>>,
    /// Entries shown in the column browser, by id, with the location whose
    /// column produced them. The browser only carries the id and label, but
    /// activation still has to honour `allowed_types` and the
    /// choose-files/directories policy, and the child location has to be
    /// resolved against its own column rather than the now-current directory.
    browser_entries: BrowserEntries,
    /// The column browser exists but its root snapshot predates a model change
    /// made outside column view; the next entry into columns rebuilds it.
    browser_stale: bool,
    search: Entity<TextField>,
    name_field: Option<Entity<TextField>>,
    message: Option<SharedString>,
    places: Vec<FilePlace>,
    capabilities: FileProviderCapabilities,
    status_error: Option<FileError>,
    expanded: bool,
    expand_serial: u64,
    expand_animated: bool,
    hide_extension: bool,
    format: usize,
    formats: Vec<FormatOption>,
    extension_policy: ExtensionPolicy,
    state_store: Option<Rc<dyn PanelStateStore>>,
    frame_store: Option<Rc<dyn crate::window_role::FrameStore>>,
    panel_state: PanelState,
    sidebar_width: f32,
    resizing_sidebar: Option<(f32, f32)>,
    search_text: String,
    can_choose_files: bool,
    can_choose_directories: bool,
    allows_multiple: bool,
    accessory: Option<AccessoryFn>,
    on_finish: Option<FinishFn>,
    location: LocationId,
    /// A finish is in flight; prevents a second Save/Open click.
    closing: bool,
    outcome: Option<PanelOutcome>,
    delivered: bool,
    pending_rename: Option<LocationId>,
    pending_selection: Option<ItemId>,
    /// A location to select after the next successful load (a renamed entry).
    pending_select: Option<LocationId>,
    browser_navigation: bool,
    app_window: Option<AnyWindowHandle>,
    owner: Option<AnyWindowHandle>,
    title: SharedString,
    focused_once: bool,
    name_focus: Option<FocusHandle>,
    new_folder_focus: FocusHandle,
    cancel_focus: FocusHandle,
    accept_focus: FocusHandle,
    menu: Option<Entity<PopupMenu>>,
    commands: Entity<CommandRegistry>,
    context_target: Option<(LocationId, FileEntry)>,
    pending_menu: Option<(Vec<MenuEntry>, Point<gpui::Pixels>)>,
    _subscriptions: Vec<Subscription>,
    _model_subscription: Subscription,
    _session_subscription: Subscription,
}

impl FilePanel {
    /// Build the panel. Called by `begin_sheet` / `run_modal` through the
    /// `SavePanel` and `OpenPanel` builders.
    pub fn build(
        options: PanelOptions,
        provider: Rc<dyn FileProvider>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let mode = options.mode;
        let remembered = options
            .state_store
            .as_ref()
            .map_or_else(PanelState::default, |store| store.load(cx));
        let expanded = if mode == PanelMode::Open {
            true
        } else if options.expanded_set {
            options.expanded
        } else {
            remembered.expanded
        };
        let hide_extension = if options.hide_extension_set {
            options.hide_extension
        } else {
            remembered.hide_extension
        };
        let view = remembered.view;
        let capabilities = provider.capabilities();
        let model =
            cx.new(|cx| FileBrowserModel::new(provider.clone(), options.location.clone(), cx));
        model.update(cx, |model, cx| {
            model.set_allowed_types(options.allowed_types.clone());
            model.set_allows_other_types(options.allows_other_types);
            model.set_view(view, cx);
        });
        let sidebar = build_sidebar(&[], cx);
        let rename_panel: Rc<RefCell<Option<WeakEntity<FilePanel>>>> = Rc::new(RefCell::new(None));
        let table = build_table(model.clone(), options.clone(), rename_panel.clone(), cx);
        let formats = if options.formats.is_empty() {
            if options.allowed_types.is_empty() {
                vec![FormatOption::new("Plain Text", ["txt"])]
            } else {
                options
                    .allowed_types
                    .iter()
                    .map(|extension| {
                        FormatOption::new(extension.to_uppercase(), [extension.clone()])
                    })
                    .collect()
            }
        } else {
            options.formats.clone()
        };
        // Name field.
        let name_field = (mode == PanelMode::Save).then(|| {
            let field = cx.new(|cx| {
                let mut field = TextField::new(window, cx);
                field.set_label("Name", cx);
                field.set_placeholder("Untitled", cx);
                field.set_text(options.name.clone(), cx);
                field
            });
            let stem = options.name.rfind('.').unwrap_or(options.name.len());
            let stem = if stem == 0 { options.name.len() } else { stem };
            field.update(cx, |field, cx| {
                crate::text_client::select_range(field, 0..stem, window, cx);
            });
            field
        });
        let search = cx.new(|cx| {
            let mut field = TextField::new(window, cx);
            field.set_search(true, cx);
            field.set_label("Search", cx);
            field.set_placeholder("Search", cx);
            field.set_size(ControlSize::Small, cx);
            field
        });
        let session = cx.new(|_| ModalSession::new(ModalScope::Document));
        let commands = cx.new(|_| CommandRegistry::default());
        let model_for_observe = model.clone();
        let session_for_observe = session.clone();
        let panel = cx.new(|cx| Self {
            mode,
            model,
            session,
            sidebar,
            table,
            icons: None,
            browser: None,
            browser_entries: HashMap::new(),
            browser_stale: false,
            search,
            name_field,
            message: options.message,
            places: Vec::new(),
            capabilities,
            status_error: None,
            expanded,
            expand_serial: 0,
            expand_animated: false,
            hide_extension,
            format: 0,
            formats,
            extension_policy: options.extension_policy.clone(),
            state_store: options.state_store.clone(),
            frame_store: options.frame_store.clone(),
            panel_state: PanelState {
                view,
                expanded,
                hide_extension,
                sidebar_width: remembered.sidebar_width,
            },
            sidebar_width: remembered.sidebar_width,
            resizing_sidebar: None,
            search_text: String::new(),
            can_choose_files: options.can_choose_files,
            can_choose_directories: options.can_choose_directories,
            allows_multiple: options.allows_multiple,
            accessory: options.accessory,
            on_finish: options.on_finish,
            location: options.location,
            closing: false,
            outcome: None,
            delivered: false,
            pending_rename: None,
            pending_selection: None,
            pending_select: None,
            browser_navigation: false,
            app_window: None,
            owner: None,
            title: "Save".into(),
            focused_once: false,
            name_focus: None,
            new_folder_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            accept_focus: cx.focus_handle(),
            menu: None,
            commands,
            context_target: None,
            pending_menu: None,
            _subscriptions: Vec::new(),
            _model_subscription: cx.observe(&model_for_observe, |_: &mut Self, _, cx| cx.notify()),
            _session_subscription: cx.observe(&session_for_observe, |panel, session, cx| {
                if session.read(cx).is_ended() {
                    panel.deliver(cx);
                }
            }),
        });
        *rename_panel.borrow_mut() = Some(panel.downgrade());
        panel.update(cx, |panel, cx| panel.wire(window, cx));
        panel.update(cx, |panel, cx| panel.load_places(cx));
        let weak = panel.downgrade();
        panel.update(cx, |panel, cx| panel.install_context_menu(weak, window, cx));
        panel
    }

    fn load_places(&mut self, cx: &mut Context<Self>) {
        let provider = self.model.read(cx).provider().clone();
        let task = provider.places(cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| match result {
                Ok(places) => panel.set_places(places, cx),
                Err(error) => {
                    panel.status_error = Some(error);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn set_places(&mut self, places: Vec<FilePlace>, cx: &mut Context<Self>) {
        self.places = places;
        let roots = sidebar_roots(&self.places);
        // A glyph is a promise. Only a provider that advertises ejection gets
        // an actionable accessory; the rest show no eject control at all.
        let eject: Vec<(String, String)> = if self.capabilities.eject {
            self.places
                .iter()
                .filter(|place| place.eject)
                .map(|place| (place.id.to_string(), place.label.to_string()))
                .collect()
        } else {
            Vec::new()
        };
        let location = self.location.clone();
        let weak = cx.entity().downgrade();
        self.sidebar.update(cx, |sidebar, cx| {
            if sidebar.replace(roots, cx).is_err() {
                return;
            }
            sidebar.set_row_accessories(
                move |row| {
                    let Some((id, label)) = eject.iter().find(|(id, _)| id == &row.id).cloned()
                    else {
                        return Vec::new();
                    };
                    let id: LocationId = id.into();
                    let weak = weak.clone();
                    vec![RowAccessory::button(
                        format!("eject-{id}"),
                        format!("Eject {label}"),
                        "\u{23CF}",
                        move |_, cx| {
                            let Some(panel) = weak.upgrade() else {
                                return;
                            };
                            let id = id.clone();
                            panel.update(cx, |panel, cx| panel.eject_place(id, cx));
                        },
                    )]
                },
                cx,
            );
            sidebar.select(&location, cx);
        });
        cx.notify();
    }

    fn eject_place(&mut self, location: LocationId, cx: &mut Context<Self>) {
        let provider = self.model.read(cx).provider().clone();
        let task = provider.eject(&location, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| match result {
                Ok(()) => {
                    panel.status_error = None;
                    panel.load_places(cx);
                }
                Err(error) => {
                    // The device must stay visible when ejection fails.
                    panel.status_error = Some(error);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn wire(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let model = self.model.clone();
        self._subscriptions.push(
            cx.subscribe(&model, |panel, _, event: &FileBrowserEvent, cx| {
                panel.model_event(event, cx)
            }),
        );
        let table = self.table.clone();
        self._subscriptions.push(
            cx.subscribe_in(&table, window, |panel, _, event, window, cx| match event {
                TableEvent::Activated(_) => panel.table_activated(cx),
                TableEvent::SelectionChanged(_) => cx.notify(),
                TableEvent::ContextMenu { ids, position } => {
                    panel.file_context(ids, *position, window, cx)
                }
                TableEvent::BackgroundContextMenu { position } => {
                    panel.background_context(*position, window, cx)
                }
                TableEvent::Sorted { column, direction } => {
                    panel.table_sorted(column, *direction, cx)
                }
                _ => {}
            }),
        );
        let sidebar = self.sidebar.clone();
        self._subscriptions.push(cx.subscribe(
            &sidebar,
            |panel, _, event: &SourceListEvent, cx| {
                if let SourceListEvent::Selected(id) = event {
                    if !id.starts_with("search:") {
                        panel.navigate(id, false, cx);
                    }
                }
            },
        ));
        let search = self.search.clone();
        self._subscriptions.push(
            cx.subscribe(&search, |panel, _, event: &TextFieldEvent, cx| {
                if matches!(event, TextFieldEvent::Changed) {
                    panel.search_text = panel.search.read(cx).text().to_lowercase();
                    panel.apply_search_filter(cx);
                }
            }),
        );
        if let Some(field) = &self.name_field {
            let field = field.clone();
            self._subscriptions.push(cx.subscribe(
                &field,
                |panel, _, event: &TextFieldEvent, cx| match event {
                    TextFieldEvent::Submitted => panel.save_clicked(cx),
                    TextFieldEvent::Changed => {
                        panel.status_error = None;
                        cx.notify();
                    }
                    TextFieldEvent::EndedEditing => cx.notify(),
                },
            ));
        }
        if let Some(field) = &self.name_field {
            let handle = field.read(cx).focus_handle(cx);
            self.name_focus = Some(handle);
        }
        let location = self.location.clone();
        self.sidebar
            .update(cx, |sidebar, cx| sidebar.select(&location, cx));
        self.refresh_stops(cx);
    }

    fn install_context_menu(
        &mut self,
        weak: WeakEntity<FilePanel>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let commands = self.commands.clone();
        let rows = [
            ("file.open", "Open"),
            ("file.rename", "Rename"),
            ("file.copy-path", "Copy Path"),
            ("file.trash", "Move to Trash"),
            ("file.delete-permanently", "Delete Immediately\u{2026}"),
            ("file.new-folder", "New Folder"),
            ("file.sort.name", "Name"),
            ("file.sort.kind", "Kind"),
            ("file.sort.modified", "Date Modified"),
            ("file.sort.size", "Size"),
            ("file.view.icons", "as Icons"),
            ("file.view.list", "as List"),
            ("file.view.columns", "as Columns"),
            ("file.show-hidden", "Show Hidden Files"),
        ];
        for (id, label) in rows {
            let weak = weak.clone();
            commands.update(cx, |registry, cx| {
                registry
                    .register(
                        CommandDescriptor::action(id, label),
                        move |id, window, cx| {
                            if let Some(panel) = weak.upgrade() {
                                panel.update(cx, |panel, cx| {
                                    panel.context_command(id.as_str(), window, cx)
                                });
                            }
                        },
                        cx,
                    )
                    .expect("unique panel context command");
            });
        }
        let menu = cx.new(|cx| {
            PopupMenu::new(
                MenuModel {
                    label: "File".into(),
                    entries: Vec::new(),
                },
                commands,
                window,
                cx,
            )
        });
        self.menu = Some(menu);
    }

    fn open_menu(
        &mut self,
        entries: Vec<MenuEntry>,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = &self.menu {
            menu.update(cx, |menu, cx| {
                menu.set_model(
                    MenuModel {
                        label: "File".into(),
                        entries,
                    },
                    window,
                    cx,
                );
                menu.open(position, window, cx);
            });
        }
    }

    fn context_target_for(&self, id: &ItemId, cx: &App) -> Option<(LocationId, FileEntry)> {
        self.browser_entry(id).or_else(|| {
            let model = self.model.read(cx);
            model
                .selected_entries(cx)
                .into_iter()
                .find(|entry| entry.id == *id)
                .map(|entry| (model.location().clone(), entry))
        })
    }

    fn file_menu_entries(
        &self,
        parent: &LocationId,
        entry: &FileEntry,
        cx: &App,
    ) -> Vec<MenuEntry> {
        let caps = self.capabilities;
        let mut entries = vec![MenuEntry::Heading(entry.name.to_string())];
        if self.mode == PanelMode::Open {
            entries.push(MenuEntry::Command("file.open".into()));
        }
        if caps.rename {
            entries.push(MenuEntry::Command("file.rename".into()));
        }
        if caps.copy_path && self.model.read(cx).path_of_child(parent, entry).is_some() {
            entries.push(MenuEntry::Command("file.copy-path".into()));
        }
        if caps.trash || caps.delete_permanently {
            entries.push(MenuEntry::Separator);
            if caps.trash {
                entries.push(MenuEntry::Command("file.trash".into()));
            }
            if caps.delete_permanently {
                entries.push(MenuEntry::Command("file.delete-permanently".into()));
            }
        }
        if caps.create_folder {
            entries.push(MenuEntry::Separator);
            entries.push(MenuEntry::Command("file.new-folder".into()));
        }
        entries
    }

    fn file_context(
        &mut self,
        ids: &[ItemId],
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = ids.first() else {
            self.background_context(position, window, cx);
            return;
        };
        let Some(target) = self.context_target_for(id, cx) else {
            return;
        };
        self.context_target = Some(target.clone());
        let (parent, entry) = target;
        let entries = self.file_menu_entries(&parent, &entry, cx);
        self.open_menu(entries, position, window, cx);
    }

    fn queue_file_context(
        &mut self,
        ids: &[ItemId],
        position: Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(id) = ids.first() else {
            self.queue_background_context(position, cx);
            return;
        };
        let Some(target) = self.context_target_for(id, cx) else {
            return;
        };
        self.context_target = Some(target.clone());
        let (parent, entry) = target;
        let entries = self.file_menu_entries(&parent, &entry, cx);
        self.pending_menu = Some((entries, position));
        cx.notify();
    }

    fn queue_background_context(&mut self, position: Point<gpui::Pixels>, cx: &mut Context<Self>) {
        self.context_target = None;
        self.pending_menu = Some((self.background_menu_entries(), position));
        cx.notify();
    }

    fn background_menu_entries(&self) -> Vec<MenuEntry> {
        vec![
            MenuEntry::Command("file.new-folder".into()),
            MenuEntry::Separator,
            MenuEntry::Submenu {
                label: "Sort By".into(),
                entries: vec![
                    MenuEntry::Command("file.sort.name".into()),
                    MenuEntry::Command("file.sort.kind".into()),
                    MenuEntry::Command("file.sort.modified".into()),
                    MenuEntry::Command("file.sort.size".into()),
                ],
            },
            MenuEntry::Submenu {
                label: "View".into(),
                entries: vec![
                    MenuEntry::Command("file.view.icons".into()),
                    MenuEntry::Command("file.view.list".into()),
                    MenuEntry::Command("file.view.columns".into()),
                ],
            },
            MenuEntry::Separator,
            MenuEntry::Command("file.show-hidden".into()),
        ]
    }

    fn background_context(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.context_target = None;
        let entries = self.background_menu_entries();
        self.open_menu(entries, position, window, cx);
    }

    fn context_command(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        match id {
            "file.new-folder" => self.new_folder(cx),
            "file.open" => {
                if let Some((parent, entry)) = self.context_target.clone() {
                    if entry.is_dir && !entry.is_package {
                        let location = self.model.read(cx).location_of_child(&parent, &entry, cx);
                        self.navigate(location, true, cx);
                    } else if self.mode == PanelMode::Open && self.choosable_now(&entry, cx) {
                        self.finish_open(&[(parent, entry)], cx);
                    }
                }
            }
            "file.rename" => {
                if let Some((_, entry)) = self.context_target.clone() {
                    match self.model.read(cx).view() {
                        FileViewMode::Columns => {
                            if let Some(browser) = &self.browser {
                                browser.update(cx, |browser, cx| {
                                    browser.begin_editing(&entry.id, window, cx);
                                });
                            }
                        }
                        FileViewMode::Icons => {
                            self.ensure_icon_grid(cx);
                            if let Some(icons) = &self.icons {
                                icons.update(cx, |grid, cx| {
                                    grid.begin_editing(entry.id.clone(), window, cx);
                                });
                            }
                        }
                        FileViewMode::List => {
                            self.model
                                .read(cx)
                                .controller()
                                .update(cx, |controller, cx| {
                                    controller.select(entry.id.clone());
                                    cx.notify();
                                });
                            self.table.update(cx, |table, cx| {
                                table.begin_editing(entry.id.clone(), "name", window, cx)
                            });
                        }
                    }
                }
            }
            "file.trash" => {
                if let Some((parent, entry)) = self.context_target.clone() {
                    let provider = self.model.read(cx).provider().clone();
                    let task = provider.trash(&parent, &entry.id, cx);
                    self.run_removal(task, cx);
                }
            }
            "file.delete-permanently" => {
                if let Some((parent, entry)) = self.context_target.clone() {
                    self.confirm_permanent_delete(parent, entry, cx);
                }
            }
            "file.copy-path" => {
                if let Some((parent, entry)) = self.context_target.clone() {
                    if let Some(path) = self.model.read(cx).path_of_child(&parent, &entry) {
                        cx.write_to_clipboard(ClipboardItem::new_string(
                            path.display().to_string(),
                        ));
                    }
                }
            }
            "file.view.icons" => self.set_view(FileViewMode::Icons, cx),
            "file.view.list" => self.set_view(FileViewMode::List, cx),
            "file.view.columns" => self.set_view(FileViewMode::Columns, cx),
            "file.sort.name" => self.set_sort(FileSortKey::Name, cx),
            "file.sort.kind" => self.set_sort(FileSortKey::Kind, cx),
            "file.sort.modified" => self.set_sort(FileSortKey::Modified, cx),
            "file.sort.size" => self.set_sort(FileSortKey::Size, cx),
            "file.show-hidden" => {
                let show = !self.model.read(cx).show_hidden();
                self.model
                    .update(cx, |model, cx| model.set_show_hidden(show, cx));
            }
            _ => {}
        }
    }

    fn run_removal(&mut self, task: Task<Result<(), FileError>>, cx: &mut Context<Self>) {
        let model = self.model.clone();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| {
                panel.context_target = None;
                panel.status_error = None;
                if let Err(error) = result {
                    panel.status_error = Some(error);
                }
                model.update(cx, |model, cx| model.reload(cx));
                cx.notify();
            });
        })
        .detach();
    }

    /// One rename implementation for the table, icon grid and column browser.
    /// The widgets report intent only; the panel validates, runs the provider
    /// task, and reconciles the result. Returns whether the edit was accepted.
    fn commit_rename(
        &mut self,
        parent: LocationId,
        id: ItemId,
        new_name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let provider = self.model.read(cx).provider().clone();
        if !self.capabilities.rename || new_name.trim().is_empty() {
            return false;
        }
        if let Err(error) = provider.validate_name(&parent, new_name, cx) {
            self.status_error = Some(error);
            cx.notify();
            return false;
        }
        let old_name = self.entry_for_id(&id, cx).map(|entry| entry.name);
        if old_name.as_deref() == Some(new_name) {
            return false;
        }
        let resulting_location = provider.child_location(&parent, new_name);
        let name = new_name.to_string();
        let task = provider.rename(&parent, &id, &name, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| {
                panel.rename_finished(parent, resulting_location, result, cx);
            });
        })
        .detach();
        true
    }

    fn entry_for_id(&self, id: &ItemId, cx: &App) -> Option<FileEntry> {
        if let Some((_, entry)) = self.browser_entries.get(id) {
            return Some(entry.clone());
        }
        let controller = self.model.read(cx).controller();
        let controller = controller.read(cx);
        controller
            .index_of(id)
            .and_then(|index| controller.item(index))
            .cloned()
    }

    fn rename_finished(
        &mut self,
        parent: LocationId,
        resulting_location: Option<LocationId>,
        result: Result<(), FileError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(()) => {
                self.status_error = None;
                self.pending_select = resulting_location;
                let current = self.model.read(cx).location().clone();
                if parent == current {
                    self.model.update(cx, |model, cx| model.reload(cx));
                } else if self.model.read(cx).view() == FileViewMode::Columns {
                    // A renamed child of a non-current column: refresh that
                    // branch instead of reloading the current directory.
                    let branch = self
                        .browser_entries
                        .iter()
                        .find(|(_, (_, entry))| entry.location == parent && entry.is_dir)
                        .map(|(id, _)| id.clone());
                    if let Some(branch) = branch {
                        self.browser_children(branch, cx);
                    }
                }
            }
            Err(error) => {
                // Keep the current selection coherent: the file did not move,
                // so the row and its id stay valid and the name snaps back.
                self.pending_select = None;
                self.status_error = Some(error);
                cx.notify();
            }
        }
    }

    fn confirm_permanent_delete(
        &mut self,
        parent: LocationId,
        entry: FileEntry,
        cx: &mut Context<Self>,
    ) {
        let Some(owner) = self.owner() else {
            return;
        };
        let panel = cx.entity().downgrade();
        let name = entry.name.to_string();
        let alert = Alert::new(format!("Delete \u{201C}{name}\u{201D} immediately?"))
            .message("This cannot be undone.")
            .window_title("Delete")
            .button(AlertButton::new("alert-cancel", "Cancel").role(AlertRole::Cancel))
            .button(AlertButton::new("alert-delete", "Delete").role(AlertRole::Destructive))
            .on_result(move |result: &AlertResult, _, cx| {
                if result.choice.as_deref() != Some("alert-delete") {
                    return;
                }
                if let Some(panel) = panel.upgrade() {
                    panel.update(cx, |panel, cx| {
                        let provider = panel.model.read(cx).provider().clone();
                        let task = provider.delete_permanently(&parent, &entry.id, cx);
                        panel.run_removal(task, cx);
                    });
                }
            });
        let owner_for_alert = owner;
        cx.defer(move |cx| {
            let _ = owner_for_alert.update(cx, |_, window, cx| {
                alert.run_modal(window, cx);
            });
        });
    }

    /// Apply a context-menu sort. Choosing the current key again reverses it;
    /// choosing a new key uses that key's natural first direction.
    fn set_sort(&mut self, key: FileSortKey, cx: &mut Context<Self>) {
        let sort = match self.model.read(cx).sort() {
            Some(current) if current.key == key => current.toggled(),
            Some(_) | None => FileSort::new(key, key.default_direction()),
        };
        self.apply_sort(sort, cx);
    }

    /// A header click already decided the direction; apply it to the one panel
    /// sort model. The table's indicator is set by `apply_sort`.
    fn table_sorted(&mut self, column: &str, direction: SortDirection, cx: &mut Context<Self>) {
        let Some(key) = FileSortKey::from_column_id(column) else {
            return;
        };
        self.apply_sort(FileSort::new(key, direction), cx);
    }

    fn apply_sort(&mut self, sort: FileSort, cx: &mut Context<Self>) {
        self.model
            .update(cx, |model, cx| model.set_sort(Some(sort), cx));
        self.table.update(cx, |table, cx| {
            table.show_sort(sort.key.column_id(), sort.direction, cx);
        });
        if self.model.read(cx).view() == FileViewMode::Columns {
            self.rebuild_browser(cx);
        } else if self.browser.is_some() {
            self.browser_stale = true;
        }
        cx.notify();
    }

    fn model_event(&mut self, event: &FileBrowserEvent, cx: &mut Context<Self>) {
        match event {
            FileBrowserEvent::LocationChanged { location } => {
                self.location = location.clone();
                self.sidebar
                    .update(cx, |sidebar, cx| sidebar.select(location, cx));
                if self.model.read(cx).view() == FileViewMode::Columns && !self.browser_navigation {
                    self.rebuild_browser(cx);
                }
            }
            FileBrowserEvent::Loaded { .. } => {
                if self.pending_rename.is_some() {
                    cx.notify();
                }
                if self.model.read(cx).view() == FileViewMode::Columns
                    && !self.browser_navigation
                    && self.pending_rename.is_none()
                {
                    self.rebuild_browser(cx);
                } else if self.browser.is_some() {
                    self.browser_stale = true;
                }
                if let Some(id) = self.pending_selection.take() {
                    self.apply_selection(&id, cx);
                }
                if let Some(location) = self.pending_select.take() {
                    if let Some(id) = self.entry_id_for_location(&location, cx) {
                        self.apply_selection(&id, cx);
                    }
                }
                self.browser_navigation = false;
            }
            FileBrowserEvent::Loading { .. } | FileBrowserEvent::Failed { .. } => {}
            FileBrowserEvent::ViewChanged { .. } => {
                match self.model.read(cx).view() {
                    FileViewMode::Icons => self.ensure_icon_grid(cx),
                    FileViewMode::Columns => self.ensure_browser(cx),
                    FileViewMode::List => {}
                }
                self.save_panel_state(cx);
            }
        }
        cx.notify();
    }

    fn navigate(
        &mut self,
        location: impl Into<LocationId>,
        from_browser: bool,
        cx: &mut Context<Self>,
    ) {
        self.status_error = None;
        self.browser_navigation = from_browser;
        self.model
            .update(cx, |model, cx| model.navigate(location.into(), cx));
        cx.notify();
    }

    fn apply_search_filter(&mut self, cx: &mut Context<Self>) {
        let query = self.search_text.clone();
        let controller = self.model.read(cx).controller();
        controller.update(cx, |controller, cx| {
            if query.is_empty() {
                controller.set_filter(None::<fn(&FileEntry) -> bool>);
            } else {
                controller.set_filter(Some(move |entry: &FileEntry| {
                    entry.name_key().contains(&query)
                }));
            }
            cx.notify();
        });
        cx.notify();
    }

    fn refresh_stops(&self, cx: &mut Context<Self>) {
        let mut stops = Vec::new();
        let browser_stop = || match self.model.read(cx).view() {
            FileViewMode::List => self.table.read(cx).focus_handle(cx),
            FileViewMode::Icons => self.icons.as_ref().map_or_else(
                || self.table.read(cx).focus_handle(cx),
                |icons| icons.read(cx).focus_handle(),
            ),
            FileViewMode::Columns => self
                .browser
                .as_ref()
                .and_then(|browser| browser.read(cx).column_table(0))
                .map_or_else(
                    || self.table.read(cx).focus_handle(cx),
                    |table| table.read(cx).focus_handle(cx),
                ),
        };
        match (self.mode, self.expanded) {
            // HIG p. 248: Save As field -> source list -> visible columns ->
            // back to the field.
            (PanelMode::Save, true) => {
                if let Some(name) = &self.name_focus {
                    stops.push(name.clone());
                }
                stops.push(self.sidebar.read(cx).focus_handle(cx));
                stops.push(browser_stop());
            }
            (PanelMode::Save, false) => {
                if let Some(name) = &self.name_focus {
                    stops.push(name.clone());
                }
                stops.push(self.cancel_focus.clone());
                stops.push(self.accept_focus.clone());
            }
            (PanelMode::Open, _) => {
                stops.push(self.sidebar.read(cx).focus_handle(cx));
                stops.push(browser_stop());
                stops.push(self.search.read(cx).focus_handle(cx));
                stops.push(self.cancel_focus.clone());
                stops.push(self.accept_focus.clone());
            }
        }
        self.session
            .update(cx, |session, _| session.set_focus_stops(stops));
    }

    fn table_activated(&mut self, cx: &mut Context<Self>) {
        if self.model.read(cx).view() == FileViewMode::Columns {
            let Some((parent, entry)) = self.browser_target(cx) else {
                return;
            };
            if entry.is_dir && !entry.is_package {
                let location = self.model.read(cx).location_of_child(&parent, &entry, cx);
                if self.can_choose_directories {
                    self.finish_open(&[(parent, entry)], cx);
                } else {
                    self.navigate(location, true, cx);
                }
            } else if self.mode == PanelMode::Open && self.choosable_now(&entry, cx) {
                self.finish_open(&[(parent, entry)], cx);
            }
            return;
        }
        let selected = self.model.read(cx).selected_entries(cx);
        let Some(entry) = selected.first() else {
            return;
        };
        if entry.is_dir && !entry.is_package {
            let location = self.model.read(cx).location_of_entry(entry, cx);
            self.navigate(location, false, cx);
            return;
        }
        if self.mode == PanelMode::Open {
            self.open_clicked(cx);
        }
    }

    fn accept_focused(&mut self, cx: &mut Context<Self>) {
        match self.mode {
            PanelMode::Save => self.save_clicked(cx),
            PanelMode::Open => self.open_clicked(cx),
        }
    }

    fn name(&self, cx: &App) -> String {
        self.name_field
            .as_ref()
            .map(|field| field.read(cx).text().trim().to_string())
            .unwrap_or_default()
    }

    /// The extension `Required` appends and accepts: the current Format's
    /// first extension, or the model's first allowed type when the consumer
    /// set only `allowed_types`.
    fn required_extension(&self, cx: &App) -> Option<String> {
        self.formats
            .get(self.format)
            .and_then(|format| format.extensions.first())
            .or_else(|| self.model.read(cx).allowed_types().first())
            .map(|extension| extension.trim_start_matches('.').to_string())
    }

    /// The name the provider will receive after the panel's extension policy.
    fn resolved_name(&self, cx: &App) -> String {
        let typed = self.name(cx);
        if self.mode != PanelMode::Save || typed.is_empty() {
            return typed;
        }
        let extension = |name: &str| {
            name.rsplit_once('.')
                .filter(|(stem, ext)| !stem.is_empty() && !ext.is_empty())
                .map(|(_, ext)| ext.to_lowercase())
        };
        match &self.extension_policy {
            ExtensionPolicy::Any => typed,
            ExtensionPolicy::Required => {
                if extension(&typed).is_some() {
                    typed
                } else if let Some(first) = self.required_extension(cx) {
                    format!("{typed}.{first}")
                } else {
                    typed
                }
            }
            ExtensionPolicy::Fixed(wanted) => {
                let wanted = wanted.trim_start_matches('.').to_lowercase();
                if extension(&typed).as_deref() == Some(wanted.as_str()) {
                    typed
                } else {
                    let stem = typed
                        .rsplit_once('.')
                        .filter(|(stem, _)| !stem.is_empty())
                        .map_or(typed.as_str(), |(stem, _)| stem);
                    format!("{stem}.{wanted}")
                }
            }
        }
    }

    /// The first reason the current Save name is not acceptable, if any. The
    /// panel owns the extension policy and the structural rules; the provider
    /// owns host-specific rules such as reserved device names.
    fn name_error(&self, cx: &App) -> Option<FileError> {
        if self.mode != PanelMode::Save {
            return None;
        }
        let typed = self.name(cx);
        if typed.is_empty() {
            return None;
        }
        let name = self.resolved_name(cx);
        if name == "." || name == ".." {
            return Some(FileError::new("That name is reserved."));
        }
        if let (ExtensionPolicy::Required, PanelMode::Save) = (&self.extension_policy, self.mode) {
            if let Some(required) = self.required_extension(cx) {
                let has_required = name
                    .rsplit_once('.')
                    .filter(|(stem, _)| !stem.is_empty())
                    .is_some_and(|(_, ext)| required.eq_ignore_ascii_case(ext));
                if !has_required {
                    return Some(FileError::new(format!("The name must end in .{required}.")));
                }
            }
        }
        self.model.read(cx).validate_name(&name, cx).err()
    }

    fn name_valid(&self, cx: &App) -> bool {
        self.mode != PanelMode::Save || (!self.name(cx).is_empty() && self.name_error(cx).is_none())
    }

    fn save_clicked(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.mode != PanelMode::Save {
            return;
        }
        if !self.name_valid(cx) {
            self.status_error = self.name_error(cx);
            cx.notify();
            return;
        }
        let name = self.resolved_name(cx);
        let model = self.model.clone();
        let task = model.update(cx, |model, cx| model.exists(&name, cx));
        self.closing = true;
        self.status_error = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| {
                panel.closing = false;
                match result {
                    Ok(true) => panel.show_replace(&name, cx),
                    Ok(false) => panel.accept_save(&name, SaveDisposition::New, cx),
                    Err(error) => {
                        // An IO failure is not "the target is missing"; keep
                        // the panel open and report it.
                        panel.status_error = Some(error);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn show_replace(&mut self, name: &str, cx: &mut Context<Self>) {
        // Never overwrite without the alert. If no window can host it, leave
        // the panel open (the session blocks a second Save meanwhile).
        let Some(owner) = self.owner() else {
            self.closing = false;
            cx.notify();
            return;
        };
        // HIG p. 251 and windows.md: the collision alert is application-modal.
        // It must not be a second sheet over the Save sheet (p. 236).
        let panel = cx.entity().downgrade();
        let name = name.to_string();
        let alert = Alert::replace_existing(name.clone())
            .window_title("Replace")
            .on_result(move |result: &AlertResult, _, cx| {
                if result.choice.as_deref() == Some("alert-replace") {
                    if let Some(panel) = panel.upgrade() {
                        panel.update(cx, |panel, cx| {
                            panel.accept_save(&name, SaveDisposition::ReplaceConfirmed, cx);
                        });
                    }
                }
            });
        let _ = owner.update(cx, |_, window, cx| {
            alert.run_modal(window, cx);
        });
    }

    fn accept_save(&mut self, name: &str, disposition: SaveDisposition, cx: &mut Context<Self>) {
        let (location, path) = {
            let model = self.model.read(cx);
            (model.child_location(name), model.path_for(name))
        };
        let outcome = PanelOutcome {
            result: ModalResult::Accepted,
            paths: path.into_iter().collect(),
            locations: location.into_iter().collect(),
            hide_extension: self.hide_extension,
            disposition: Some(disposition),
        };
        self.finish(outcome, cx);
    }

    fn open_clicked(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.mode != PanelMode::Open {
            return;
        }
        if self.model.read(cx).view() == FileViewMode::Columns {
            if let Some((parent, entry)) = self.browser_target(cx) {
                if self.choosable_now(&entry, cx) {
                    self.finish_open(&[(parent, entry)], cx);
                }
            }
            return;
        }
        let selected = self.model.read(cx).selected_entries(cx);
        if selected.is_empty()
            || (!self.allows_multiple && selected.len() > 1)
            || !selected.iter().all(|entry| {
                choosable(entry, self.can_choose_files, self.can_choose_directories)
                    && (entry.is_dir || self.model.read(cx).matches_types(entry))
            })
        {
            if selected.len() == 1 && selected[0].is_dir && !selected[0].is_package {
                let location = self.model.read(cx).location_of_entry(&selected[0], cx);
                self.navigate(location, false, cx);
            }
            return;
        }
        let parent = self.model.read(cx).location().clone();
        let targets: Vec<(LocationId, FileEntry)> = selected
            .into_iter()
            .map(|entry| (parent.clone(), entry))
            .collect();
        self.finish_open(&targets, cx);
    }

    fn new_folder(&mut self, cx: &mut Context<Self>) {
        if !self.capabilities.create_folder {
            return;
        }
        let entries = self.model.read(cx).controller().read(cx).visible_items();
        let base = "untitled folder".to_string();
        let mut name = base.clone();
        let mut counter = 2;
        while entries.iter().any(|entry| entry.name == name) {
            name = format!("{base} {counter}");
            counter += 1;
        }
        let model = self.model.clone();
        let task = model.update(cx, |model, cx| model.create_folder(&name, cx));
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| match result {
                Ok(location) => {
                    panel.pending_rename = Some(location);
                    panel.model.update(cx, |model, cx| model.reload(cx));
                    cx.notify();
                }
                Err(error) => {
                    panel.status_error = Some(error);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !sheet::dismiss(self.session.clone(), ModalResult::Rejected, window, cx) {
            modal::finish(self.session.clone(), ModalResult::Rejected, window, cx);
        }
    }

    fn finish(&mut self, outcome: PanelOutcome, cx: &mut Context<Self>) {
        self.outcome = Some(outcome);
        self.closing = true;
        if let Some(handle) = self.owner.or(self.app_window) {
            let session = self.session.clone();
            let accepted = self
                .outcome
                .as_ref()
                .is_some_and(|outcome| outcome.result == ModalResult::Accepted);
            let result = if accepted {
                ModalResult::Accepted
            } else {
                ModalResult::Rejected
            };
            // The panel is often finishing from inside its own window update;
            // defer the modal finish so re-entering that window is safe.
            let sheet_owned = self.owner.is_some();
            cx.defer(move |cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    if session.read(cx).is_ended() {
                        return;
                    }
                    if sheet_owned && sheet::dismiss(session.clone(), result, window, cx) {
                        return;
                    }
                    modal::finish(session, result, window, cx);
                });
            });
        }
        cx.notify();
    }

    fn deliver(&mut self, cx: &mut Context<Self>) {
        if self.delivered {
            return;
        }
        let result = self.session.read(cx).phase();
        if !matches!(result, ModalPhase::Ended(_)) {
            return;
        }
        self.delivered = true;
        let modal_result = match result {
            ModalPhase::Ended(result) => result,
            _ => ModalResult::Cancelled,
        };
        let outcome = self.outcome.take().unwrap_or(PanelOutcome {
            result: modal_result,
            paths: Vec::new(),
            locations: Vec::new(),
            hide_extension: self.hide_extension,
            disposition: None,
        });
        let outcome = PanelOutcome {
            result: modal_result,
            ..outcome
        };
        if let Some(window) = self.app_window {
            let store = self.frame_store.clone();
            let _ = window.update(cx, |_, window, cx| {
                if let Some(store) = &store {
                    crate::window_role::save_frame(
                        WindowRole::OpenPanel,
                        window,
                        store.as_ref(),
                        cx,
                    );
                }
                window.remove_window();
            });
        }
        if let Some(f) = &self.on_finish {
            f(&outcome, cx);
        }
        cx.notify();
    }

    fn owner(&self) -> Option<AnyWindowHandle> {
        self.owner.or(self.app_window)
    }

    // --- presentation ------------------------------------------------------

    fn ensure_browser(&mut self, cx: &mut Context<Self>) {
        if self.browser.is_some() {
            if !self.browser_stale {
                return;
            }
            self.browser = None;
            self.browser_entries.clear();
            self.browser_stale = false;
        }
        let mut entries: Vec<FileEntry> = self
            .model
            .read(cx)
            .controller()
            .read(cx)
            .visible_items()
            .into_iter()
            .cloned()
            .collect();
        self.sort_entries(&mut entries, cx);
        self.browser_entries.clear();
        let parent = self.model.read(cx).location().clone();
        for entry in &entries {
            self.browser_entries
                .insert(entry.id.clone(), (parent.clone(), entry.clone()));
        }
        let nodes: Vec<BrowserNode> = entries
            .iter()
            .map(|entry| browser_node(entry, cx))
            .collect();
        let browser = cx.new(|cx| ColumnBrowser::new(nodes, cx));
        let weak = cx.entity().downgrade();
        browser.update(cx, |browser, cx| {
            browser.set_rename(
                move |parent, id, new_name, cx| {
                    let Some(panel) = weak.upgrade() else {
                        return false;
                    };
                    let parent = parent.clone();
                    let id = id.clone();
                    let name = new_name.to_string();
                    panel.update(cx, |panel, cx| panel.commit_rename(parent, id, &name, cx))
                },
                cx,
            );
        });
        if self.mode == PanelMode::Open {
            let weak = cx.entity().downgrade();
            browser.update(cx, |browser, cx| {
                browser.set_preview(
                    move |node, window, cx| {
                        let entry = weak
                            .upgrade()
                            .and_then(|panel| panel.read(cx).browser_entry(&node.id))
                            .map(|(_, entry)| entry);
                        file_preview(entry.as_ref(), window, cx)
                    },
                    cx,
                );
            });
        }
        self.browser = Some(browser.clone());
        let browser_for_events = browser.clone();
        let subscription = cx.subscribe(
            &browser_for_events,
            |panel, _, event: &BrowserEvent, cx| match event {
                BrowserEvent::NeedsChildren(parent) => panel.browser_children(parent.clone(), cx),
                BrowserEvent::PathChanged(path) => {
                    if let Some(last) = path.last() {
                        // A file row is not a location. Only a branch changes
                        // the current directory; selecting a file leaves the
                        // toolbar and the model where they are.
                        let branch = panel
                            .browser_entries
                            .get(last)
                            .is_some_and(|(_, entry)| entry.is_dir && !entry.is_package);
                        if branch {
                            panel.navigate(last.clone(), true, cx);
                        }
                    }
                }
                BrowserEvent::Activated(id) => {
                    panel.browser_activated(id, cx);
                }
                BrowserEvent::ContextMenu { ids, position } => {
                    panel.queue_file_context(ids, *position, cx);
                }
                BrowserEvent::BackgroundContextMenu { position } => {
                    panel.queue_background_context(*position, cx);
                }
            },
        );
        self._subscriptions.push(subscription);
        self.refresh_stops(cx);
    }

    fn rebuild_browser(&mut self, cx: &mut Context<Self>) {
        self.browser = None;
        self.browser_stale = false;
        self.ensure_browser(cx);
        cx.notify();
    }

    fn ensure_icon_grid(&mut self, cx: &mut Context<Self>) {
        if self.icons.is_some() {
            return;
        }
        let controller = self.model.read(cx).controller();
        let icon_model = self.model.clone();
        let icons = cx.new(|cx| {
            IconGrid::new(
                controller,
                move |entry: &FileEntry, selected, _active, window, cx| {
                    let dim = !icon_model.read(cx).matches_types(entry);
                    entry.icon.file_element(
                        32.0,
                        icons::FileIconState {
                            selected,
                            inactive: !window.is_window_active(),
                            disabled: dim,
                        },
                    )
                },
                |entry: &FileEntry| entry.name.clone(),
                cx,
            )
        });
        let weak = cx.entity().downgrade();
        let rename_model = self.model.clone();
        icons.update(cx, |grid, cx| {
            grid.set_multiple(self.allows_multiple, cx);
            grid.set_rename(
                move |entry: &FileEntry, new_name: &str, cx: &mut App| {
                    let Some(panel) = weak.upgrade() else {
                        return false;
                    };
                    let parent = rename_model.read(cx).location().clone();
                    let id = entry.id.clone();
                    let name = new_name.to_string();
                    panel.update(cx, |panel, cx| panel.commit_rename(parent, id, &name, cx))
                },
                cx,
            );
        });
        let subscription =
            cx.subscribe(&icons, |panel, _, event: &IconGridEvent, cx| match event {
                IconGridEvent::SelectionChanged => cx.notify(),
                IconGridEvent::Activated(_) => panel.table_activated(cx),
                IconGridEvent::ContextMenu { ids, position } => {
                    panel.queue_file_context(ids, *position, cx);
                }
                IconGridEvent::BackgroundContextMenu { position } => {
                    panel.queue_background_context(*position, cx);
                }
            });
        self._subscriptions.push(subscription);
        self.icons = Some(icons);
        self.refresh_stops(cx);
    }

    fn sort_entries(&self, entries: &mut [FileEntry], cx: &App) {
        self.model.read(cx).sort_entries(entries);
    }

    fn save_panel_state(&mut self, cx: &mut Context<Self>) {
        self.panel_state = PanelState {
            view: self.model.read(cx).view(),
            expanded: self.expanded,
            hide_extension: self.hide_extension,
            sidebar_width: self.sidebar_width,
        };
        if let Some(store) = &self.state_store {
            store.save(self.panel_state, cx);
        }
    }

    fn choose_format(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(format) = self.formats.get(index).cloned() else {
            return;
        };
        self.format = index;
        self.model
            .update(cx, |model, _| model.set_allowed_types(format.extensions));
        cx.notify();
    }

    fn browser_children(&mut self, node: ItemId, cx: &mut Context<Self>) {
        let Some((parent, entry)) = self.browser_entry(&node) else {
            return;
        };
        let (location, provider) = {
            let model = self.model.read(cx);
            (
                model.location_of_child(&parent, &entry, cx),
                model.provider().clone(),
            )
        };
        let task = provider.children(&location, cx);
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| {
                let Some(browser) = panel.browser.clone() else {
                    return;
                };
                match result {
                    Ok(listing) => {
                        let FileListing {
                            mut entries,
                            warnings,
                        } = listing;
                        if let Some(warning) = warnings.into_iter().next() {
                            panel.status_error = Some(warning);
                        }
                        panel.sort_entries(&mut entries, cx);
                        let enrichment = provider.metadata_enrichment(&entries, cx);
                        let ids: Vec<ItemId> =
                            entries.iter().map(|entry| entry.id.clone()).collect();
                        for entry in &entries {
                            panel
                                .browser_entries
                                .insert(entry.id.clone(), (location.clone(), entry.clone()));
                        }
                        let nodes: Vec<BrowserNode> = entries
                            .iter()
                            .map(|entry| browser_node(entry, cx))
                            .collect();
                        browser.update(cx, |browser, cx| {
                            browser.set_children(node.as_str(), nodes, cx)
                        });
                        panel.prune_browser_entries(cx);
                        if let Some(task) = enrichment {
                            let node = node.clone();
                            cx.spawn(async move |this, cx| {
                                let result = task.await;
                                let _ = this.update(cx, |panel, cx| {
                                    let Ok(updates) = result else {
                                        return;
                                    };
                                    for (id, metadata) in updates {
                                        if let Some((_, entry)) = panel.browser_entries.get_mut(&id)
                                        {
                                            metadata.apply(entry);
                                        }
                                    }
                                    let Some(browser) = panel.browser.clone() else {
                                        return;
                                    };
                                    let mut refreshed: Vec<FileEntry> = ids
                                        .iter()
                                        .filter_map(|id| {
                                            panel
                                                .browser_entries
                                                .get(id)
                                                .map(|(_, entry)| entry.clone())
                                        })
                                        .collect();
                                    panel.sort_entries(&mut refreshed, cx);
                                    let nodes: Vec<BrowserNode> = refreshed
                                        .iter()
                                        .map(|entry| browser_node(entry, cx))
                                        .collect();
                                    browser.update(cx, |browser, cx| {
                                        browser.set_children(node.as_str(), nodes, cx)
                                    });
                                });
                            })
                            .detach();
                        }
                    }
                    Err(error) => {
                        panel.status_error = Some(error);
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn entry_id_for_location(&self, location: &LocationId, cx: &App) -> Option<ItemId> {
        self.model
            .read(cx)
            .controller()
            .read(cx)
            .visible_items()
            .into_iter()
            .find(|entry| &entry.location == location)
            .map(|entry| entry.id.clone())
    }

    fn browser_entry(&self, id: &ItemId) -> Option<(LocationId, FileEntry)> {
        self.browser_entries.get(id).cloned()
    }

    /// Drop cold column entries once the map is over budget. Ids still shown
    /// in any column survive; reopening a cached branch re-requests its
    /// children (the browser fills them from cache first, then refreshes).
    fn prune_browser_entries(&mut self, cx: &App) {
        if self.browser_entries.len() <= MAX_BROWSER_ENTRIES {
            return;
        }
        let Some(browser) = &self.browser else {
            self.browser_entries.clear();
            return;
        };
        let visible: std::collections::HashSet<ItemId> =
            browser.read(cx).visible_ids(cx).into_iter().collect();
        self.browser_entries.retain(|id, _| visible.contains(id));
    }

    /// The selected column-browser entry, when the panel is in column mode.
    fn browser_target(&self, cx: &App) -> Option<(LocationId, FileEntry)> {
        if self.model.read(cx).view() != FileViewMode::Columns {
            return None;
        }
        let browser = self.browser.as_ref()?;
        let id = browser.read(cx).path(cx).last()?.clone();
        self.browser_entry(&id)
    }

    fn choosable_now(&self, entry: &FileEntry, cx: &App) -> bool {
        choosable(entry, self.can_choose_files, self.can_choose_directories)
            && (entry.is_dir || entry.is_package || self.model.read(cx).matches_types(entry))
    }

    fn finish_open(&mut self, targets: &[(LocationId, FileEntry)], cx: &mut Context<Self>) {
        let (paths, locations) = {
            let model = self.model.read(cx);
            let paths = targets
                .iter()
                .filter_map(|(parent, entry)| model.path_of_child(parent, entry))
                .collect();
            let locations = targets
                .iter()
                .map(|(parent, entry)| model.location_of_child(parent, entry, cx))
                .collect();
            (paths, locations)
        };
        self.finish(
            PanelOutcome {
                result: ModalResult::Accepted,
                paths,
                locations,
                hide_extension: false,
                disposition: None,
            },
            cx,
        );
    }

    fn browser_activated(&mut self, id: &ItemId, cx: &mut Context<Self>) {
        if self.mode != PanelMode::Open {
            return;
        }
        let Some((parent, entry)) = self.browser_entry(id) else {
            return;
        };
        if entry.is_dir && !entry.is_package {
            let location = self.model.read(cx).location_of_child(&parent, &entry, cx);
            if self.can_choose_directories {
                self.finish_open(&[(parent, entry)], cx);
            } else {
                self.navigate(location, true, cx);
            }
            return;
        }
        if self.choosable_now(&entry, cx) {
            self.finish_open(&[(parent, entry)], cx);
        }
    }

    fn sidebar(&self) -> Entity<SourceList> {
        self.sidebar.clone()
    }
}

fn sidebar_roots(places: &[FilePlace]) -> Vec<silvestris_core::source_list::SourceNode> {
    let mut devices = Vec::new();
    let mut locations = Vec::new();
    let mut searches = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for place in places {
        if !seen.insert(place.id.clone()) {
            continue;
        }
        let mut node = silvestris_core::source_list::SourceNode::new(
            place.id.to_string(),
            place.label.to_string(),
        );
        node.icon = Some(place.icon.role().to_string());
        match place.section {
            PlaceSection::Devices => devices.push(node),
            PlaceSection::Places => locations.push(node),
            PlaceSection::SearchFor => searches.push(node),
        }
    }
    let mut roots = Vec::new();
    if !devices.is_empty() {
        roots.push(silvestris_core::source_list::SourceNode::section(
            "devices", "Devices", devices,
        ));
    }
    if !locations.is_empty() {
        roots.push(silvestris_core::source_list::SourceNode::section(
            "places", "Places", locations,
        ));
    }
    if !searches.is_empty() {
        roots.push(silvestris_core::source_list::SourceNode::section(
            "search-for",
            "Search For",
            searches,
        ));
    }
    roots
}

fn build_sidebar(places: &[FilePlace], cx: &mut App) -> Entity<SourceList> {
    let roots = sidebar_roots(places);
    let sidebar = cx.new(|cx| SourceList::new(roots, cx).expect("unique place ids"));
    sidebar.update(cx, |sidebar, cx| {
        sidebar.style(SourceListStyle::Primary, cx);
        sidebar.set_icon_renderer(icons::source_renderer(16.0), cx);
    });
    sidebar
}

fn build_table(
    model: Entity<FileBrowserModel>,
    options: PanelOptions,
    rename_panel: Rc<RefCell<Option<WeakEntity<FilePanel>>>>,
    cx: &mut App,
) -> Entity<TableView<FileEntry>> {
    let allowed = options.allowed_types.clone();
    let allows_other = options.allows_other_types;
    let dims = move |entry: &FileEntry| {
        if entry.is_dir || allows_other {
            return false;
        }
        if allowed.is_empty() {
            return false;
        }
        !entry.matches_types(&allowed)
    };
    let name_dims = Rc::new(dims);
    let columns = vec![
        TableColumn::custom("name", "Name", move |entry: &FileEntry, cell, window, _| {
            let dim = name_dims(entry);
            let label = entry.name.clone();
            let icon = entry.icon.file_element(
                16.0,
                icons::FileIconState {
                    selected: cell.selected,
                    inactive: !window.is_window_active(),
                    disabled: dim,
                },
            );
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .overflow_hidden()
                .child(div().flex_none().child(icon))
                .child(div().truncate().child(label))
                .into_any_element()
        })
        .text_value(|entry: &FileEntry| entry.name.clone())
        .width(220.0)
        .sort_by_text()
        .editable({
            let model = model.clone();
            let rename_panel = rename_panel.clone();
            move |entry: &FileEntry, new_name: &str, cx: &mut App| {
                let weak = rename_panel.borrow().clone();
                let Some(panel) = weak.and_then(|weak| weak.upgrade()) else {
                    return false;
                };
                let parent = model.read(cx).location().clone();
                let id = entry.id.clone();
                let name = new_name.to_string();
                panel.update(cx, |panel, cx| panel.commit_rename(parent, id, &name, cx))
            }
        }),
        TableColumn::text("modified", "Date Modified", |entry: &FileEntry| {
            entry.modified_label()
        })
        .width(145.0)
        .sortable(|a: &FileEntry, b: &FileEntry| a.modified.cmp(&b.modified)),
        TableColumn::text("size", "Size", |entry: &FileEntry| entry.size_label())
            .width(64.0)
            .hidden(true)
            .align(crate::controls::ColumnAlign::Trailing)
            .sortable(|a: &FileEntry, b: &FileEntry| a.size.cmp(&b.size)),
        TableColumn::text("kind", "Kind", |entry: &FileEntry| {
            SharedString::from(entry.kind.label())
        })
        .width(80.0)
        .hidden(true)
        .sortable(|a: &FileEntry, b: &FileEntry| a.kind.cmp(&b.kind)),
    ];
    let controller = model.read(cx).controller();
    let sort = model.read(cx).sort();
    let table = cx.new(|cx| {
        let mut table = TableView::new(controller, columns, cx);
        table.set_row_height(19.0, cx);
        table.set_allows_multiple(options.allows_multiple);
        table.set_fill_width(true, cx);
        table.set_external_sort(true, cx);
        table
    });
    if let Some(sort) = sort {
        table.update(cx, |table, cx| {
            table.show_sort(sort.key.column_id(), sort.direction, cx);
        });
    }
    table
}

fn browser_node(entry: &FileEntry, _: &App) -> BrowserNode {
    let icon = entry.icon;
    let node = if entry.is_dir && !entry.is_package {
        BrowserNode::branch(entry.id.clone(), entry.name.clone())
    } else {
        BrowserNode::leaf(entry.id.clone(), entry.name.clone())
    };
    node.icon(move |window, _| {
        icon.file_element(
            16.0,
            icons::FileIconState {
                selected: false,
                inactive: !window.is_window_active(),
                disabled: false,
            },
        )
    })
}

/// The Open panel's preview column: icon, name, kind, size and date for the
/// selected leaf. This is layout, not a document renderer; a consumer that
/// wants a rich preview supplies its own accessory or a dedicated view.
fn file_preview(entry: Option<&FileEntry>, window: &mut Window, cx: &mut App) -> AnyElement {
    let theme = cx.theme().clone();
    let Some(entry) = entry else {
        return div().into_any_element();
    };
    let icon = entry.icon.file_element(
        64.0,
        icons::FileIconState {
            selected: false,
            inactive: !window.is_window_active(),
            disabled: false,
        },
    );
    let mut lines = div().flex().flex_col().gap(px(3.)).text_size(px(11.));
    lines = lines.child(
        div()
            .font_weight(gpui::FontWeight::BOLD)
            .child(entry.name.clone()),
    );
    for line in [
        entry.kind.label().to_string(),
        entry.size_label().to_string(),
        entry.modified_label().to_string(),
    ] {
        lines = lines.child(
            div()
                .text_color(hsla(theme.colors.text_secondary))
                .child(line),
        );
    }
    div()
        .debug_selector(|| "panel-preview".into())
        .flex()
        .flex_col()
        .items_center()
        .gap(px(10.))
        .p(px(8.))
        .child(icon)
        .child(div().w_full().child(lines))
        .into_any_element()
}

impl Render for FilePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            if let Some(name) = &self.name_focus {
                window.focus(name, cx);
            }
        }
        if let Some((entries, position)) = self.pending_menu.take() {
            self.open_menu(entries, position, window, cx);
        }
        if let Some(pending) = self.pending_rename.take() {
            if !self.model.read(cx).is_loading() {
                if let Some(pending_id) = self.entry_id_for_location(&pending, cx) {
                    if self.model.read(cx).view() == FileViewMode::Columns {
                        self.rebuild_browser(cx);
                        let began = self.browser.as_ref().is_some_and(|browser| {
                            browser.update(cx, |browser, cx| {
                                browser.begin_editing(&pending_id, window, cx)
                            })
                        });
                        if !began {
                            self.pending_rename = Some(pending);
                        }
                    } else {
                        let controller = self.model.read(cx).controller();
                        controller.update(cx, |controller, cx| {
                            controller.select(pending_id.clone());
                            cx.notify();
                        });
                        self.table.update(cx, |table, cx| {
                            table.begin_editing(pending_id.clone(), "name", window, cx)
                        });
                    }
                } else {
                    self.pending_rename = Some(pending);
                }
            } else {
                self.pending_rename = Some(pending);
            }
        }
        let theme = cx.theme().clone();
        let model = self.model.read(cx);
        let view = model.view();
        let can_back = model.can_go_back();
        let can_forward = model.can_go_forward();
        let current = model.current_entry(cx);
        let ancestors = model.path_ancestors(cx);
        let places = self.places.clone();
        let loading = model.is_loading();
        let error = model
            .error()
            .cloned()
            .or_else(|| self.status_error.clone())
            .or_else(|| model.warnings().first().cloned());
        let selected_count = model.selected_entries(cx).len();

        let this = cx.entity();
        let back = {
            let this = this.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                let _ = window;
                this.update(cx, |panel, cx| {
                    panel.model.update(cx, |model, cx| model.go_back(cx));
                    cx.notify();
                });
            })
        };
        let forward = {
            let this = this.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                let _ = window;
                this.update(cx, |panel, cx| {
                    panel.model.update(cx, |model, cx| model.go_forward(cx));
                    cx.notify();
                });
            })
        };
        let view_fn = {
            let this = this.clone();
            Rc::new(move |mode: FileViewMode, _: &mut Window, cx: &mut App| {
                this.update(cx, |panel, cx| {
                    panel.model.update(cx, |model, cx| model.set_view(mode, cx));
                    match mode {
                        FileViewMode::Icons => panel.ensure_icon_grid(cx),
                        FileViewMode::Columns => panel.ensure_browser(cx),
                        FileViewMode::List => {}
                    }
                    panel.save_panel_state(cx);
                    panel.refresh_stops(cx);
                    cx.notify();
                });
            })
        };
        let nav_fn = {
            let this = this.clone();
            Rc::new(move |id: &SharedString, _: &mut Window, cx: &mut App| {
                let id = id.clone();
                this.update(cx, |panel, cx| panel.navigate(id, false, cx));
            })
        };
        let toolbar = PanelToolbar {
            view,
            can_go_back: can_back,
            can_go_forward: can_forward,
            current,
            ancestors,
            recent: places
                .iter()
                .map(|place| {
                    crate::controls::path_control::PathEntry::new(
                        place.id.clone(),
                        place.label.clone(),
                        place.icon,
                    )
                })
                .collect(),
            search: self.search.clone(),
            on_back: back,
            on_forward: forward,
            on_view: view_fn,
            on_navigate: nav_fn,
        };
        let expanded = self.expanded;
        let mut panel = div()
            .id("file-panel")
            .debug_selector(|| "file-panel".into())
            .when(self.mode != PanelMode::Save, |el| el.size_full())
            .w_full()
            .flex()
            .flex_col()
            .bg(hsla(theme.colors.window))
            .font_family(theme.type_.ui)
            .text_size(px(theme.type_.size_regular))
            .text_color(hsla(theme.colors.text))
            .key_context("SilvestrisFilePanel")
            .on_key_down(cx.listener(|panel, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "tab" && !event.keystroke.modifiers.modified() {
                    cx.stop_propagation();
                    modal::cycle_focus(
                        &panel.session,
                        !event.keystroke.modifiers.shift,
                        window,
                        cx,
                    );
                }
            }));
        let browser_area: AnyElement = match view {
            FileViewMode::List => self.table.clone().into_any_element(),
            FileViewMode::Icons => {
                if self.icons.is_none() {
                    self.ensure_icon_grid(cx);
                }
                self.icons
                    .clone()
                    .map(IntoElement::into_any_element)
                    .unwrap_or_else(|| self.table.clone().into_any_element())
            }
            FileViewMode::Columns => {
                if self.browser.is_none() {
                    self.ensure_browser(cx);
                }
                self.browser
                    .clone()
                    .map(IntoElement::into_any_element)
                    .unwrap_or_else(|| self.table.clone().into_any_element())
            }
        };
        let browser_area = div()
            .debug_selector(|| "panel-browser-area".into())
            .flex_1()
            .min_h(px(0.))
            .flex()
            .border_t_1()
            .border_b_1()
            .border_color(hsla(theme.colors.divider))
            .child(
                div()
                    .debug_selector(|| "panel-sidebar".into())
                    .relative()
                    .flex_none()
                    .w(px(self.sidebar_width))
                    .h_full()
                    .child(self.sidebar())
                    .child(self.sidebar_divider(window, cx)),
            )
            .child(div().flex_1().min_w_0().h_full().child(browser_area));

        // One Save panel, two states. The chooser block and the Where row
        // trade height under a single 220 ms ease; the sheet/card is
        // content-sized so it zooms open and closed as one surface.
        let save = self.mode == PanelMode::Save;
        let serial = self.expand_serial;
        let animate = self.expand_animated && save;
        let duration = if animate {
            Duration::from_millis(220)
        } else {
            Duration::ZERO
        };
        let easing = gpui::ease_in_out;
        let target = if expanded { 1.0 } else { 0.0 };
        let progress = move |t: f32| if expanded { t } else { 1.0 - t };
        let accessory_height = if self.accessory.is_some() {
            chrome::ACCESSORY_HEIGHT
        } else {
            0.0
        };
        let message_height = if self.message.is_some() {
            chrome::MESSAGE_HEIGHT
        } else {
            0.0
        };
        let extra = accessory_height + message_height;

        if save {
            if let Some(message) = &self.message {
                panel = panel.child(
                    div()
                        .debug_selector(|| "panel-message".into())
                        .flex_none()
                        .w_full()
                        .px(px(12.))
                        .pt(px(8.))
                        .text_center()
                        .text_size(px(12.))
                        .child(message.clone()),
                );
            }
            panel = panel.child(self.name_row(window, cx));
            let where_row = div().child(self.where_row(cx));
            let where_row: AnyElement = if animate {
                where_row
                    .with_animation(
                        gpui::ElementId::Name(format!("save-where-{serial}").into()),
                        Animation::new(duration).with_easing(easing),
                        move |el, t| {
                            let p = progress(t);
                            el.h(px(30.0 * (1.0 - p)))
                                .opacity(1.0 - p)
                                .overflow_hidden()
                        },
                    )
                    .into_any_element()
            } else {
                where_row
                    .h(px(30.0 * (1.0 - target)))
                    .opacity(1.0 - target)
                    .overflow_hidden()
                    .into_any_element()
            };
            panel = panel.child(where_row);
        }
        let mut chooser = div()
            .debug_selector(|| "panel-chooser".into())
            .flex_none()
            .flex()
            .flex_col()
            .overflow_hidden()
            .when(save, |el| el.border_t_1())
            .border_color(hsla(theme.colors.divider))
            .child(toolbar)
            .child(browser_area);
        if save {
            chooser = chooser.child(self.format_row(cx));
        }
        if save {
            let chooser: AnyElement = if animate {
                chooser
                    .with_animation(
                        gpui::ElementId::Name(format!("save-chooser-{serial}").into()),
                        Animation::new(duration).with_easing(easing),
                        move |el, t| {
                            let p = progress(t);
                            el.h(px(292.0 * p)).opacity(p)
                        },
                    )
                    .into_any_element()
            } else {
                chooser
                    .h(px(292.0 * target))
                    .opacity(target)
                    .into_any_element()
            };
            panel = panel.child(chooser);
        } else {
            panel = panel.child(chooser.flex_1());
        }
        if save {
            // Minimal Save keeps its accessory space above the button row, as
            // 10.6 does, instead of leaving a gap below the buttons.
            panel = panel.child(div().flex_1().min_h(px(0.)));
        }
        if let Some(accessory) = &self.accessory {
            panel = panel.child(
                div()
                    .debug_selector(|| "panel-accessory".into())
                    .flex_none()
                    .px(px(12.))
                    .py(px(6.))
                    .child(
                        div()
                            .w_full()
                            .min_h(px(76.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(hsla(theme.colors.group_border))
                            .bg(hsla(theme.colors.group_fill))
                            .p(px(8.))
                            .child(accessory(window, cx)),
                    ),
            );
        } else if !save {
            panel = panel.child(self.open_info_row(cx));
        }
        panel = panel.child(self.button_row(window, cx, selected_count, loading, error));
        if let Some(menu) = &self.menu {
            panel = panel.child(deferred(menu.clone()).priority(1));
        }
        if save {
            if animate {
                panel
                    .with_animation(
                        gpui::ElementId::Name(format!("save-root-{serial}").into()),
                        Animation::new(duration).with_easing(easing),
                        move |el, t| {
                            let p = progress(t);
                            el.h(px(160.0 + 216.0 * p + extra))
                        },
                    )
                    .into_any_element()
            } else {
                panel
                    .h(px(160.0 + 216.0 * target + extra))
                    .into_any_element()
            }
        } else {
            panel.into_any_element()
        }
    }
}

impl FilePanel {
    fn name_row(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let field = self.name_field.clone();
        let mut row = div()
            .debug_selector(|| "save-name-row".into())
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(8.))
            .child(div().flex_none().text_size(px(12.)).child("Save As:"));
        if let Some(field) = field {
            row = row.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .child(field.clone()),
            );
        }
        if self.mode == PanelMode::Save {
            let expanded = self.expanded;
            let this = cx.entity();
            row = row.child(
                Disclosure::new("save-disclosure", "Show file browser")
                    .style(DisclosureStyle::Button)
                    .expanded(expanded)
                    .on_toggle(move |next, _, cx| {
                        let next = *next;
                        this.update(cx, |panel, cx| {
                            panel.expanded = next;
                            panel.expand_serial = panel.expand_serial.wrapping_add(1);
                            panel.expand_animated = true;
                            panel.save_panel_state(cx);
                            panel.refresh_stops(cx);
                            cx.notify();
                        });
                    }),
            );
        }
        let _ = window;
        row.into_any_element()
    }

    /// The minimal Save dialog's Where pop-up (HIG p. 247): volumes, sidebar
    /// places and recent places, all as display names, never a raw id.
    fn where_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let current = model.current_entry(cx);
        let ancestors = model.path_ancestors(cx);
        let recent: Vec<PathEntry> = self
            .places
            .iter()
            .map(|place| PathEntry::new(place.id.clone(), place.label.clone(), place.icon))
            .collect();
        let this = cx.entity();
        div()
            .debug_selector(|| "save-where-row".into())
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .pb(px(8.))
            .child(div().flex_none().text_size(px(12.)).child("Where:"))
            .child(
                div().w(px(280.)).child(
                    PathPopUp::new("save-where", current)
                        .ancestors(ancestors)
                        .recent(recent)
                        .on_navigate(move |id, _, cx| {
                            let id = id.clone();
                            this.update(cx, |panel, cx| panel.navigate(id, false, cx));
                        }),
                ),
            )
            .into_any_element()
    }

    /// Expanded Save: the Format slot and Hide extension (HIG p. 248).
    fn format_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let labels: Vec<SharedString> = self.formats.iter().map(|f| f.label.clone()).collect();
        let this = cx.entity();
        let popup = PopUpButton::new("save-format", "Format", labels)
            .selected(Some(self.format))
            .size(ControlSize::Regular)
            .min_width(140.0)
            .on_choose(move |index, _, cx| {
                let index = *index;
                this.update(cx, |panel, cx| panel.choose_format(index, cx));
            });
        let hide = self.hide_extension;
        let this = cx.entity();
        div()
            .debug_selector(|| "save-format-row".into())
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(12.))
            .py(px(6.))
            .child(div().flex_none().text_size(px(12.)).child("Format:"))
            .child(popup)
            .child(div().flex_1())
            .child(
                Checkbox::new("hide-extension", "Hide extension")
                    .size(ControlSize::Small)
                    .checked(hide)
                    .on_change(move |state, _, cx| {
                        this.update(cx, |panel, cx| {
                            panel.hide_extension = *state == CheckState::On;
                            panel.save_panel_state(cx);
                            cx.notify();
                        });
                    }),
            )
            .into_any_element()
    }

    fn sidebar_divider(&self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let this = cx.entity();
        let divider = cx.theme().colors.divider;
        div()
            .id("panel-sidebar-divider")
            .debug_selector(|| "panel-sidebar-divider".into())
            .absolute()
            .right_0()
            .top_0()
            .bottom_0()
            .w(px(5.))
            .flex()
            .justify_center()
            .cursor(gpui::CursorStyle::ResizeLeftRight)
            .child(
                gpui::canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        crate::paint::vline(
                            window,
                            bounds.center().x,
                            bounds.origin.y,
                            bounds.size.height,
                            divider,
                        );
                    },
                )
                .w_full()
                .h_full(),
            )
            .on_mouse_down(gpui::MouseButton::Left, move |event, window, cx| {
                let start_x = f32::from(event.position.x);
                this.update(cx, |panel, cx| {
                    panel.resizing_sidebar = Some((start_x, panel.sidebar_width));
                    cx.notify();
                });
                let moving = this.clone();
                window.on_mouse_event(move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                    if phase != gpui::DispatchPhase::Capture {
                        return;
                    }
                    let x = f32::from(event.position.x);
                    moving.update(cx, |panel, cx| {
                        let Some((start, width)) = panel.resizing_sidebar else {
                            return;
                        };
                        let next = (width + (x - start)).clamp(120.0, 320.0);
                        if (next - panel.sidebar_width).abs() >= 0.5 {
                            panel.sidebar_width = next;
                            cx.notify();
                        }
                    });
                });
                let finished = this.clone();
                window.on_mouse_event(move |_: &gpui::MouseUpEvent, phase, _, cx| {
                    if phase != gpui::DispatchPhase::Capture {
                        return;
                    }
                    finished.update(cx, |panel, cx| {
                        if panel.resizing_sidebar.take().is_some() {
                            panel.save_panel_state(cx);
                            cx.notify();
                        }
                    });
                });
                cx.stop_propagation();
            })
            .into_any_element()
    }

    /// The Open panel's default selection band, matching the reference
    /// layout: name, kind, size and date for the chosen file. A consumer that
    /// supplies its own `accessory` replaces it.
    fn open_info_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let entry = if self.model.read(cx).view() == FileViewMode::Columns {
            self.browser_target(cx).map(|(_, entry)| entry)
        } else {
            self.model.read(cx).selected_entries(cx).into_iter().next()
        };
        let rows: Vec<(&'static str, String)> = entry.map_or_else(Vec::new, |entry| {
            vec![
                ("File", entry.name.to_string()),
                ("Kind", entry.kind.label().to_string()),
                ("Size", entry.size_label().to_string()),
                ("Last modified", entry.modified_label().to_string()),
            ]
        });
        let mut info = div()
            .debug_selector(|| "open-info".into())
            .flex()
            .flex_col()
            .items_center()
            .gap(px(2.))
            .text_size(px(11.));
        if rows.is_empty() {
            info = info.child(
                div()
                    .text_color(hsla(theme.colors.text_secondary))
                    .child("No file selected"),
            );
        } else {
            for (label, value) in rows {
                info = info.child(
                    div()
                        .flex()
                        .gap(px(4.))
                        .child(
                            div()
                                .flex_none()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child(format!("{label}:")),
                        )
                        .child(div().truncate().child(value)),
                );
            }
        }
        div()
            .flex_none()
            .px(px(12.))
            .py(px(6.))
            .child(
                div()
                    .w_full()
                    .min_h(px(66.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(hsla(theme.colors.group_border))
                    .bg(hsla(theme.colors.group_fill))
                    .p(px(8.))
                    .child(info),
            )
            .into_any_element()
    }

    fn button_row(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        selected_count: usize,
        loading: bool,
        error: Option<FileError>,
    ) -> AnyElement {
        let name_ok = self.mode != PanelMode::Save || self.name_valid(cx);
        let accept_enabled = if loading || self.closing {
            false
        } else {
            match self.mode {
                PanelMode::Save => name_ok,
                PanelMode::Open => {
                    if self.model.read(cx).view() == FileViewMode::Columns {
                        self.browser_target(cx)
                            .is_some_and(|(_, entry)| self.choosable_now(&entry, cx))
                    } else {
                        selected_count > 0 && (self.allows_multiple || selected_count == 1) && {
                            let selected = self.model.read(cx).selected_entries(cx);
                            selected.iter().all(|entry| {
                                (choosable(
                                    entry,
                                    self.can_choose_files,
                                    self.can_choose_directories,
                                ) || (entry.is_dir && !entry.is_package))
                                    && (entry.is_dir || self.model.read(cx).matches_types(entry))
                            })
                        }
                    }
                }
            }
        };
        let new_folder = {
            let this = cx.entity();
            Button::new("panel-new-folder", "New Folder")
                .size(ControlSize::Regular)
                .min_width(80.0)
                .focus_handle(self.new_folder_focus.clone())
                .on_click(move |_, _, cx| {
                    this.update(cx, |panel, cx| panel.new_folder(cx));
                })
        };
        let cancel = {
            let this = cx.entity();
            Button::new("panel-cancel", "Cancel")
                .size(ControlSize::Regular)
                .min_width(58.0)
                .cancel_button(true)
                .focus_handle(self.cancel_focus.clone())
                .on_click(move |_, window, cx| {
                    this.update(cx, |panel, cx| panel.cancel(window, cx));
                })
        };
        let accept_label = match self.mode {
            PanelMode::Save => "Save",
            PanelMode::Open => "Open",
        };
        let accept = {
            let this = cx.entity();
            Button::new("panel-accept", accept_label)
                .size(ControlSize::Regular)
                .min_width(58.0)
                .default_button(true)
                .disabled(!accept_enabled)
                .focus_handle(self.accept_focus.clone())
                .on_click(move |_, _, cx| {
                    this.update(cx, |panel, cx| panel.accept_focused(cx));
                })
        };
        let show_new_folder =
            self.mode == PanelMode::Save && self.expanded && self.capabilities.create_folder;
        let status = error
            .map(|error| error.message.to_string())
            .or_else(|| loading.then(|| "Loading\u{2026}".to_string()));
        let has_status = status.is_some();
        let _ = window;
        div()
            .debug_selector(|| "panel-buttons".into())
            .flex_none()
            .flex()
            .items_center()
            .gap(px(8.))
            .h(px(chrome::BUTTON_ROW_HEIGHT))
            .px(px(12.))
            .when(show_new_folder, |el| el.child(new_folder))
            .when_some(status, |el, status| {
                el.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(hsla(cx.theme().colors.text_secondary))
                        .child(status),
                )
            })
            .when(!has_status, |el| el.child(div().flex_1()))
            .child(cancel)
            .child(accept)
            .into_any_element()
    }
}

/// A root view for an application-modal panel window.
struct PanelWindow {
    panel: Entity<FilePanel>,
}

impl Render for PanelWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.panel.read(cx).title.clone();
        WindowFrame::new(title)
            .role(WindowRole::OpenPanel)
            .child(self.panel.clone())
    }
}

/// Present a panel as a document sheet: Save/Save As, or Choose-style open
/// requests (HIG pp. 247, 251). One sheet per window; a second request is
/// cancelled rather than stacked.
pub fn begin_sheet(
    panel: Entity<FilePanel>,
    owner: AnyWindowHandle,
    title: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<FilePanel> {
    let session = panel.read(cx).session.clone();
    let save = panel.read(cx).mode == PanelMode::Save;
    let height = panel.read(cx).preferred_height();
    panel.update(cx, |panel, cx| {
        panel.owner = Some(owner);
        panel.title = title.to_string().into();
        panel.session.update(cx, |session, _| {
            session.set_scope(ModalScope::Document);
        });
        cx.notify();
    });
    let sheet = if save {
        // The Save panel's own root is content-sized and animated; the sheet
        // card must follow it frame by frame rather than hold a fixed height.
        let content_panel = panel.clone();
        sheet::Sheet::new(
            title.to_string(),
            chrome::PANEL_WIDTH,
            session.clone(),
            move |_: &mut Window, _: &mut App| content_panel.clone().into_any_element(),
            cx,
        )
    } else {
        let content_panel = panel.clone();
        sheet::Sheet::new_sized(
            title.to_string(),
            chrome::PANEL_WIDTH,
            height,
            session.clone(),
            move |_: &mut Window, _: &mut App| content_panel.clone().into_any_element(),
            cx,
        )
    };
    if !sheet::present(owner, sheet, cx) {
        modal::finish(session, ModalResult::Cancelled, window, cx);
    }
    panel
}

/// Present a panel in its own application-modal window. Open uses this by
/// default (HIG p. 245); Save uses it only when the application has no
/// document window. The title accepts an app name, e.g. "TextEdit: Open".
pub fn run_modal(
    panel: Entity<FilePanel>,
    title: &str,
    window: &mut Window,
    cx: &mut App,
) -> Entity<FilePanel> {
    let width = chrome::PANEL_WIDTH;
    let height = chrome::PANEL_HEIGHT;
    let parent = window.bounds();
    let store = panel.read(cx).frame_store.clone();
    let bounds = crate::window_role::restore_bounds(
        WindowRole::OpenPanel,
        gpui::Bounds::new(
            gpui::point(px(0.), px(0.)),
            gpui::size(px(width), px(height)),
        ),
        Some(parent),
        store
            .as_deref()
            .map(|store| store as &dyn crate::window_role::FrameStore),
        cx,
    );
    let panel_for_window = panel.clone();
    let window_title = title.to_string();
    let opened = open_role(
        WindowRole::OpenPanel,
        window_title.clone(),
        bounds,
        cx,
        move |window, cx| {
            let handle = window.window_handle();
            panel_for_window.update(cx, |panel, cx| {
                panel.app_window = Some(handle);
                panel.title = window_title.clone().into();
                panel.session.update(cx, |session, _| {
                    session.set_scope(ModalScope::Application);
                });
                cx.notify();
            });
            let session = panel_for_window.read(cx).session.clone();
            modal::present(session, window, cx);
            window.on_window_should_close(cx, |window, cx| {
                modal::cancel_owned(window.window_handle(), cx);
                true
            });
            cx.new(|_| PanelWindow {
                panel: panel_for_window,
            })
        },
    );
    match opened {
        Ok(handle) => {
            let any = *handle;
            panel.update(cx, |panel, _| panel.app_window = Some(any));
        }
        Err(_) => {
            panel.update(cx, |panel, cx| {
                let session = panel.session.clone();
                modal::finish(session, ModalResult::Cancelled, window, cx);
            });
        }
    }
    panel
}

impl FilePanel {
    /// Content height in the panel's host: the Open window's content area,
    /// or the sheet's content.
    /// Content height in the panel's host: the Open window's content area,
    /// or the sheet's content. Includes an optional message and accessory.
    #[must_use]
    pub fn preferred_height(&self) -> f32 {
        let base = if self.mode == PanelMode::Save && !self.expanded {
            chrome::COLLAPSED_HEIGHT
        } else {
            chrome::PANEL_CONTENT_HEIGHT
        };
        let accessory = if self.accessory.is_some() {
            chrome::ACCESSORY_HEIGHT
        } else {
            0.0
        };
        let message = if self.message.is_some() {
            chrome::MESSAGE_HEIGHT
        } else {
            0.0
        };
        base + accessory + message
    }

    /// Select one entry by its provider id. In column view this also sets the
    /// browser path, so a restored selection and its preview survive a reload.
    pub fn select_entry(&mut self, id: &ItemId, cx: &mut Context<Self>) {
        if self.model.read(cx).is_loading() {
            self.pending_selection = Some(id.clone());
        } else {
            self.pending_selection = None;
            self.apply_selection(id, cx);
        }
    }

    fn apply_selection(&mut self, id: &ItemId, cx: &mut Context<Self>) {
        self.model
            .read(cx)
            .controller()
            .update(cx, |controller, cx| {
                controller.select(id.clone());
                cx.notify();
            });
        if self.model.read(cx).view() == FileViewMode::Columns {
            self.ensure_browser(cx);
            if let Some(browser) = &self.browser {
                let path = [id.clone()];
                browser.update(cx, |browser, cx| browser.set_path(&path, cx));
            }
        }
        cx.notify();
    }

    #[must_use]
    pub fn mode(&self) -> PanelMode {
        self.mode
    }

    #[must_use]
    pub fn model(&self) -> Entity<FileBrowserModel> {
        self.model.clone()
    }

    #[must_use]
    pub fn session(&self) -> Entity<ModalSession> {
        self.session.clone()
    }

    #[must_use]
    pub fn expanded(&self) -> bool {
        self.expanded
    }

    #[must_use]
    pub fn selected_count(&self, cx: &App) -> usize {
        self.model.read(cx).selected_entries(cx).len()
    }

    /// The provider or mutation error currently shown in the panel's status
    /// line, if any.
    #[must_use]
    pub fn status_error(&self) -> Option<&FileError> {
        self.status_error.as_ref()
    }

    /// The name field's current text, or empty for an Open panel.
    #[must_use]
    pub fn name_text(&self, cx: &App) -> String {
        self.name(cx)
    }

    /// Set the name field programmatically without a user event.
    pub fn set_name(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(field) = &self.name_field else {
            return;
        };
        field.update(cx, |field, cx| field.set_text(name.to_string(), cx));
        let stem = name.rfind('.').unwrap_or(name.len());
        let stem = if stem == 0 { name.len() } else { stem };
        field.update(cx, |field, cx| {
            crate::text_client::select_range(field, 0..stem, window, cx);
        });
        cx.notify();
    }

    /// Whether the list or a column is editing a name.
    #[must_use]
    pub fn is_editing(&self, cx: &App) -> bool {
        self.table.read(cx).is_editing()
            || self
                .browser
                .as_ref()
                .is_some_and(|browser| browser.read(cx).is_editing(cx))
            || self
                .icons
                .as_ref()
                .is_some_and(|grid| grid.read(cx).is_editing())
    }

    /// Switch presentation without a toolbar click. The location, controller
    /// and selection stay; the choice is written to the state store.
    pub fn set_view(&mut self, view: FileViewMode, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| model.set_view(view, cx));
        match view {
            FileViewMode::Icons => self.ensure_icon_grid(cx),
            FileViewMode::Columns => self.ensure_browser(cx),
            FileViewMode::List => {}
        }
        self.save_panel_state(cx);
        self.refresh_stops(cx);
        cx.notify();
    }

    /// The remembered sidebar width in logical pixels (M: panel.geometry).
    #[must_use]
    pub fn sidebar_width(&self) -> f32 {
        self.sidebar_width
    }

    #[must_use]
    pub fn table(&self) -> Entity<TableView<FileEntry>> {
        self.table.clone()
    }

    #[must_use]
    pub fn sidebar_entity(&self) -> Entity<SourceList> {
        self.sidebar.clone()
    }

    /// The column browser when it has been built, for tests and consumers that
    /// inspect its presentation state.
    #[must_use]
    pub fn browser(&self) -> Option<Entity<ColumnBrowser>> {
        self.browser.clone()
    }

    /// Activate the default button programmatically (Return outside a text
    /// field). The button click path uses the same method.
    pub fn submit(&mut self, cx: &mut Context<Self>) {
        self.accept_focused(cx);
    }

    /// Cancel the panel. The sheet's session receives `Rejected`.
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel(window, cx);
    }

    /// Double-click on the current selection. Tests and programmatic callers
    /// use the same path as the table's `Activated` event.
    pub fn activate_selection(&mut self, cx: &mut Context<Self>) {
        self.table_activated(cx);
    }

    /// Move keyboard focus to the name field.
    pub fn focus_name(&self, window: &mut Window, cx: &mut App) {
        if let Some(handle) = &self.name_focus {
            window.focus(handle, cx);
        }
    }

    #[must_use]
    pub fn name_focus_handle(&self) -> Option<FocusHandle> {
        self.name_focus.clone()
    }
}
