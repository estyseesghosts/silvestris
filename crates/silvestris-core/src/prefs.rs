//! Shell preferences and safe persistence.
//!
//! The consumer owns the storage location. Rules from handbook 14/19 and Lotus
//! preferences.rs: a file that fails to parse is never overwritten; unknown
//! fields are preserved; writes go to a sibling temporary file and are renamed
//! into place so a crash cannot leave a truncated file.
//!
//! `Versioned` adds ordered migrations on the raw JSON value, so a migration
//! can rename or reshape fields while unknown/newer fields stay untouched.
//! `Storage` is the seam for an application-provided backend; the default is a
//! JSON file. Change notification lives in the GPUI wrapper
//! (`silvestris::prefs::Preferences`) because it needs the app context.

use std::path::{Path, PathBuf};

use serde::{de::DeserializeOwned, Deserialize, Serialize};

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

/// A preferences type that carries its schema version.
pub trait Versioned: Serialize + DeserializeOwned + Default {
    fn schema_version(&self) -> u32;
    fn set_schema_version(&mut self, version: u32);
}

impl Versioned for ShellPreferences {
    fn schema_version(&self) -> u32 {
        self.schema_version
    }

    fn set_schema_version(&mut self, version: u32) {
        self.schema_version = version;
    }
}

/// One ordered migration on the raw JSON object. `from` is the version the
/// document must be at for the migration to run; the result is `to`.
#[derive(Clone, Copy, Debug)]
pub struct Migration {
    pub from: u32,
    pub to: u32,
    pub apply: fn(&mut serde_json::Map<String, serde_json::Value>) -> Result<(), String>,
}

impl Migration {
    #[must_use]
    pub const fn new(
        from: u32,
        to: u32,
        apply: fn(&mut serde_json::Map<String, serde_json::Value>) -> Result<(), String>,
    ) -> Self {
        Self { from, to, apply }
    }
}

/// Apply migrations in order until the document reaches the target version.
/// A gap (no migration from the current version) is an error, not a silent
/// partial upgrade: saving a half-migrated document would lose data.
pub fn migrate(
    value: &mut serde_json::Map<String, serde_json::Value>,
    from: u32,
    target: u32,
    migrations: &[Migration],
) -> Result<u32, String> {
    let mut current = from;
    let mut steps = 0;
    while current < target {
        let Some(migration) = migrations.iter().find(|m| m.from == current) else {
            return Err(format!(
                "no migration from schema version {current} to {target}"
            ));
        };
        (migration.apply)(value)?;
        current = migration.to;
        steps += 1;
        if steps > 64 {
            return Err("migration chain does not terminate".into());
        }
    }
    Ok(current)
}

/// A preferences backend. Read and write whole documents; the caller owns the
/// format. The default implementation is [`FileStorage`].
pub trait Storage {
    fn load(&self) -> std::io::Result<Option<String>>;
    fn save(&self, text: &str) -> std::io::Result<()>;
    fn describe(&self) -> String;
}

impl Storage for Box<dyn Storage> {
    fn load(&self) -> std::io::Result<Option<String>> {
        (**self).load()
    }

    fn save(&self, text: &str) -> std::io::Result<()> {
        (**self).save(text)
    }

    fn describe(&self) -> String {
        (**self).describe()
    }
}

/// A JSON file backend. Writes a sibling temporary file and renames it into
/// place so a crash cannot leave a truncated file.
#[derive(Clone, Debug)]
pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Storage for FileStorage {
    fn load(&self) -> std::io::Result<Option<String>> {
        match std::fs::read_to_string(&self.path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            other => other.map(Some),
        }
    }

    fn save(&self, text: &str) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &self.path)
    }

    fn describe(&self) -> String {
        self.path.display().to_string()
    }
}

/// Result of reading a preferences file.
#[derive(Debug)]
pub enum Loaded<T> {
    /// No file yet: defaults, and saving is allowed.
    Missing(T),
    Ok(T),
    /// The file exists but does not parse. Defaults are returned and the
    /// caller must not save over it (`Store::writable` is false).
    Malformed {
        value: T,
        error: String,
    },
}

/// A preferences document that refuses to clobber a file it could not read.
pub struct Store<S: Storage = FileStorage> {
    storage: S,
    writable: bool,
    version: Option<u32>,
}

impl<S: Storage> std::fmt::Debug for Store<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("storage", &self.storage.describe())
            .field("writable", &self.writable)
            .field("version", &self.version)
            .finish()
    }
}

