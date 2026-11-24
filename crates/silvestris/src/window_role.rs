//! Window roles and their 10.6 policy (#20).
//!
//! A consumer picks a role instead of assembling window options by hand. The
//! role decides resizability, which title-bar buttons are live, default
//! placement, whether the window belongs to a parent and the autosave key for
//! frame persistence. Artwork stays in the theme; this module is policy.
//!
//! Evidence: HIG p. 191 types of windows, p. 197-198 title bar and buttons,
//! p. 227 panels, p. 235-237 dialogs and sheets, p. 244 preferences windows.
//! The utility title-bar height is an estimate; see `docs/exceptions.md`.

use gpui::{
    point, px, size, AnyWindowHandle, App, Bounds, Entity, Pixels, Render, Size, Window,
    WindowHandle, WindowKind, WindowOptions,
};
use silvestris_core::tokens::Theme;

/// A window's role in the application. Every Silvestris window picks one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowRole {
    /// Document window: resizable, close/minimize/zoom, title, frame autosave.
    Document,
    /// Utility window: resizable, close/minimize/zoom, small title bar.
    Utility,
    /// Panel or inspector: close only, floats above its document, hides on app
    /// deactivation where the platform supports it.
    Panel,
    /// Preferences: no resize control, no minimize or zoom (HIG p. 244).
    Preferences,
    /// Modeless dialog: close only, not resizable, centred on its parent.
    Dialog,
    /// Assistant: fixed size, close only, no minimize or zoom.
    Assistant,
    /// About: fixed size, close only.
    About,
}

impl WindowRole {
    pub const ALL: [WindowRole; 7] = [
        WindowRole::Document,
        WindowRole::Utility,
        WindowRole::Panel,
        WindowRole::Preferences,
        WindowRole::Dialog,
        WindowRole::Assistant,
        WindowRole::About,
    ];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            WindowRole::Document => "Document",
            WindowRole::Utility => "Utility",
            WindowRole::Panel => "Panel",
            WindowRole::Preferences => "Preferences",
            WindowRole::Dialog => "Dialog",
            WindowRole::Assistant => "Assistant",
            WindowRole::About => "About",
        }
    }

    /// Policy for this role. The flags are evidence, not preference: see the
    /// module docs for the HIG pages behind each one.
    #[must_use]
    pub const fn spec(self) -> RoleSpec {
        match self {
            WindowRole::Document => RoleSpec {
                resizable: true,
                close: true,
                minimize: true,
                zoom: true,
                shows_title: true,
                centers_on_parent: false,
                hides_on_deactivate: false,
                autosave: Some("document"),
                small_titlebar: false,
            },
            WindowRole::Utility => RoleSpec {
                resizable: true,
                close: true,
                minimize: true,
                zoom: true,
                shows_title: true,
                centers_on_parent: false,
                hides_on_deactivate: false,
                autosave: Some("utility"),
                small_titlebar: true,
            },
            WindowRole::Panel => RoleSpec {
                resizable: true,
                close: true,
                minimize: false,
                zoom: false,
                shows_title: true,
                centers_on_parent: false,
                hides_on_deactivate: true,
                autosave: Some("panel"),
                small_titlebar: false,
            },
            WindowRole::Preferences => RoleSpec {
                resizable: false,
                close: true,
                // 10.6 System Preferences keeps minimize active and disables
                // zoom (HIG p. 244 and the Appearance capture); see
                // `chrome::fixed_window_options`.
                minimize: true,
                zoom: false,
                shows_title: true,
                centers_on_parent: true,
                hides_on_deactivate: false,
                autosave: Some("preferences"),
                small_titlebar: false,
            },
            WindowRole::Dialog => RoleSpec {
                resizable: false,
                close: true,
                minimize: false,
                zoom: false,
                shows_title: true,
                centers_on_parent: true,
                hides_on_deactivate: false,
                autosave: None,
                small_titlebar: false,
            },
            WindowRole::Assistant => RoleSpec {
                resizable: false,
                close: true,
                minimize: false,
                zoom: false,
                shows_title: true,
                centers_on_parent: true,
                hides_on_deactivate: false,
                autosave: None,
                small_titlebar: false,
            },
            WindowRole::About => RoleSpec {
                resizable: false,
                close: true,
                minimize: false,
                zoom: false,
                shows_title: true,
                // An About window belongs to the application, not a document:
                // the caller centres it on the screen (HIG p. 234).
                centers_on_parent: false,
                hides_on_deactivate: false,
                autosave: None,
                small_titlebar: false,
            },
        }
    }

    /// Height of the drawn title row. Utility windows use a smaller band
    /// (HIG p. 198 figure); 16 px is an estimate, recorded in
    /// `docs/exceptions.md`.
    #[must_use]
    pub fn titlebar_height(self, theme: &Theme) -> f32 {
        if self.spec().small_titlebar {
            16.0
        } else {
            theme.metrics.titlebar_height
        }
    }

    /// Frame-autosave key for this role, when the window persists its frame.
    #[must_use]
    pub fn autosave_key(self) -> Option<&'static str> {
        self.spec().autosave
    }
}

