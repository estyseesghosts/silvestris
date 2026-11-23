//! Observable preferences (#62).
//!
//! `silvestris_core::prefs::Store` owns the document, the version and the
//! safety rules. This wrapper adds the GPUI half: an entity other views can
//! observe, and keyed change callbacks for controls (and #61 bindings) that
//! only care about one field.
//!
//! The application decides where the document lives. `Preferences::open` uses a
//! JSON file; `Preferences::with_storage` takes any
//! [`silvestris_core::prefs::Storage`], so a consumer can keep its own format,
//! database or test memory.
//!
//! A change writes through immediately when the store is writable. A malformed
//! file disables saving (the core store refuses to overwrite it) and `set`
//! reports that no write happened. Callers still get the in-memory value.
//!
//! Key callbacks fire after the value changed and before `cx.notify`; a
//! callback that needs another window to repaint should change state there
//! through its own entity.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use gpui::{App, AppContext, Context, Entity};
use silvestris_core::prefs::{Loaded, Migration, Storage, Store, Versioned};

type Callback<T> = Rc<dyn Fn(&T)>;

/// An application preferences document other views can observe.
pub struct Preferences<T: Versioned> {
    store: Store<Box<dyn Storage>>,
    value: T,
    keyed: BTreeMap<String, Vec<Callback<T>>>,
    all: Vec<Callback<T>>,
}