impl Store<FileStorage> {
    /// Open a JSON file. `Loaded::Malformed` means the file exists and did not
    /// parse; saving is disabled until the application resolves it.
    pub fn open<T: for<'de> Deserialize<'de> + Default>(
        path: impl Into<PathBuf>,
    ) -> (Self, Loaded<T>) {
        Self::with_storage(FileStorage::new(path))
    }

    /// Open a versioned JSON file and run ordered migrations.
    pub fn open_versioned<T: Versioned>(
        path: impl Into<PathBuf>,
        migrations: &[Migration],
    ) -> (Self, Loaded<T>) {
        Self::with_storage_versioned(FileStorage::new(path), migrations)
    }
}

impl<S: Storage> Store<S> {
    /// Use an application-provided backend.
    pub fn with_storage<T: for<'de> Deserialize<'de> + Default>(storage: S) -> (Self, Loaded<T>) {
        let (store, value) = Self::read(storage, None);
        (store, value)
    }

    /// Use an application-provided backend and run ordered migrations.
    pub fn with_storage_versioned<T: Versioned>(
        storage: S,
        migrations: &[Migration],
    ) -> (Self, Loaded<T>) {
        let target = T::default().schema_version();
        let (store, loaded): (Self, Loaded<T>) =
            Self::read_with_migrations(storage, target, migrations);
        (store, loaded)
    }

    fn read<T: for<'de> Deserialize<'de> + Default>(
        storage: S,
        _version: Option<u32>,
    ) -> (Self, Loaded<T>) {
        let mut store = Self {
            storage,
            writable: true,
            version: None,
        };
        let loaded = match store.storage.load() {
            Err(e) => {
                store.writable = false;
                Loaded::Malformed {
                    value: T::default(),
                    error: e.to_string(),
                }
            }
            Ok(None) => Loaded::Missing(T::default()),
            Ok(Some(text)) => match serde_json::from_str(&text) {
                Ok(value) => Loaded::Ok(value),
                Err(e) => {
                    store.writable = false;
                    Loaded::Malformed {
                        value: T::default(),
                        error: e.to_string(),
                    }
                }
            },
        };
        (store, loaded)
    }

    fn read_with_migrations<T: Versioned>(
        storage: S,
        target: u32,
        migrations: &[Migration],
    ) -> (Self, Loaded<T>) {
        let mut store = Self {
            storage,
            writable: true,
            version: None,
        };
        let loaded = match store.storage.load() {
            Err(e) => {
                store.writable = false;
                Loaded::Malformed {
                    value: T::default(),
                    error: e.to_string(),
                }
            }
            Ok(None) => Loaded::Missing(T::default()),
            Ok(Some(text)) => match serde_json::from_str::<serde_json::Value>(&text) {
                Err(e) => {
                    store.writable = false;
                    Loaded::Malformed {
                        value: T::default(),
                        error: e.to_string(),
                    }
                }
                Ok(serde_json::Value::Object(mut value)) => {
                    let from = value
                        .get("schema_version")
                        .and_then(serde_json::Value::as_u64)
                        .map_or(0, |v| v as u32);
                    let migrated = if from == 0 || from < target {
                        migrate(&mut value, from, target, migrations).map(|_| (target, true))
                    } else {
                        // A newer document is left exactly as written. Silvestris
                        // must not downgrade fields it does not understand.
                        Ok((from, false))
                    };
                    match migrated.and_then(|(version, did_migrate)| {
                        if did_migrate {
                            value.insert("schema_version".into(), serde_json::json!(version));
                        }
                        serde_json::from_value::<T>(serde_json::Value::Object(value))
                            .map(|value| (value, version))
                            .map_err(|e| e.to_string())
                    }) {
                        Ok((value, version)) => {
                            store.version = Some(version);
                            Loaded::Ok(value)
                        }
                        Err(error) => {
                            store.writable = false;
                            Loaded::Malformed {
                                value: T::default(),
                                error,
                            }
                        }
                    }
                }
                Ok(_) => {
                    store.writable = false;
                    Loaded::Malformed {
                        value: T::default(),
                        error: "preferences root is not a JSON object".into(),
                    }
                }
            },
        };
        (store, loaded)
    }

    #[must_use]
    pub fn writable(&self) -> bool {
        self.writable
    }

