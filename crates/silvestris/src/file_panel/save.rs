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
//!     .present(Some(owner), window, cx);
//! ```
//!
//! The panel returns a destination; it never writes a file. `present` uses a
//! document sheet when `owner` is given, and an application-modal dialog when
//! it is `None`.

use std::rc::Rc;

use gpui::{AnyWindowHandle, App, Entity, Window};

use super::panel::{self, FilePanel, PanelOptions, PanelOutcome};
use super::provider::FileProvider;

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

    /// The suggested name. For a Save As the field selects the base name and
    /// leaves the extension unselected, as 10.6 does.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.options.name = name.into();
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

    /// Start with "Hide extension" checked.
    #[must_use]
    pub fn hide_extension(mut self, hide: bool) -> Self {
        self.options.hide_extension = hide;
        self
    }

    /// Start with the file browser expanded (the disclosure).
    #[must_use]
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.options.expanded = expanded;
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

    /// Show the panel. `owner` is the document window for a sheet; `None`
    /// opens an application-modal dialog.
    pub fn present(
        self,
        owner: Option<AnyWindowHandle>,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<FilePanel> {
        let panel = FilePanel::build(self.options, self.provider, window, cx);
        panel::present(panel, owner, "Save", window, cx)
    }
}