/// The policy flags behind a role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleSpec {
    pub resizable: bool,
    pub close: bool,
    pub minimize: bool,
    pub zoom: bool,
    pub shows_title: bool,
    pub centers_on_parent: bool,
    pub hides_on_deactivate: bool,
    pub autosave: Option<&'static str>,
    pub small_titlebar: bool,
}

/// Options for `role`. `bounds` is the requested frame; a centered role keeps
/// its size and the caller centres it with [`centered_on`] when it has a
/// parent.
#[must_use]
pub fn role_options(
    role: WindowRole,
    title: impl Into<gpui::SharedString>,
    bounds: Bounds<Pixels>,
    cx: &App,
) -> WindowOptions {
    let spec = role.spec();
    let mut options = crate::chrome::window_options(title, bounds, default_minimum(), cx);
    options.is_resizable = spec.resizable;
    options.window_min_size = if spec.resizable {
        Some(default_minimum())
    } else {
        None
    };
    // A panel is a normal window on platforms without cross-application
    // floating; the capability struct records the difference.
    options.kind = WindowKind::Normal;
    options
}

fn default_minimum() -> Size<Pixels> {
    size(px(320.), px(200.))
}

/// Place a window of `size` in the centre of `parent`.
#[must_use]
pub fn centered_on(parent: Bounds<Pixels>, window: Size<Pixels>) -> Bounds<Pixels> {
    let x = parent.origin.x + (parent.size.width - window.width) / 2.0;
    let y = parent.origin.y + (parent.size.height - window.height) / 2.0;
    Bounds::new(point(x, y), window)
}

/// Offset a new window from the previous one. HIG p. 218 gives a 20 px step for
/// a window opened after another; the application decides when to cascade (a
/// new untitled document) versus restoring a saved frame. Cascading stops being
/// useful once the window would leave the display, which only the caller can
/// decide because it knows the screen.
#[must_use]
pub fn cascade_from(previous: Bounds<Pixels>, step: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(previous.origin.x + px(step), previous.origin.y + px(step)),
        previous.size,
    )
}

/// App-owned frame store. Silvestris never writes a preference file itself:
/// the application supplies the backend. [`PreferenceFrames`] is the adapter
/// over #62's `Preferences` entity; the app context lets a store update an
/// entity.
pub trait FrameStore {
    fn load(&self, key: &str, cx: &App) -> Option<Bounds<Pixels>>;
    fn save(&self, key: &str, bounds: Bounds<Pixels>, cx: &mut App);
}

/// One saved window frame, in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SavedFrame {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl From<Bounds<Pixels>> for SavedFrame {
    fn from(b: Bounds<Pixels>) -> Self {
        Self {
            x: f32::from(b.origin.x),
            y: f32::from(b.origin.y),
            width: f32::from(b.size.width),
            height: f32::from(b.size.height),
        }
    }
}

impl From<SavedFrame> for Bounds<Pixels> {
    fn from(f: SavedFrame) -> Self {
        Bounds::new(point(px(f.x), px(f.y)), size(px(f.width), px(f.height)))
    }
}

/// Saved frames by autosave key. Put one field of this type in the app's
/// preferences document.
pub type SavedFrames = std::collections::BTreeMap<String, SavedFrame>;