    #[must_use]
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Replace the backend while keeping the document state. Used to erase the
    /// storage type behind `Box<dyn Storage>` in the GPUI wrapper.
    #[must_use]
    pub fn map_storage<S2: Storage>(self, f: impl FnOnce(S) -> S2) -> Store<S2> {
        Store {
            storage: f(self.storage),
            writable: self.writable,
            version: self.version,
        }
    }

    /// The version this store was opened at, when it is versioned.
    #[must_use]
    pub fn version(&self) -> Option<u32> {
        self.version
    }

    /// Atomically replace the document. Refuses if the original was malformed.
    pub fn save<T: Serialize>(&self, value: &T) -> std::io::Result<()> {
        if !self.writable {
            return Err(std::io::Error::other(
                "preferences file did not parse; refusing to overwrite it",
            ));
        }
        let text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
        self.storage.save(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("silvestris-prefs-{}-{name}", std::process::id()));
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

    #[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    struct Demo {
        schema_version: u32,
        name: String,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    }

    impl Default for Demo {
        fn default() -> Self {
            Self {
                schema_version: 2,
                name: "default".into(),
                extra: Default::default(),
            }
        }
    }

    impl Versioned for Demo {
        fn schema_version(&self) -> u32 {
            self.schema_version
        }

        fn set_schema_version(&mut self, version: u32) {
            self.schema_version = version;
        }
    }

    fn rename_name(value: &mut serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
        if let Some(old) = value.remove("title") {
            value.insert("name".into(), old);
        }
        Ok(())
    }

    fn add_version(value: &mut serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
        value.insert("schema_version".into(), serde_json::json!(1));
        Ok(())
    }

    #[test]
    fn ordered_migrations_run_and_preserve_unknown_fields() {
        let p = temp("migrate");
        std::fs::write(
            &p,
            r#"{"title":"lotus","desktop":{"schema_version":1},"unknown":[1]}"#,
        )
        .unwrap();
        let migrations = [
            Migration::new(0, 1, add_version),
            Migration::new(1, 2, rename_name),
        ];
        let (store, loaded) = Store::open_versioned::<Demo>(&p, &migrations);
        let Loaded::Ok(value) = loaded else {
            panic!("{loaded:?}")
        };
        assert_eq!(value.schema_version, 2);
        assert_eq!(value.name, "lotus");
        assert_eq!(value.extra["unknown"], serde_json::json!([1]));
        assert!(store.writable());
        store.save(&value).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("\"schema_version\": 2"), "{text}");
        assert!(text.contains("unknown"), "{text}");
    }

    #[test]
    fn a_migration_gap_disables_saving_and_keeps_the_file() {
        let p = temp("gap");
        let original = r#"{"schema_version":1,"name":"x"}"#;
        std::fs::write(&p, original).unwrap();
        let (store, loaded) = Store::open_versioned::<Demo>(&p, &[]);
        assert!(matches!(loaded, Loaded::Malformed { .. }));
        assert!(!store.writable());
        assert!(store.save(&Demo::default()).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), original);
    }

    #[test]
    fn a_future_version_is_kept_as_is_and_writable() {
        let p = temp("future");
        std::fs::write(&p, r#"{"schema_version":9,"name":"newer"}"#).unwrap();
        let (store, loaded) = Store::open_versioned::<Demo>(&p, &[]);
        let Loaded::Ok(value) = loaded else {
            panic!("{loaded:?}")
        };
        assert_eq!(value.schema_version, 9);
        assert!(store.writable());
    }

    #[derive(Default)]
    struct Memory(std::cell::RefCell<Option<String>>);

    impl Storage for Memory {
        fn load(&self) -> std::io::Result<Option<String>> {
            Ok(self.0.borrow().clone())
        }

        fn save(&self, text: &str) -> std::io::Result<()> {
            *self.0.borrow_mut() = Some(text.to_string());
            Ok(())
        }

        fn describe(&self) -> String {
            "memory".into()
        }
    }

    #[test]
    fn an_application_backend_can_replace_the_file() {
        let (store, loaded) = Store::with_storage::<Demo>(Memory::default());
        assert!(matches!(loaded, Loaded::Missing(_)));
        store.save(&Demo::default()).unwrap();
        let text = store.storage().0.borrow().clone().unwrap();
        assert!(text.contains("\"name\": \"default\""), "{text}");
        assert_eq!(store.storage().describe(), "memory");
    }
}
