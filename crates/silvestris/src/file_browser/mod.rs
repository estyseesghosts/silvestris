//! Neutral file-browser core (#78).
//!
//! The provider protocol, entry data, browser model, view mode, one shared
//! sort policy and navigation events live here. Open/Save panels consume this
//! layer; a future Finder-style application instantiates it directly and must
//! not create an `OpenPanel` as its root.
//!
//! This layer never touches `std::fs`, sheets, dialogs, buttons, extension
//! policy or modal state. Identity is provider-owned: `LocationId` is opaque
//! and display names are never turned back into paths.

pub mod model;
pub mod provider;
#[cfg(feature = "std-fs")]
pub mod std_fs;

pub use model::{
    choosable, compare_kind_folders_first, place_icon, FileBrowserEvent, FileBrowserModel,
    FileSort, FileSortKey, FileViewMode,
};
pub use provider::{
    modified_seconds, validate_structural_name, FileAncestor, FileEntry, FileError, FileKind,
    FileListing, FileMetadata, FilePlace, FileProvider, FileProviderCapabilities, MemoryProvider,
    MetadataUpdates, PlaceSection,
};
#[cfg(feature = "std-fs")]
pub use std_fs::StdFsProvider;