impl<T: Versioned + Clone + PartialEq + 'static> Preferences<T> {
    /// Open a JSON file and run the ordered migrations. The `Loaded` value is
    /// returned beside the entity so the caller can distinguish a missing file
    /// from a malformed one before showing the UI.
    pub fn open(
        path: impl Into<PathBuf>,
        migrations: &[Migration],
        cx: &mut App,
    ) -> (Entity<Self>, Loaded<T>) {
        let (store, loaded) = Store::open_versioned::<T>(path, migrations);
        let value = match &loaded {
            Loaded::Missing(value) | Loaded::Ok(value) | Loaded::Malformed { value, .. } => {
                value.clone()
            }
        };
        let entity = cx.new(|_| Self {
            store: store.map_storage(|storage| Box::new(storage) as Box<dyn Storage>),
            value,
            keyed: BTreeMap::new(),
            all: Vec::new(),
        });
        (entity, loaded)
    }

    /// Use an application-provided backend.
    pub fn with_storage(
        storage: Box<dyn Storage>,
        migrations: &[Migration],
        cx: &mut App,
    ) -> (Entity<Self>, Loaded<T>) {
        let (store, loaded) = Store::with_storage_versioned::<T>(storage, migrations);
        let value = match &loaded {
            Loaded::Missing(value) | Loaded::Ok(value) | Loaded::Malformed { value, .. } => {
                value.clone()
            }
        };
        let entity = cx.new(|_| Self {
            store: store.map_storage(|storage| Box::new(storage) as Box<dyn Storage>),
            value,
            keyed: BTreeMap::new(),
            all: Vec::new(),
        });
        (entity, loaded)
    }

    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Whether the document may be written. False after a malformed read.
    #[must_use]
    pub fn writable(&self) -> bool {
        self.store.writable()
    }

    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.store
            .version()
            .unwrap_or_else(|| self.value.schema_version())
    }

    /// Change one field and notify observers. `key` labels the changed setting
    /// for keyed callbacks ("theme", "sidebar.width", ...).
    ///
    /// Returns whether the value changed. A failed write keeps the in-memory
    /// value and returns the error so the caller can surface it.
    pub fn set(
        &mut self,
        key: &str,
        change: impl FnOnce(&mut T),
        cx: &mut Context<Self>,
    ) -> Result<bool, std::io::Error> {
        let before = self.value.clone();
        change(&mut self.value);
        if self.value == before {
            return Ok(false);
        }
        let saved = self.store.save(&self.value);
        self.notify(key, cx);
        saved.map(|()| true)
    }

    /// Register a callback for one key.
    pub fn on_change(&mut self, key: impl Into<String>, f: impl Fn(&T) + 'static) {
        self.keyed.entry(key.into()).or_default().push(Rc::new(f));
    }

    /// Register a callback for every change.
    pub fn subscribe(&mut self, f: impl Fn(&T) + 'static) {
        self.all.push(Rc::new(f));
    }

    /// Write the current value without changing it.
    pub fn save(&self) -> std::io::Result<()> {
        self.store.save(&self.value)
    }

    fn notify(&self, key: &str, cx: &mut Context<Self>) {
        if let Some(callbacks) = self.keyed.get(key) {
            for callback in callbacks {
                callback(&self.value);
            }
        }
        for callback in &self.all {
            callback(&self.value);
        }
        // Every observer (and therefore every bound control) re-reads.
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use gpui::TestAppContext;
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Demo {
        schema_version: u32,
        theme: String,
        sidebar: f32,
        #[serde(flatten)]
        extra: serde_json::Map<String, serde_json::Value>,
    }

    impl Default for Demo {
        fn default() -> Self {
            Self {
                schema_version: 1,
                theme: String::new(),
                sidebar: 0.0,
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

    fn demo_version(value: &mut serde_json::Map<String, serde_json::Value>) -> Result<(), String> {
        value.insert("schema_version".into(), serde_json::json!(1));
        Ok(())
    }

    #[derive(Default)]
    struct Memory(RefCell<Option<String>>);

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

    fn app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app
    }

    #[test]
    fn one_key_notifies_its_own_subscribers_and_observers() {
        let mut app = app();
        let (prefs, loaded) = app.update(|cx| {
            let (prefs, loaded) =
                Preferences::<Demo>::with_storage(Box::new(Memory::default()), &[], cx);
            (prefs, loaded)
        });
        assert!(matches!(loaded, Loaded::Missing(_)));
        let theme_hits = Rc::new(Cell::new(0));
        let sidebar_hits = Rc::new(Cell::new(0));
        let all_hits = Rc::new(Cell::new(0));
        let t = theme_hits.clone();
        let s = sidebar_hits.clone();
        let a = all_hits.clone();
        prefs.update(&mut app, |prefs, cx| {
            prefs.on_change("theme", move |_| t.set(t.get() + 1));
            prefs.on_change("sidebar.width", move |_| s.set(s.get() + 1));
            prefs.subscribe(move |_| a.set(a.get() + 1));
            prefs
                .set("theme", |value: &mut Demo| value.theme = "dark".into(), cx)
                .unwrap();
        });
        assert_eq!(theme_hits.get(), 1);
        assert_eq!(sidebar_hits.get(), 0);
        assert_eq!(all_hits.get(), 1);
        prefs.update(&mut app, |prefs, cx| {
            prefs
                .set("theme", |value: &mut Demo| value.theme = "dark".into(), cx)
                .unwrap();
        });
        assert_eq!(theme_hits.get(), 1, "an unchanged value does not notify");
    }

    #[test]
    fn a_change_is_written_through_the_application_backend() {
        let mut app = app();
        let memory = Rc::new(Memory::default());
        let backend = memory.clone();
        let (prefs, _) =
            app.update(|cx| Preferences::<Demo>::with_storage(Box::new(Boxed(backend)), &[], cx));
        prefs.update(&mut app, |prefs, cx| {
            prefs
                .set(
                    "sidebar.width",
                    |value: &mut Demo| value.sidebar = 300.0,
                    cx,
                )
                .unwrap();
        });
        let text = memory.0.borrow().clone().expect("saved");
        assert!(text.contains("\"sidebar\": 300.0"), "{text}");
    }

    struct Boxed(Rc<Memory>);

    impl Storage for Boxed {
        fn load(&self) -> std::io::Result<Option<String>> {
            self.0.load()
        }

        fn save(&self, text: &str) -> std::io::Result<()> {
            self.0.save(text)
        }

        fn describe(&self) -> String {
            self.0.describe()
        }
    }

    #[test]
    fn a_migration_upgrades_the_entity_and_keeps_newer_fields() {
        let mut app = app();
        let memory = Rc::new(Memory(RefCell::new(Some(
            r#"{"theme":"graphite","sidebar":280.0,"future":{"a":1}}"#.into(),
        ))));
        let backend = memory.clone();
        let migrations = [Migration::new(0, 1, demo_version)];
        let (prefs, loaded) = app.update(|cx| {
            Preferences::<Demo>::with_storage(Box::new(Boxed(backend)), &migrations, cx)
        });
        let Loaded::Ok(value) = loaded else {
            panic!("{loaded:?}")
        };
        assert_eq!(value.schema_version, 1);
        assert_eq!(value.theme, "graphite");
        assert_eq!(value.sidebar, 280.0);
        assert_eq!(value.extra["future"], serde_json::json!({"a": 1}));
        prefs.update(&mut app, |prefs, cx| {
            prefs
                .set("theme", |value: &mut Demo| value.theme = "aqua".into(), cx)
                .unwrap();
        });
        let text = memory.0.borrow().clone().unwrap();
        assert!(text.contains("\"future\""), "{text}");
        assert!(text.contains("\"schema_version\": 1"), "{text}");
    }

    #[test]
    fn a_malformed_document_disables_writes_and_keeps_the_bytes() {
        let mut app = app();
        let memory = Rc::new(Memory(RefCell::new(Some("{not json".into()))));
        let backend = memory.clone();
        let (prefs, loaded) =
            app.update(|cx| Preferences::<Demo>::with_storage(Box::new(Boxed(backend)), &[], cx));
        assert!(matches!(loaded, Loaded::Malformed { .. }));
        assert!(!app.update(|cx| prefs.read(cx).writable()));
        let result = prefs.update(&mut app, |prefs, cx| {
            prefs.set("theme", |value: &mut Demo| value.theme = "x".into(), cx)
        });
        assert!(result.is_err());
        assert_eq!(memory.0.borrow().as_deref(), Some("{not json"));
    }
}
