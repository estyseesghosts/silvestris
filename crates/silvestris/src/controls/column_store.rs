//! Table column autosave (#32): column order, widths and hidden columns
//! persist under an autosave key through the app's preference store, the
//! same seam window frames use (`window_role::FrameStore`).
use std::cell::RefCell;
use std::collections::BTreeMap;

use gpui::{App, Entity};

/// One table's saved column layout.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedColumns {
    /// Column ids, left to right. Ids missing here keep their declared order
    /// after the saved ones.
    pub order: Vec<String>,
    /// Widths in logical pixels by column id.
    pub widths: BTreeMap<String, f32>,
    /// Hidden column ids.
    pub hidden: Vec<String>,
}

/// Saved layouts by autosave key. Put one field of this type in the app's
/// preferences document.
pub type SavedTables = BTreeMap<String, SavedColumns>;

/// Where a table reads and writes its column layout.
pub trait ColumnStore {
    fn load(&self, key: &str, cx: &App) -> Option<SavedColumns>;
    fn save(&self, key: &str, columns: SavedColumns, cx: &mut App);
}

/// Keeps layouts in memory: for tests and for apps without a preference
/// document.
#[derive(Default)]
pub struct MemoryColumns {
    tables: RefCell<SavedTables>,
}

impl ColumnStore for MemoryColumns {
    fn load(&self, key: &str, _: &App) -> Option<SavedColumns> {
        self.tables.borrow().get(key).cloned()
    }
    fn save(&self, key: &str, columns: SavedColumns, _: &mut App) {
        self.tables.borrow_mut().insert(key.to_string(), columns);
    }
}

/// A [`ColumnStore`] that writes through the app's #62 `Preferences`
/// entity, so layouts are versioned and migrated with the rest of its
/// settings.
pub struct PreferenceColumns<T: silvestris_core::prefs::Versioned> {
    prefs: Entity<crate::prefs::Preferences<T>>,
    tables: fn(&T) -> &SavedTables,
    tables_mut: fn(&mut T) -> &mut SavedTables,
}

impl<T> PreferenceColumns<T>
where
    T: silvestris_core::prefs::Versioned + Clone + PartialEq + 'static,
{
    /// `tables` and `tables_mut` select the `SavedTables` field.
    #[must_use]
    pub fn new(
        prefs: Entity<crate::prefs::Preferences<T>>,
        tables: fn(&T) -> &SavedTables,
        tables_mut: fn(&mut T) -> &mut SavedTables,
    ) -> Self {
        Self {
            prefs,
            tables,
            tables_mut,
        }
    }
}

impl<T> ColumnStore for PreferenceColumns<T>
where
    T: silvestris_core::prefs::Versioned + Clone + PartialEq + 'static,
{
    fn load(&self, key: &str, cx: &App) -> Option<SavedColumns> {
        (self.tables)(self.prefs.read(cx).value()).get(key).cloned()
    }

    fn save(&self, key: &str, columns: SavedColumns, cx: &mut App) {
        let tables_mut = self.tables_mut;
        let key = key.to_string();
        self.prefs.update(cx, |prefs, cx| {
            // A read-only store keeps the layout in memory; the preferences
            // entity already reports that write failure.
            let _ = prefs.set(
                &format!("table.columns.{key}"),
                |value| {
                    tables_mut(value).insert(key.clone(), columns.clone());
                },
                cx,
            );
        });
    }
}
