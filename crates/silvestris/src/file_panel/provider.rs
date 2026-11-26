//! Transitional re-export of the generic file provider.
//!
//! The implementation moved to [`crate::file_browser::provider`]. Keep
//! importing `file_panel::provider` during the Chaus migration window.

pub use crate::file_browser::provider::*;
