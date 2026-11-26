//! One host for the three file presentations (#78/#33/#34/#35).
//!
//! `FileViewHost` is deliberately thin: `FileBrowserModel` already owns the
//! current location, history, filter, sort and stable selection. The host gives
//! icon/list/column presentations one switch entry point, so a consumer cannot
//! create three competing navigation models or reset to the home folder when
//! the view changes.

use gpui::{App, Context, Entity};

use super::model::{FileBrowserModel, FileSort, FileViewMode};
use crate::binding::ItemId;
use crate::controls::path_control::LocationId;

#[derive(Clone)]
pub struct FileViewHost {
    model: Entity<FileBrowserModel>,
}

impl FileViewHost {
    pub fn new(model: Entity<FileBrowserModel>) -> Self {
        Self { model }
    }

    pub fn model(&self) -> Entity<FileBrowserModel> {
        self.model.clone()
    }

    pub fn view(&self, cx: &App) -> FileViewMode {
        self.model.read(cx).view()
    }

    /// Switch presentation. Location, history, sort and the current selection
    /// survive; only the presentation changes.
    pub fn set_view(&mut self, view: FileViewMode, cx: &mut Context<Self>) {
        self.model.update(cx, |model, cx| model.set_view(view, cx));
        cx.notify();
    }

    pub fn show_icons(&mut self, cx: &mut Context<Self>) {
        self.set_view(FileViewMode::Icons, cx);
    }

    pub fn show_list(&mut self, cx: &mut Context<Self>) {
        self.set_view(FileViewMode::List, cx);
    }

    pub fn show_columns(&mut self, cx: &mut Context<Self>) {
        self.set_view(FileViewMode::Columns, cx);
    }

    pub fn location(&self, cx: &App) -> LocationId {
        self.model.read(cx).location().clone()
    }

    pub fn selected_ids(&self, cx: &App) -> Vec<ItemId> {
        self.model.read(cx).selected_ids(cx)
    }

    pub fn sort(&self, cx: &App) -> Option<FileSort> {
        self.model.read(cx).sort()
    }
}
