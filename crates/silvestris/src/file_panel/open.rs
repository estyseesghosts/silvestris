//! OpenPanel (#29): choosing one or more files, or a folder.
//!
//! ```ignore
//! OpenPanel::new(provider)
//!     .directory(home)
//!     .allows_multiple(false)
//!     .can_choose_files(true)
//!     .can_choose_directories(false)
//!     .on_finish(|outcome, cx| {
//!         for path in &outcome.paths {
//!             open_document(path, cx);
//!         }
//!     })
//!     .run_modal(window, cx);
//! ```
//!
//! File > Open is application-modal and is its own framed window (HIG p. 245):
//! [`OpenPanel::run_modal`] is the primary path. [`OpenPanel::begin_sheet`]
//! exists for Choose-style requests (p. 251), which are attached to a
//! document. Double-click enters a folder or accepts a file. Open is disabled
//! while the selection contains anything the flags do not allow.

use std::rc::Rc;

use gpui::{AnyWindowHandle, App, Entity, SharedString, Window};

use super::panel::{self, FilePanel, PanelOptions, PanelOutcome};
use super::provider::FileProvider;
use super::state::PanelStateStore;

pub struct OpenPanel {
    provider: Rc<dyn FileProvider>,
    options: PanelOptions,
    title: SharedString,
}

impl OpenPanel {
    #[must_use]
    pub fn new(provider: Rc<dyn FileProvider>) -> Self {
        Self {
            provider,
            options: PanelOptions::new(super::panel::PanelMode::Open, "".into()),
            title: "Open".into(),
        }
    }

    #[must_use]
    pub fn directory(mut self, location: impl Into<super::LocationId>) -> Self {
        self.options.location = location.into();
        self
    }

    /// The dialog title; default "Open", or an app-supplied "TextEdit: Open"
    /// (HIG p. 245).
    #[must_use]
    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = title.into();
        self
    }

    /// Allow more than one selected row.
    #[must_use]
    pub fn allows_multiple(mut self, multiple: bool) -> Self {
        self.options.allows_multiple = multiple;
        self
    }

    /// Whether files (and packages) may be chosen.
    #[must_use]
    pub fn can_choose_files(mut self, files: bool) -> Self {
        self.options.can_choose_files = files;
        self
    }

    /// Whether folders may be chosen. When false, Open on a folder enters it.
    #[must_use]
    pub fn can_choose_directories(mut self, directories: bool) -> Self {
        self.options.can_choose_directories = directories;
        self
    }

    /// Allow only these extensions (without the dot). Files that do not match
    /// stay visible but cannot be chosen.
    #[must_use]
    pub fn allowed_types(mut self, types: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.options.allowed_types = types.into_iter().map(Into::into).collect();
        self.options.allows_other_types = false;
        self
    }

    #[must_use]
    pub fn allows_other_types(mut self, allows: bool) -> Self {
        self.options.allows_other_types = allows;
        self
    }

    /// Persist the view mode and sidebar width in the application's
    /// preferences entity.
    #[must_use]
    pub fn state_store(mut self, store: Rc<dyn PanelStateStore>) -> Self {
        self.options.state_store = Some(store);
        self
    }

    /// Restore and persist the Open window's frame through the application's
    /// frame store (HIG p. 245, "remember").
    #[must_use]
    pub fn frame_store(mut self, store: Rc<dyn crate::window_role::FrameStore>) -> Self {
        self.options.frame_store = Some(store);
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

    #[must_use]
    pub fn on_finish(mut self, f: impl Fn(&PanelOutcome, &mut App) + 'static) -> Self {
        self.options.on_finish = Some(Rc::new(f));
        self
    }

    /// File > Open: an application-modal framed window (HIG p. 245).
    pub fn run_modal(self, window: &mut Window, cx: &mut App) -> Entity<FilePanel> {
        let title = self.title.to_string();
        let panel = FilePanel::build(self.options, self.provider, window, cx);
        panel::run_modal(panel, &title, window, cx)
    }

    /// Choose-style presentation attached to a document (HIG p. 251).
    pub fn begin_sheet(
        self,
        owner: AnyWindowHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<FilePanel> {
        let title = self.title.to_string();
        let panel = FilePanel::build(self.options, self.provider, window, cx);
        panel::begin_sheet(panel, owner, &title, window, cx)
    }
}
