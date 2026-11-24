//! Shared Open/Save panel view (#29/T7).
//!
//! The panel composes the generic controls: `SourceList` sidebar, `TableView`
//! list or `ColumnBrowser` columns, `PathPopUp`, `TextField` search and name
//! fields, the accessory slot and the button row. All file data comes from
//! `FileBrowserModel` / `FileProvider`; the panel owns policy only.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyElement, AnyWindowHandle, App, Context, Entity, FocusHandle, Focusable,
    KeyDownEvent, SharedString, Subscription, Window,
};

use crate::binding::ItemId;
use crate::chrome::WindowFrame;
use crate::controls::column_browser::{BrowserEvent, BrowserNode, ColumnBrowser};
use crate::controls::icon_grid::{IconGrid, IconGridEvent};
use crate::controls::path_control::{LocationId, PathEntry, PathPopUp};
use crate::controls::popup_button::PopUpButton;
use crate::controls::source_list::{RowAccessory, SourceList, SourceListEvent, SourceListStyle};
use crate::controls::table_view::{TableColumn, TableEvent, TableView};
use crate::controls::text_field::{TextField, TextFieldEvent};
use crate::controls::{Alert, AlertResult, Button, CheckState, Checkbox, ControlSize, Disclosure};
use crate::convert::hsla;
use crate::file_panel::chrome::{self, PanelToolbar};
use crate::file_panel::model::{choosable, FileBrowserEvent, FileBrowserModel, FileViewMode};
use crate::file_panel::provider::{FileEntry, FileError, FilePlace, FileProvider, PlaceSection};
use crate::file_panel::state::{PanelState, PanelStateStore};
use crate::icons;
use crate::modal::{self, ModalPhase, ModalResult, ModalScope, ModalSession};
use crate::sheet;
use crate::window_role::{open_role, WindowRole};
use crate::ActiveTheme;

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
            state_store: None,
            frame_store: None,
            accessory: None,
            on_finish: None,
        }
    }
}

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
    browser_entries: HashMap<ItemId, (LocationId, FileEntry)>,
    search: Entity<TextField>,
    name_field: Option<Entity<TextField>>,
    expanded: bool,
    hide_extension: bool,
    format: usize,
    formats: Vec<FormatOption>,
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
    browser_navigation: bool,
    app_window: Option<AnyWindowHandle>,
    owner: Option<AnyWindowHandle>,
    title: SharedString,
    focused_once: bool,
    name_focus: Option<FocusHandle>,
    new_folder_focus: FocusHandle,
    cancel_focus: FocusHandle,
    accept_focus: FocusHandle,
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
        let model =
            cx.new(|cx| FileBrowserModel::new(provider.clone(), options.location.clone(), cx));
        model.update(cx, |model, cx| {
            model.set_allowed_types(options.allowed_types.clone());
            model.set_allows_other_types(options.allows_other_types);
            model.set_view(view, cx);
        });
        let places = provider.places(cx);
        let sidebar = build_sidebar(&places, cx);
        let table = build_table(model.clone(), provider.clone(), options.clone(), cx);
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
            search,
            name_field,
            expanded,
            hide_extension,
            format: 0,
            formats,
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
            browser_navigation: false,
            app_window: None,
            owner: None,
            title: "Save".into(),
            focused_once: false,
            name_focus: None,
            new_folder_focus: cx.focus_handle(),
            cancel_focus: cx.focus_handle(),
            accept_focus: cx.focus_handle(),
            _subscriptions: Vec::new(),
            _model_subscription: cx.observe(&model_for_observe, |_: &mut Self, _, cx| cx.notify()),
            _session_subscription: cx.observe(&session_for_observe, |panel, session, cx| {
                if session.read(cx).is_ended() {
                    panel.deliver(cx);
                }
            }),
        });
        panel.update(cx, |panel, cx| panel.wire(cx));
        panel
    }

    fn wire(&mut self, cx: &mut Context<Self>) {
        let model = self.model.clone();
        self._subscriptions.push(
            cx.subscribe(&model, |panel, _, event: &FileBrowserEvent, cx| {
                panel.model_event(event, cx)
            }),
        );
        let table = self.table.clone();
        self._subscriptions.push(cx.subscribe(
            &table,
            |panel, _, event: &TableEvent, cx| match event {
                TableEvent::Activated(_) => panel.table_activated(cx),
                TableEvent::SelectionChanged(_) => cx.notify(),
                _ => {}
            },
        ));
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
                    TextFieldEvent::Changed | TextFieldEvent::EndedEditing => cx.notify(),
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
                    entry.name.to_lowercase().contains(&query)
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

    fn name_valid(&self, cx: &App) -> bool {
        let name = self.name(cx);
        !name.is_empty()
            && name != "."
            && name != ".."
            && !name.contains('/')
            && !name.contains('\\')
            && !name.contains(':')
    }

    fn save_clicked(&mut self, cx: &mut Context<Self>) {
        if self.closing || self.mode != PanelMode::Save || !self.name_valid(cx) {
            return;
        }
        let name = self.name(cx);
        let entry = FileEntry::document(name.clone(), name.clone());
        let model = self.model.clone();
        let task = model.update(cx, |model, cx| model.exists(&name, cx));
        self.closing = true;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let exists = task.await;
            let _ = this.update(cx, |panel, cx| {
                panel.closing = false;
                if exists {
                    panel.show_replace(&name, &entry, cx);
                } else {
                    panel.accept_save(&name, &entry, cx);
                }
            });
        })
        .detach();
    }

    fn show_replace(&mut self, name: &str, entry: &FileEntry, cx: &mut Context<Self>) {
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
        let entry = entry.clone();
        let alert = Alert::replace_existing(name.clone())
            .window_title("Replace")
            .on_result(move |result: &AlertResult, _, cx| {
                if result.choice.as_deref() == Some("alert-replace") {
                    if let Some(panel) = panel.upgrade() {
                        panel.update(cx, |panel, cx| {
                            panel.accept_save(&name, &entry, cx);
                        });
                    }
                }
            });
        let _ = owner.update(cx, |_, window, cx| {
            alert.run_modal(window, cx);
        });
    }

    fn accept_save(&mut self, name: &str, entry: &FileEntry, cx: &mut Context<Self>) {
        let (location, path) = {
            let model = self.model.read(cx);
            (
                model.location_of_entry(entry, cx),
                model.path_of_entry(entry),
            )
        };
        let outcome = PanelOutcome {
            result: ModalResult::Accepted,
            paths: path.into_iter().collect(),
            locations: vec![location],
            hide_extension: self.hide_extension,
        };
        self.finish(outcome, cx);
        let _ = name;
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
        let rename_id: LocationId = name.clone().into();
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |panel, cx| match result {
                Ok(_) => {
                    panel.pending_rename = Some(rename_id.clone());
                    panel.model.update(cx, |model, cx| model.reload(cx));
                    cx.notify();
                }
                Err(_) => cx.notify(),
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
            return;
        }
        let entries: Vec<FileEntry> = self
            .model
            .read(cx)
            .controller()
            .read(cx)
            .visible_items()
            .into_iter()
            .cloned()
            .collect();
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
        let provider = self.model.read(cx).provider().clone();
        let model = self.model.clone();
        browser.update(cx, |browser, cx| {
            browser.set_rename(
                move |parent, id, new_name, cx| {
                    if new_name.trim().is_empty()
                        || new_name.contains('/')
                        || new_name.contains('\\')
                    {
                        return false;
                    }
                    let parent: LocationId = parent.clone();
                    let id = id.clone();
                    let name = new_name.to_string();
                    let task = provider.rename(&parent, &id, &name, cx);
                    let model = model.clone();
                    cx.spawn(async move |cx| {
                        let _ = task.await;
                        cx.update(|cx| model.update(cx, |model, cx| model.reload(cx)));
                    })
                    .detach();
                    true
                },
                cx,
            );
        });
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
            },
        );
        self._subscriptions.push(subscription);
        self.refresh_stops(cx);
    }

    fn rebuild_browser(&mut self, cx: &mut Context<Self>) {
        self.browser = None;
        self.ensure_browser(cx);
        cx.notify();
    }

    fn ensure_icon_grid(&mut self, cx: &mut Context<Self>) {
        if self.icons.is_some() {
            return;
        }
        let controller = self.model.read(cx).controller();
        let model = self.model.clone();
        let icon_model = model.clone();
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
        icons.update(cx, |grid, cx| {
            grid.set_multiple(self.allows_multiple, cx);
        });
        let subscription =
            cx.subscribe(&icons, |panel, _, event: &IconGridEvent, cx| match event {
                IconGridEvent::SelectionChanged => cx.notify(),
                IconGridEvent::Activated(_) => panel.table_activated(cx),
            });
        self._subscriptions.push(subscription);
        self.icons = Some(icons);
        let _ = model;
        self.refresh_stops(cx);
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
                let Some(browser) = &panel.browser else {
                    return;
                };
                if let Ok(entries) = result {
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
                }
            });
        })
        .detach();
    }

    fn browser_entry(&self, id: &ItemId) -> Option<(LocationId, FileEntry)> {
        self.browser_entries.get(id).cloned()
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

fn build_sidebar(places: &[FilePlace], cx: &mut App) -> Entity<SourceList> {
    let mut devices = Vec::new();
    let mut locations = Vec::new();
    let mut searches = Vec::new();
    let eject: Vec<String> = places
        .iter()
        .filter(|place| place.eject)
        .map(|place| place.id.to_string())
        .collect();
    for place in places {
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
    let sidebar = cx.new(|cx| SourceList::new(roots, cx).expect("unique place ids"));
    sidebar.update(cx, |sidebar, cx| {
        sidebar.style(SourceListStyle::Primary, cx);
        sidebar.set_icon_renderer(icons::source_renderer(16.0), cx);
        sidebar.set_row_accessories(
            move |row| {
                if eject.iter().any(|id| id == &row.id) {
                    vec![RowAccessory::Eject]
                } else {
                    Vec::new()
                }
            },
            cx,
        );
    });
    sidebar
}

fn build_table(
    model: Entity<FileBrowserModel>,
    provider: Rc<dyn FileProvider>,
    options: PanelOptions,
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
            let provider = provider.clone();
            let model = model.clone();
            move |entry: &FileEntry, new_name: &str, cx: &mut App| {
                if new_name.trim().is_empty() || entry.name == new_name {
                    return false;
                }
                let location = model.read(cx).location().clone();
                let id = entry.id.clone();
                let name = new_name.to_string();
                let task = provider.rename(&location, &id, &name, cx);
                let model = model.clone();
                cx.spawn(async move |cx| {
                    let _ = task.await;
                    cx.update(|cx| model.update(cx, |model, cx| model.reload(cx)));
                })
                .detach();
                true
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
    cx.new(|cx| {
        let mut table = TableView::new(controller, columns, cx);
        table.set_row_height(19.0, cx);
        table.set_allows_multiple(options.allows_multiple);
        table.set_fill_width(true, cx);
        table
    })
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

impl Render for FilePanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.focused_once {
            self.focused_once = true;
            if let Some(name) = &self.name_focus {
                window.focus(name, cx);
            }
        }
        if let Some(pending) = self.pending_rename.take() {
            if !self.model.read(cx).is_loading() {
                if self.model.read(cx).view() == FileViewMode::Columns {
                    self.rebuild_browser(cx);
                    let began = self.browser.as_ref().is_some_and(|browser| {
                        browser.update(cx, |browser, cx| {
                            browser.begin_editing(&pending, window, cx)
                        })
                    });
                    if !began {
                        self.pending_rename = Some(pending);
                    }
                } else {
                    let controller = self.model.read(cx).controller();
                    controller.update(cx, |controller, cx| {
                        controller.select(pending.clone());
                        cx.notify();
                    });
                    self.table.update(cx, |table, cx| {
                        table.begin_editing(pending.clone(), "name", window, cx)
                    });
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
        let places = model.places(cx);
        let loading = model.is_loading();
        let error = model.error().cloned();
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
            .size_full()
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
        if self.mode == PanelMode::Save {
            panel = panel.child(self.name_row(window));
            if !expanded {
                panel = panel.child(self.where_row(cx));
            }
        }
        if self.mode == PanelMode::Open || expanded {
            panel = panel.child(toolbar);
        }
        if expanded {
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
            panel = panel.child(
                div()
                    .debug_selector(|| "panel-browser-area".into())
                    .flex_1()
                    .min_h(px(80.))
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
                    .child(div().flex_1().min_w_0().h_full().child(browser_area)),
            );
            if self.mode == PanelMode::Save {
                panel = panel.child(self.format_row(cx));
            }
        }
        if let Some(accessory) = &self.accessory {
            panel = panel.child(
                div()
                    .flex_none()
                    .px(px(12.))
                    .py(px(6.))
                    .child(accessory(window, cx)),
            );
        }
        panel = panel.child(self.button_row(window, cx, selected_count, loading, error));
        panel
    }
}

impl FilePanel {
    fn name_row(&self, window: &mut Window) -> AnyElement {
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
        let _ = window;
        row.into_any_element()
    }

    /// The minimal Save dialog's Where pop-up (HIG p. 247): volumes, sidebar
    /// places and recent places, all as display names, never a raw id.
    fn where_row(&self, cx: &mut Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        let current = model.current_entry(cx);
        let ancestors = model.path_ancestors(cx);
        let recent: Vec<PathEntry> = model
            .places(cx)
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
        let disclosure = if self.mode == PanelMode::Save {
            let expanded = self.expanded;
            let this = cx.entity();
            Some(
                Disclosure::new("save-disclosure", "Show file browser")
                    .expanded(expanded)
                    .on_toggle(move |next, _, cx| {
                        let next = *next;
                        this.update(cx, |panel, cx| {
                            panel.expanded = next;
                            panel.save_panel_state(cx);
                            panel.refresh_stops(cx);
                            cx.notify();
                        });
                    }),
            )
        } else {
            None
        };
        let show_new_folder = self.mode == PanelMode::Save && self.expanded;
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
            .children(disclosure)
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
    let height = panel.read(cx).preferred_height();
    panel.update(cx, |panel, cx| {
        panel.owner = Some(owner);
        panel.title = title.to_string().into();
        panel.session.update(cx, |session, _| {
            session.set_scope(ModalScope::Document);
        });
        cx.notify();
    });
    let sheet = sheet::Sheet::new_sized(
        title.to_string(),
        chrome::PANEL_WIDTH,
        height,
        session.clone(),
        {
            let panel = panel.clone();
            move |_, _| panel.clone().into_any_element()
        },
        cx,
    );
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
    fn preferred_height(&self) -> f32 {
        if self.mode == PanelMode::Save && !self.expanded {
            chrome::COLLAPSED_HEIGHT
        } else {
            chrome::PANEL_CONTENT_HEIGHT
        }
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