/// A [`FrameStore`] that writes through the app's #62 `Preferences` entity:
/// frames live in the app's own document, versioned and migrated with the rest
/// of its settings, and observers see the change like any other setting.
pub struct PreferenceFrames<T: silvestris_core::prefs::Versioned> {
    prefs: Entity<crate::prefs::Preferences<T>>,
    frames: fn(&T) -> &SavedFrames,
    frames_mut: fn(&mut T) -> &mut SavedFrames,
}

impl<T> PreferenceFrames<T>
where
    T: silvestris_core::prefs::Versioned + Clone + PartialEq + 'static,
{
    /// `frames` and `frames_mut` select the `SavedFrames` field.
    #[must_use]
    pub fn new(
        prefs: Entity<crate::prefs::Preferences<T>>,
        frames: fn(&T) -> &SavedFrames,
        frames_mut: fn(&mut T) -> &mut SavedFrames,
    ) -> Self {
        Self {
            prefs,
            frames,
            frames_mut,
        }
    }
}

impl<T> FrameStore for PreferenceFrames<T>
where
    T: silvestris_core::prefs::Versioned + Clone + PartialEq + 'static,
{
    fn load(&self, key: &str, cx: &App) -> Option<Bounds<Pixels>> {
        (self.frames)(self.prefs.read(cx).value())
            .get(key)
            .copied()
            .map(Bounds::from)
    }

    fn save(&self, key: &str, bounds: Bounds<Pixels>, cx: &mut App) {
        let frames_mut = self.frames_mut;
        let key = key.to_string();
        self.prefs.update(cx, |prefs, cx| {
            // A read-only store (malformed file) keeps the frame in memory;
            // the preferences entity already reports that write failure.
            let _ = prefs.set(
                &format!("window.frame.{key}"),
                |value| {
                    frames_mut(value).insert(key.clone(), SavedFrame::from(bounds));
                },
                cx,
            );
        });
    }
}

/// Bounds to open `role` with: the stored frame when present, otherwise
/// `default` centred on `parent` for roles that centre, otherwise `default`.
#[must_use]
pub fn restore_bounds(
    role: WindowRole,
    default: Bounds<Pixels>,
    parent: Option<Bounds<Pixels>>,
    store: Option<&dyn FrameStore>,
    cx: &App,
) -> Bounds<Pixels> {
    if let Some(stored) = store
        .zip(role.autosave_key())
        .and_then(|(store, key)| store.load(key, cx))
    {
        return stored;
    }
    match parent {
        Some(parent) if role.spec().centers_on_parent => centered_on(parent, default.size),
        _ => default,
    }
}

/// Persist the window's current frame under its role's autosave key.
pub fn save_frame(role: WindowRole, window: &Window, store: &dyn FrameStore, cx: &mut App) {
    if let Some(key) = role.autosave_key() {
        store.save(key, window.bounds(), cx);
    }
}

/// Windows owned by a parent. The parent's close policy calls
/// [`OwnedWindows::close_all`]; Silvestris does not install its own
/// `on_window_should_close` handler because GPUI keeps one per window and the
/// application owns the close decision (#37).
#[derive(Default)]
pub struct OwnedWindows {
    children: Vec<AnyWindowHandle>,
}

impl OwnedWindows {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.children.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.children.is_empty()
    }

    pub fn register(&mut self, child: AnyWindowHandle) {
        self.children.push(child);
    }

    /// Close every owned window. Dropped or already-closed windows are skipped.
    pub fn close_all(&mut self, cx: &mut App) {
        for child in self.children.drain(..) {
            let _ = child.update(cx, |_, window, _| window.remove_window());
        }
    }
}

/// Open a role window. The caller builds the root view, usually a
/// `WindowFrame` with `.role(role)`.
pub fn open_role<V: 'static + Render>(
    role: WindowRole,
    title: impl Into<gpui::SharedString>,
    bounds: Bounds<Pixels>,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<WindowHandle<V>> {
    crate::chrome::open_window(role_options(role, title, bounds, cx), cx, build)
}

/// A role window owned by another window, for `open_owned_role`.
#[derive(Clone)]
pub struct OwnedWindow {
    pub role: WindowRole,
    pub title: gpui::SharedString,
    /// The frame to use when there is nothing saved and the role does not
    /// centre on its parent.
    pub default_bounds: Bounds<Pixels>,
    pub parent: AnyWindowHandle,
}

