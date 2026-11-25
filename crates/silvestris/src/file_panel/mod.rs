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
#[cfg(feature = "std-fs")]
pub mod std_fs;

pub use crate::controls::path_control::LocationId;
pub use model::{
    choosable, compare_kind_folders_first, place_icon, FileBrowserEvent, FileBrowserModel,
    FileViewMode,
};
pub use open::OpenPanel;
pub use panel::{
    begin_sheet, run_modal, ExtensionPolicy, FilePanel, FormatOption, PanelMode, PanelOptions,
    PanelOutcome, SaveDisposition,
};
pub use provider::{
    modified_seconds, validate_structural_name, FileAncestor, FileEntry, FileError, FileKind,
    FileListing, FilePlace, FileProvider, FileProviderCapabilities, MemoryProvider, PlaceSection,
};
pub use save::SavePanel;
pub use state::{MemoryPanelStateStore, PanelState, PanelStateStore, PreferencePanelStates};

#[cfg(feature = "std-fs")]
pub use std_fs::StdFsProvider;
