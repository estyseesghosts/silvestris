//! Open, Save and Choose panels (#29).
//!
//! The panel is composition and policy: `FileProvider` supplies data,
//! `FileBrowserModel` owns the location/history/selection, and generic
//! Silvestris controls draw the toolbar, sidebar, browser, name field and
//! buttons. The panel never reads a filesystem and never writes a file; it
//! returns the chosen host paths or location ids and the consumer does the
//! I/O.

pub mod chrome;
pub mod model;
pub mod open;
pub mod panel;
pub mod provider;
pub mod save;
pub mod state;

pub use crate::controls::path_control::LocationId;
pub use model::{
    choosable, compare_kind_folders_first, place_icon, FileBrowserEvent, FileBrowserModel,
    FileSort, FileSortKey, FileViewMode,
};
pub use open::OpenPanel;
pub use panel::{
    begin_sheet, run_modal, ExtensionPolicy, FilePanel, FormatOption, PanelMode, PanelOptions,
    PanelOutcome, SaveDisposition,
};
pub use provider::{
    modified_seconds, validate_structural_name, FileAncestor, FileEntry, FileError, FileKind,
    FileListing, FileMetadata, FilePlace, FileProvider, FileProviderCapabilities, MemoryProvider,
    PlaceSection,
};
pub use save::SavePanel;
pub use state::{MemoryPanelStateStore, PanelState, PanelStateStore, PreferencePanelStates};

#[cfg(feature = "std-fs")]
pub use crate::file_browser::StdFsProvider;

/// Panel-owned extension policy. The browser core deliberately knows nothing
/// about allowed extensions; Open dims/disables non-matching files, and Save
/// uses the list for its extension disposition.
#[must_use]
pub fn matches_types(entry: &FileEntry, types: &[String], allows_other: bool) -> bool {
    if entry.is_dir || allows_other || types.is_empty() {
        return true;
    }
    let extension = entry.extension();
    types.iter().any(|allowed| {
        allowed
            .trim_start_matches('.')
            .eq_ignore_ascii_case(&extension)
    })
}
