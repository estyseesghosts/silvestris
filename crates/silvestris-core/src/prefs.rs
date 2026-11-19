//! Shell preferences and safe persistence.
//!
//! The consumer owns the file path. Rules from handbook 14/19 and Lotus
//! preferences.rs: a file that fails to parse is never overwritten; unknown
//! fields are preserved; writes go to a sibling temporary file and are
//! renamed into place so a crash cannot leave a truncated file.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::layout::{InspectorPreference, SidebarPreference};
use crate::theme::ThemePreference;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShellPreferences {
    #[serde(default = "schema")]
    pub schema_version: u32,
    #[serde(default)]
    pub theme: ThemePreference,
    #[serde(default = "default_sidebar")]
    pub sidebar: SidebarPreference,
    #[serde(default = "default_inspector")]
    pub inspector: InspectorPreference,
    /// Stable inspector panel identifier, not an index (handbook 19).
    #[serde(default)]
    pub inspector_panel: Option<String>,
    #[serde(default)]
    pub toolbar: ToolbarDisplay,
    /// Fields written by newer versions or by the consumer; kept verbatim.
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

fn schema() -> u32 {
    SCHEMA_VERSION
}
fn default_sidebar() -> SidebarPreference {
    SidebarPreference {
        expanded: true,
        width: 264.0,
    }
}
fn default_inspector() -> InspectorPreference {
    // New installs: overlay by default (#131).
    InspectorPreference {
        visible: false,
        pinned: false,
        width: 420.0,
    }
}

impl Default for ShellPreferences {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            theme: ThemePreference::default(),
            sidebar: default_sidebar(),
            inspector: default_inspector(),
            inspector_panel: None,
            toolbar: ToolbarDisplay::default(),
            extra: Default::default(),
        }
    }
}

/// HIG p. 255-264 toolbar display modes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolbarDisplay {
    #[default]
    IconAndText,
    IconOnly,
    TextOnly,
}

/// Result of reading a preferences file.
#[derive(Debug)]
pub enum Loaded<T> {
    /// No file yet: defaults, and saving is allowed.
    Missing(T),
    Ok(T),
    /// The file exists but does not parse. Defaults are returned and the
    /// caller must not save over it (`Store::writable` is false).
    Malformed { value: T, error: String },
}

/// A JSON preferences file that refuses to clobber a file it could not read.
#[derive(Debug)]
pub struct Store {
    path: PathBuf,
    writable: bool,
}

impl Store {
    pub fn open<T: for<'de> Deserialize<'de> + Default>(path: impl Into<PathBuf>) -> (Self, Loaded<T>) {
        let path = path.into();
        match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (Store { path, writable: true }, Loaded::Missing(T::default()))
            }
            Err(e) => (
                Store {
                    path,
                    writable: false,
                },
                Loaded::Malformed {
                    value: T::default(),
                    error: e.to_string(),
                },
            ),
            Ok(text) => match serde_json::from_str(&text) {
                Ok(v) => (Store { path, writable: true }, Loaded::Ok(v)),
                Err(e) => (
                    Store {
                        path,
                        writable: false,
                    },
                    Loaded::Malformed {
                        value: T::default(),
                        error: e.to_string(),
                    },
                ),
            },
        }
    }

    pub fn writable(&self) -> bool {
        self.writable
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically replace the file. Refuses if the original was malformed.
    pub fn save<T: Serialize>(&self, value: &T) -> std::io::Result<()> {
        if !self.writable {
            return Err(std::io::Error::other(
                "preferences file did not parse; refusing to overwrite it",
            ));
        }
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("silvestris-prefs-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("prefs.json")
    }

    #[test]
    fn malformed_files_are_never_overwritten() {
        let p = temp("malformed");
        std::fs::write(&p, "{ not json").unwrap();
        let (store, loaded) = Store::open::<ShellPreferences>(&p);
        assert!(matches!(loaded, Loaded::Malformed { .. }));
        assert!(store.save(&ShellPreferences::default()).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{ not json");
    }

    #[test]
    fn unknown_fields_round_trip_and_widths_survive() {
        let p = temp("roundtrip");
        std::fs::write(
            &p,
            r#"{"sidebar":{"expanded":false,"width":333},"future_field":[1,2]}"#,
        )
        .unwrap();
        let (store, loaded) = Store::open::<ShellPreferences>(&p);
        let Loaded::Ok(prefs) = loaded else { panic!() };
        assert_eq!(prefs.sidebar.width, 333.0);
        assert!(!prefs.sidebar.expanded);
        assert_eq!(prefs.inspector, default_inspector());
        store.save(&prefs).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("future_field"), "{text}");
    }
}
