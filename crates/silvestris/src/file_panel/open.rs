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
//!     .present(Some(owner), window, cx);
//! ```
//!
//! Double-click enters a folder or accepts a file. Open is disabled while the
//! selection contains anything the flags do not allow.

use std::rc::Rc;

use gpui::{AnyWindowHandle, App, Entity, Window};

use super::panel::{self, FilePanel, PanelOptions, PanelOutcome};
use super::provider::FileProvider;

pub struct OpenPanel {
    provider: Rc<dyn FileProvider>,
    options: PanelOptions,
}

impl OpenPanel {
    #[must_use]
    pub fn new(provider: Rc<dyn FileProvider>) -> Self {
        Self {
            provider,
            options: PanelOptions::new(super::panel::PanelMode::Open, "".into()),
        }
    }

    #[must_use]
    pub fn directory(mut self, location: impl Into<super::LocationId>) -> Self {
        self.options.location = location.into();
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

    pub fn present(
        self,
        owner: Option<AnyWindowHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<FilePanel> {
        let panel = FilePanel::build(self.options, self.provider, window, cx);
        panel::present(panel, owner, "Open", window, cx)
    }
}