impl OwnedWindow {
    #[must_use]
    pub fn new(
        role: WindowRole,
        title: impl Into<gpui::SharedString>,
        default_bounds: Bounds<Pixels>,
        parent: AnyWindowHandle,
    ) -> Self {
        Self {
            role,
            title: title.into(),
            default_bounds,
            parent,
        }
    }
}

/// Open a role window owned by a parent. The frame is restored from `store`
/// when the role has an autosave key, centred on the parent when the role
/// centres, and the handle is registered so the parent can close it.
pub fn open_owned_role<V: 'static + Render>(
    request: OwnedWindow,
    owned: &mut OwnedWindows,
    store: Option<&dyn FrameStore>,
    cx: &mut App,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> anyhow::Result<WindowHandle<V>> {
    let parent_bounds = request
        .parent
        .update(cx, |_, window, _| window.bounds())
        .ok();
    let bounds = restore_bounds(
        request.role,
        request.default_bounds,
        parent_bounds,
        store,
        cx,
    );
    let handle = open_role(request.role, request.title, bounds, cx, build)?;
    owned.register(*handle);
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use gpui::{div, prelude::*, Context, Render, TestAppContext, Window};

    use super::*;

    struct View;
    impl Render for View {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    #[derive(Default)]
    struct MapStore(RefCell<BTreeMap<String, Bounds<Pixels>>>);

    impl FrameStore for MapStore {
        fn load(&self, key: &str, _: &App) -> Option<Bounds<Pixels>> {
            self.0.borrow().get(key).copied()
        }

        fn save(&self, key: &str, bounds: Bounds<Pixels>, _: &mut App) {
            self.0.borrow_mut().insert(key.to_string(), bounds);
        }
    }

    fn test_app() -> TestAppContext {
        let app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app
    }

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds::new(point(px(x), px(y)), size(px(w), px(h)))
    }

    #[test]
    fn document_and_preferences_differ_in_the_expected_flags() {
        let document = WindowRole::Document.spec();
        assert!(document.resizable && document.minimize && document.zoom && document.close);
        assert_eq!(document.autosave, Some("document"));

        let preferences = WindowRole::Preferences.spec();
        assert!(!preferences.resizable);
        assert!(
            preferences.minimize,
            "10.6 System Preferences keeps minimize"
        );
        assert!(!preferences.zoom);
        assert!(preferences.close);
        assert_eq!(preferences.autosave, Some("preferences"));

        let about = WindowRole::About.spec();
        assert!(about.close && !about.minimize && !about.zoom && !about.resizable);
        assert!(!about.centers_on_parent);

        let dialog = WindowRole::Dialog.spec();
        assert!(dialog.centers_on_parent && !dialog.resizable);

        assert!(WindowRole::Panel.spec().hides_on_deactivate);
        assert!(WindowRole::Utility.spec().small_titlebar);
        assert_eq!(WindowRole::ALL.len(), 7);
    }

    #[test]
    fn role_options_carry_resizability_and_a_minimum_only_when_resizable() {
        let app = test_app();
        app.update(|cx| {
            let document =
                role_options(WindowRole::Document, "Doc", bounds(0., 0., 800., 600.), cx);
            assert!(document.is_resizable);
            assert!(document.window_min_size.is_some());

            let preferences = role_options(
                WindowRole::Preferences,
                "Preferences",
                bounds(0., 0., 640., 460.),
                cx,
            );
            assert!(!preferences.is_resizable);
            assert!(preferences.window_min_size.is_none());

            let dialog = role_options(WindowRole::Dialog, "Dialog", bounds(0., 0., 420., 180.), cx);
            assert!(!dialog.is_resizable);
        });
    }

    #[test]
    fn a_centered_role_uses_the_parent_and_a_stored_frame_wins() {
        let app = test_app();
        let parent = bounds(100., 50., 1000., 700.);
        let default = bounds(0., 0., 400., 300.);
        app.update(|cx| {
            let centered = restore_bounds(WindowRole::Dialog, default, Some(parent), None, cx);
            assert_eq!(centered.origin, point(px(400.), px(250.)));

            let document = restore_bounds(WindowRole::Document, default, Some(parent), None, cx);
            assert_eq!(document, default);

            let store = MapStore::default();
            let saved = bounds(12., 34., 500., 400.);
            store.save("preferences", saved, cx);
            let restored = restore_bounds(
                WindowRole::Preferences,
                default,
                Some(parent),
                Some(&store),
                cx,
            );
            assert_eq!(restored, saved);
        });
    }

    #[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
    struct AppPrefs {
        schema_version: u32,
        #[serde(default)]
        frames: SavedFrames,
    }

    impl silvestris_core::prefs::Versioned for AppPrefs {
        fn schema_version(&self) -> u32 {
            self.schema_version
        }
        fn set_schema_version(&mut self, version: u32) {
            self.schema_version = version;
        }
    }

    /// #20: the frame hook writes through the app's #62 preference store, and a
    /// reopened store restores the frame.
    #[test]
    fn frames_write_through_the_application_preferences() {
        let dir = std::env::temp_dir().join(format!(
            "silvestris-frames-{}-{}",
            std::process::id(),
            line!()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("app.json");
        let _ = std::fs::remove_file(&path);
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let (prefs, _) =
            app.update(|cx| crate::prefs::Preferences::<AppPrefs>::open(&path, &[], cx));
        let store = PreferenceFrames::new(prefs.clone(), |p| &p.frames, |p| &mut p.frames);
        app.update(|cx| {
            window
                .update(cx, |_, window, cx| {
                    save_frame(WindowRole::Preferences, window, &store, cx)
                })
                .unwrap();
        });
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("\"preferences\""), "{written}");

        let (reopened, _) =
            app.update(|cx| crate::prefs::Preferences::<AppPrefs>::open(&path, &[], cx));
        let store = PreferenceFrames::new(reopened, |p| &p.frames, |p| &mut p.frames);
        let expected = app.update(|cx| window.update(cx, |_, w, _| w.bounds()).unwrap());
        let restored = app.update(|cx| {
            restore_bounds(
                WindowRole::Preferences,
                bounds(0., 0., 10., 10.),
                None,
                Some(&store),
                cx,
            )
        });
        assert_eq!(restored, expected);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cascading_offsets_the_next_window_by_the_hig_step() {
        let previous = bounds(100., 80., 800., 600.);
        let next = cascade_from(previous, 20.);
        assert_eq!(next.origin, point(px(120.), px(100.)));
        assert_eq!(next.size, previous.size);
    }

    #[test]
    fn saving_a_frame_uses_the_role_key_and_skips_roles_without_one() {
        let mut app = test_app();
        let window = app.add_window(|_, _| View);
        let store = MapStore::default();
        app.update(|cx| {
            window
                .update(cx, |_, window, cx| {
                    save_frame(WindowRole::Document, window, &store, cx);
                    save_frame(WindowRole::Dialog, window, &store, cx);
                })
                .unwrap();
        });
        app.update(|cx| {
            assert!(store.load("document", cx).is_some());
            assert!(store.load("dialog", cx).is_none());
        });
    }

    #[test]
    fn owned_windows_open_centered_and_close_with_the_parent() {
        let mut app = test_app();
        let parent = app.add_window(|_, _| View);
        app.update(|cx| {
            parent
                .update(cx, |_, window, _| window.set_window_title("Parent"))
                .unwrap();
        });
        let mut owned = OwnedWindows::new();
        let child = app.update(|cx| {
            open_owned_role(
                OwnedWindow::new(
                    WindowRole::Dialog,
                    "Sheet target",
                    bounds(0., 0., 300., 160.),
                    *parent,
                ),
                &mut owned,
                None,
                cx,
                |_, cx| cx.new(|_| View),
            )
            .expect("open child")
        });
        assert_eq!(owned.len(), 1);
        let parent_bounds =
            app.update(|cx| parent.update(cx, |_, window, _| window.bounds()).unwrap());
        let child_bounds =
            app.update(|cx| child.update(cx, |_, window, _| window.bounds()).unwrap());
        assert_eq!(
            child_bounds.origin,
            centered_on(parent_bounds, child_bounds.size).origin
        );
        app.update(|cx| owned.close_all(cx));
        assert!(owned.is_empty());
        assert!(
            child.update(&mut app, |_, _, _| ()).is_err(),
            "closing the parent's owned windows closes the child"
        );
    }
}
