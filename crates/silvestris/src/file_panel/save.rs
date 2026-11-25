//! SavePanel (#29): the Save/Save As panel consumer API.
//!
//! ```ignore
//! SavePanel::new(provider)
//!     .directory(home)
//!     .name("Untitled.txt")
//!     .allowed_types(["txt"])
//!     .on_finish(|outcome, cx| {
//!         if let Some(path) = outcome.first_path() {
//!             save_document(path, cx);
//!         }
//!     })
//!     .begin_sheet(owner, window, cx);
//! ```
//!
//! Save is a document sheet (HIG p. 247): [`SavePanel::begin_sheet`] is the
//! primary path. [`SavePanel::run_modal`] exists for an application with no
//! document window and is named so it cannot be reached by accident.
//! The panel returns a destination; it never writes a file.

use std::rc::Rc;

use gpui::{AnyWindowHandle, App, Entity, Window};

use super::panel::{self, ExtensionPolicy, FilePanel, FormatOption, PanelOptions, PanelOutcome};
use super::provider::FileProvider;
use super::state::PanelStateStore;

pub struct SavePanel {
    provider: Rc<dyn FileProvider>,
    options: PanelOptions,
}

impl SavePanel {
    #[must_use]
    pub fn new(provider: Rc<dyn FileProvider>) -> Self {
        Self {
            provider,
            options: PanelOptions::new(super::panel::PanelMode::Save, "".into()),
        }
    }

    /// The directory the panel opens in. The provider supplies the id.
    #[must_use]
    pub fn directory(mut self, location: impl Into<super::LocationId>) -> Self {
        self.options.location = location.into();
        self
    }

    /// The directory the panel opens in, as a host path. Does nothing when the
    /// provider cannot map the path to a location; use [`directory`](Self::directory)
    /// for a provider without host paths.
    #[must_use]
    pub fn directory_path(mut self, path: impl AsRef<std::path::Path>) -> Self {
        if let Some(location) = self.provider.location_of(path.as_ref()) {
            self.options.location = location;
        }
        self
    }

    /// The suggested name. For a Save As the field selects the base name and
    /// leaves the extension unselected, as 10.6 does.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.options.name = name.into();
        self
    }

    /// An application message above the Save As row, centred like the
    /// NSSavePanel message line.
    #[must_use]
    pub fn message(mut self, message: impl Into<gpui::SharedString>) -> Self {
        self.options.message = Some(message.into());
        self
    }

    /// Allowed extensions without the dot. Empty accepts everything.
    #[must_use]
    pub fn allowed_types(mut self, types: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.options.allowed_types = types.into_iter().map(Into::into).collect();
        self
    }

    /// `false` dims files whose extension is not in `allowed_types`.
    #[must_use]
    pub fn allows_other_types(mut self, allows: bool) -> Self {
        self.options.allows_other_types = allows;
        self
    }

    /// What to do with the typed extension. The default, `Any`, keeps exactly
    /// what the user typed: `allowed_types` only filters the browser. Use
    /// `Fixed("txt")` to force an extension or `Required` to append the first
    /// allowed extension when none was typed.
    #[must_use]
    pub fn extension_policy(mut self, policy: ExtensionPolicy) -> Self {
        self.options.extension_policy = policy;
        self
    }

    /// Start with "Hide extension" checked. The choice then persists through
    /// the state store.
    #[must_use]
    pub fn hide_extension(mut self, hide: bool) -> Self {
        self.options.hide_extension = hide;
        self.options.hide_extension_set = true;
        self
    }

    /// Start with the file browser expanded. The choice then persists through
    /// the state store.
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.options.expanded = expanded;
        self.options.expanded_set = true;
        self
    }

    /// Add a Format choice (expanded Save, HIG p. 248). Choosing it applies
    /// its extensions to the allowed-type policy.
    #[must_use]
    pub fn format(
        mut self,
        label: impl Into<gpui::SharedString>,
        extensions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.options
            .formats
            .push(FormatOption::new(label, extensions));
        self
    }

    /// Persist the expanded state, Hide extension and sidebar width in the
    /// application's preferences entity.
    #[must_use]
    pub fn state_store(mut self, store: Rc<dyn PanelStateStore>) -> Self {
        self.options.state_store = Some(store);
        self
    }

    /// Application-supplied content above the button row.
    #[must_use]
    pub fn accessory(
        mut self,
        content: impl Fn(&mut Window, &mut App) -> gpui::AnyElement + 'static,
    ) -> Self {
        self.options.accessory = Some(Rc::new(content));
        self
    }

    /// Called once when the panel closes. Keep it short; saving is the
    /// consumer's job.
    #[must_use]
    pub fn on_finish(mut self, f: impl Fn(&PanelOutcome, &mut App) + 'static) -> Self {
        self.options.on_finish = Some(Rc::new(f));
        self
    }

    /// Present Save as a sheet attached to a document window (HIG p. 247).
    pub fn begin_sheet(
        self,
        owner: AnyWindowHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<FilePanel> {
        let panel = FilePanel::build(self.options, self.provider, window, cx);
        panel::begin_sheet(panel, owner, "Save", window, cx)
    }

    /// Fallback for an application with no document window. Open and Choose
    /// use [`OpenPanel::run_modal`](super::OpenPanel::run_modal) instead.
    pub fn run_modal(self, window: &mut Window, cx: &mut App) -> Entity<FilePanel> {
        let panel = FilePanel::build(self.options, self.provider, window, cx);
        panel::run_modal(panel, "Save", window, cx)
    }
}
