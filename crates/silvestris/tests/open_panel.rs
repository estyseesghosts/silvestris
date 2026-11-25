//! OpenPanel (#29): one or many files, folders, double-click navigation, the
//! allowed-type gate and the list/column view switch.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Context, Entity, Modifiers, MouseButton, TestAppContext, VisualContext,
    VisualTestContext, Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::file_panel::{FileEntry, FilePanel, MemoryProvider, OpenPanel, PanelOutcome};
use silvestris::icons::Icon;
use silvestris::{ThemeId, ThemePreference};

struct Body;

impl gpui::Render for Body {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
    }
}

struct Owner {
    body: Entity<Body>,
}

impl gpui::Render for Owner {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new("Document").child(self.body.clone())
    }
}

type Outcome = Rc<RefCell<Option<PanelOutcome>>>;

fn provider() -> MemoryProvider {
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        vec![
            FileEntry::folder("docs", "Documents"),
            FileEntry::document("note", "note.txt"),
            FileEntry::document("photo", "photo.png"),
        ],
    );
    provider.add_location(
        "/docs",
        "Documents",
        Icon::Documents,
        Some("/".into()),
        vec![FileEntry::document("inner", "inner.txt")],
    );
    provider.set_places(vec![
        silvestris::file_panel::FilePlace::new(
            "/",
            "Computer",
            Icon::Volume,
            silvestris::file_panel::PlaceSection::Devices,
        ),
        silvestris::file_panel::FilePlace::new(
            "/docs",
            "Documents",
            Icon::Documents,
            silvestris::file_panel::PlaceSection::Places,
        ),
        silvestris::file_panel::FilePlace::new(
            "search:today",
            "Today",
            Icon::Search,
            silvestris::file_panel::PlaceSection::SearchFor,
        ),
        silvestris::file_panel::FilePlace::new(
            "search:week",
            "Past Week",
            Icon::Search,
            silvestris::file_panel::PlaceSection::SearchFor,
        ),
    ]);
    provider
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn present(
    app: &mut TestAppContext,
    multiple: bool,
    choose_dirs: bool,
    types: Option<&[&str]>,
) -> (Entity<FilePanel>, Outcome, VisualTestContext) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let mut cx = cx.clone();
    let owner = cx.window_handle();
    let outcome: Outcome = Rc::default();
    let slot = outcome.clone();
    let panel = cx.update(|window, cx| {
        let mut builder = OpenPanel::new(Rc::new(provider()))
            .directory("/")
            .allows_multiple(multiple)
            .can_choose_files(true)
            .can_choose_directories(choose_dirs)
            .on_finish(move |outcome, _| *slot.borrow_mut() = Some(outcome.clone()));
        if let Some(types) = types {
            builder = builder.allowed_types(types.iter().copied());
        }
        builder.begin_sheet(owner, window, cx)
    });
    draw(&mut cx);
    (panel, outcome, cx.clone())
}

fn click_row(cx: &mut VisualTestContext, id: &str) {
    let row = cx
        .debug_bounds(Box::leak(format!("table-row-{id}").into_boxed_str()))
        .unwrap_or_else(|| panic!("row {id}"));
    cx.simulate_click(row.center(), Modifiers::default());
    cx.run_until_parked();
}

#[test]
fn open_is_disabled_until_a_choosable_row_is_selected() {
    let mut app = TestAppContext::single();
    let (_panel, _outcome, mut cx) = present(&mut app, false, false, None);
    let open = cx.debug_bounds("panel-accept").expect("open button");
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.debug_bounds("silvestris-alert").is_none());
    click_row(&mut cx, "note");
    assert!(cx.debug_bounds("panel-accept").is_some());
}

#[test]
fn opening_a_file_returns_its_location() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, false, false, None);
    click_row(&mut cx, "note");
    let open = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    assert!(outcome.accepted());
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/note")
    );
}

#[test]
fn double_click_enters_a_folder_and_then_opens_its_file() {
    let mut app = TestAppContext::single();
    let (panel, outcome, mut cx) = present(&mut app, false, false, None);
    click_row(&mut cx, "docs");
    cx.update(|_window, cx| {
        panel.update(cx, |panel, cx| panel.activate_selection(cx));
    });
    draw(&mut cx);
    assert_eq!(
        cx.read(|cx| panel.read(cx).model().read(cx).location().to_string()),
        "/docs"
    );
    click_row(&mut cx, "inner");
    cx.update(|_window, cx| {
        panel.update(cx, |panel, cx| panel.activate_selection(cx));
    });
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    assert!(outcome.accepted());
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/docs/inner")
    );
}

#[test]
fn choosing_a_folder_returns_the_folder_when_directories_are_allowed() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, false, true, None);
    click_row(&mut cx, "docs");
    let open = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/docs")
    );
}

#[test]
fn multiple_selection_reports_every_chosen_location() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, true, false, None);
    // Multiple selection is the list view's contract; column view selects the
    // active column's row only.
    let list = cx.debug_bounds("segment-list").expect("list segment");
    cx.simulate_click(list.center(), Modifiers::default());
    draw(&mut cx);
    click_row(&mut cx, "note");
    let secondary = cx.update(|_, cx| Modifiers::secondary_key(cx));
    let photo = cx.debug_bounds("table-row-photo").expect("photo");
    cx.simulate_click(photo.center(), secondary);
    cx.run_until_parked();
    let open = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    let mut locations: Vec<String> = outcome.locations.iter().map(ToString::to_string).collect();
    locations.sort();
    assert_eq!(locations, ["/note", "/photo"]);
}

#[test]
fn a_file_outside_the_allowed_types_cannot_be_opened() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, false, false, Some(&["txt"]));
    click_row(&mut cx, "photo");
    let open = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    assert!(outcome.borrow().is_none(), "png is not choosable");
    assert!(cx.debug_bounds("file-panel").is_some());
}

#[test]
fn switching_to_columns_keeps_the_location_and_selection() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, mut cx) = present(&mut app, false, false, None);
    // Start in list view: column view opens a branch column when its row is
    // selected, which is a navigation rather than a mode switch.
    let list = cx.debug_bounds("segment-list").expect("list segment");
    cx.simulate_click(list.center(), Modifiers::default());
    draw(&mut cx);
    click_row(&mut cx, "docs");
    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(&mut cx);
    assert_eq!(
        cx.read(|cx| panel.read(cx).model().read(cx).view()),
        silvestris::file_panel::FileViewMode::Columns
    );
    assert_eq!(
        cx.read(|cx| panel.read(cx).model().read(cx).location().to_string()),
        "/"
    );
    assert_eq!(
        cx.read(|cx| panel.read(cx).selected_count(cx)),
        1,
        "the selection survives the mode switch"
    );
    assert!(cx.debug_bounds("column-browser").is_some());
}

#[test]
fn columns_mode_keeps_the_location_when_a_file_is_selected() {
    let mut app = TestAppContext::single();
    let (panel, outcome, mut cx) = present(&mut app, false, false, None);
    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(&mut cx);
    click_row(&mut cx, "note");
    assert_eq!(
        cx.read(|cx| panel.read(cx).model().read(cx).location().to_string()),
        "/",
        "a file row is not a directory"
    );
    let open = cx.debug_bounds("panel-accept").expect("open button");
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/note")
    );
}

#[test]
fn columns_mode_refuses_a_file_outside_the_allowed_types() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, false, false, Some(&["txt"]));
    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(&mut cx);
    click_row(&mut cx, "photo");
    let open = cx.debug_bounds("panel-accept").expect("open button");
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    assert!(outcome.borrow().is_none(), "png is not choosable");
    assert!(cx.debug_bounds("file-panel").is_some());
}

#[test]
fn columns_mode_can_choose_a_folder_when_directories_are_allowed() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, false, true, None);
    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(&mut cx);
    click_row(&mut cx, "docs");
    let open = cx.debug_bounds("panel-accept").expect("open button");
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/docs")
    );
}

#[test]
fn a_thirty_thousand_entry_directory_builds_only_the_rows_in_view() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        (0..30_000)
            .map(|i| FileEntry::document(i.to_string(), format!("File {i}.txt")))
            .collect(),
    );
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider))
            .directory("/")
            .begin_sheet(owner, window, cx)
    });
    draw(cx);
    assert!(cx.debug_bounds("table-row-0").is_some());
    assert!(
        cx.debug_bounds("table-row-500").is_none(),
        "only the viewport's rows are built"
    );
    assert!(
        cx.read(|cx| panel
            .read(cx)
            .model()
            .read(cx)
            .controller()
            .read(cx)
            .visible_len())
            == 30_000
    );
}

#[test]
fn one_sort_model_orders_list_icons_and_columns_the_same_way() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        vec![
            FileEntry::document("b", "beta.txt"),
            FileEntry::folder("d", "Delta"),
            FileEntry::document("a", "Alpha.txt"),
            FileEntry::document("c", "charlie.txt"),
        ],
    );
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider))
            .directory("/")
            .begin_sheet(owner, window, cx)
    });
    draw(cx);
    let names = |cx: &mut VisualTestContext| -> Vec<String> {
        cx.read(|cx| {
            panel
                .read(cx)
                .model()
                .read(cx)
                .controller()
                .read(cx)
                .visible()
                .map(|entry| entry.name.to_string())
                .collect()
        })
    };

    let list = cx.debug_bounds("segment-list").expect("list segment");
    cx.simulate_click(list.center(), Modifiers::default());
    draw(cx);
    let header = cx.debug_bounds("table-header-name").expect("name header");
    cx.simulate_click(header.center(), Modifiers::default());
    draw(cx);
    assert_eq!(names(cx), ["Delta", "Alpha.txt", "beta.txt", "charlie.txt"]);
    assert_eq!(
        cx.read(|cx| panel.read(cx).table().read(cx).sort()),
        Some((
            "name".into(),
            silvestris::controls::SortDirection::Ascending
        ))
    );

    let icons = cx.debug_bounds("segment-icons").expect("icon segment");
    cx.simulate_click(icons.center(), Modifiers::default());
    draw(cx);
    assert_eq!(
        names(cx),
        ["Delta", "Alpha.txt", "beta.txt", "charlie.txt"],
        "the icon view reads the same controller"
    );

    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(cx);
    let labels: Vec<String> = cx.read(|cx| {
        panel
            .read(cx)
            .browser()
            .expect("column browser")
            .read(cx)
            .children_of("")
            .expect("root children")
            .iter()
            .map(|node| node.label.to_string())
            .collect()
    });
    assert_eq!(labels, ["Delta", "Alpha.txt", "beta.txt", "charlie.txt"]);

    // The same header toggles the shared direction; folders stay first and
    // every view follows.
    let list = cx.debug_bounds("segment-list").expect("list segment");
    cx.simulate_click(list.center(), Modifiers::default());
    draw(cx);
    let header = cx.debug_bounds("table-header-name").expect("name header");
    cx.simulate_click(header.center(), Modifiers::default());
    draw(cx);
    let descending = ["Delta", "charlie.txt", "beta.txt", "Alpha.txt"];
    assert_eq!(names(cx), descending);
    assert_eq!(
        cx.read(|cx| panel
            .read(cx)
            .model()
            .read(cx)
            .sort()
            .map(|sort| sort.direction)),
        Some(silvestris::controls::SortDirection::Descending)
    );

    let icons = cx.debug_bounds("segment-icons").expect("icon segment");
    cx.simulate_click(icons.center(), Modifiers::default());
    draw(cx);
    assert_eq!(names(cx), descending, "icons follow the toggle");

    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(cx);
    let labels: Vec<String> = cx.read(|cx| {
        panel
            .read(cx)
            .browser()
            .expect("column browser")
            .read(cx)
            .children_of("")
            .expect("root children")
            .iter()
            .map(|node| node.label.to_string())
            .collect()
    });
    assert_eq!(labels, descending, "columns follow the toggle");
}

#[test]
fn a_thirty_thousand_entry_directory_builds_only_the_icon_cells_in_view() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Computer",
        Icon::Volume,
        None,
        (0..30_000)
            .map(|i| FileEntry::document(i.to_string(), format!("File {i}.txt")))
            .collect(),
    );
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider))
            .directory("/")
            .begin_sheet(owner, window, cx)
    });
    draw(cx);
    let icons = cx.debug_bounds("segment-icons").expect("icon segment");
    cx.simulate_click(icons.center(), Modifiers::default());
    draw(cx);
    assert!(cx.debug_bounds("icon-grid").is_some());
    assert!(cx.debug_bounds("icon-cell-0").is_some());
    assert!(
        cx.debug_bounds("icon-cell-500").is_none(),
        "the icon grid materialises only the cells in view"
    );
    assert!(
        cx.read(|cx| panel
            .read(cx)
            .model()
            .read(cx)
            .controller()
            .read(cx)
            .visible_len())
            == 30_000,
        "the controller still owns the whole directory"
    );
}

#[test]
fn presenting_without_an_owner_uses_an_application_modal_window() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider()))
            .directory("/")
            .run_modal(window, cx)
    });
    draw(cx);
    assert_eq!(
        cx.read(|cx| panel.read(cx).session().read(cx).scope()),
        silvestris::modal::ModalScope::Application
    );
}

#[test]
fn open_starts_in_column_view() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, mut cx) = present(&mut app, false, false, None);
    assert_eq!(
        cx.read(|cx| panel.read(cx).model().read(cx).view()),
        silvestris::file_panel::FileViewMode::Columns,
        "the 4.15.47 AM capture opens in column view"
    );
    assert!(cx.debug_bounds("column-browser").is_some());
}

#[test]
fn icon_view_renders_and_activates_a_file() {
    let mut app = TestAppContext::single();
    let (_panel, outcome, mut cx) = present(&mut app, false, false, None);
    let icons = cx.debug_bounds("segment-icons").expect("icon segment");
    cx.simulate_click(icons.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.debug_bounds("icon-grid").is_some());
    let cell = cx.debug_bounds("icon-cell-note").expect("note icon cell");
    cx.simulate_click(cell.center(), Modifiers::default());
    draw(&mut cx);
    let open = cx.debug_bounds("panel-accept").expect("open button");
    cx.simulate_click(open.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("outcome");
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/note")
    );
}

#[test]
fn icon_cells_and_the_column_browser_open_a_context_menu() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, mut cx) = present(&mut app, false, false, None);
    let row = cx.debug_bounds("table-row-note").expect("note row");
    cx.simulate_mouse_down(row.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(row.center(), MouseButton::Right, Modifiers::default());
    draw(&mut cx);
    assert!(
        cx.debug_bounds("menu-panel-0").is_some(),
        "column row opens a file menu"
    );
    cx.simulate_keystrokes("escape");
    draw(&mut cx);
    let icons = cx.debug_bounds("segment-icons").expect("icon segment");
    cx.simulate_click(icons.center(), Modifiers::default());
    draw(&mut cx);
    let cell = cx.debug_bounds("icon-cell-note").expect("note cell");
    cx.simulate_mouse_down(cell.center(), MouseButton::Right, Modifiers::default());
    cx.simulate_mouse_up(cell.center(), MouseButton::Right, Modifiers::default());
    draw(&mut cx);
    assert!(
        cx.debug_bounds("menu-panel-0").is_some(),
        "icon cell opens a file menu"
    );
    let rename = cx.debug_bounds("menu-item-0-2").expect("Rename row");
    cx.simulate_click(rename.center(), Modifiers::default());
    draw(&mut cx);
    assert!(
        cx.read(|cx| panel.read(cx).is_editing(cx)),
        "icon view renames in place"
    );
}

#[test]
fn the_sidebar_section_triangles_collapse_their_groups() {
    let mut app = TestAppContext::single();
    let (_panel, _outcome, mut cx) = present(&mut app, false, false, None);
    assert!(cx.debug_bounds("source-section-devices").is_some());
    assert!(cx.debug_bounds("source-row-/").is_some());
    let disclosure = cx
        .debug_bounds("disclose-devices")
        .expect("devices triangle");
    cx.simulate_click(disclosure.center(), Modifiers::default());
    draw(&mut cx);
    assert!(
        cx.debug_bounds("source-row-/").is_none(),
        "the device row collapses with its section"
    );
    let disclosure = cx
        .debug_bounds("disclose-devices")
        .expect("devices triangle");
    cx.simulate_click(disclosure.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.debug_bounds("source-row-/").is_some());
}

#[test]
fn the_open_dialog_is_the_measured_size_resizable_and_not_clipped() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let _panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider()))
            .directory("/")
            .run_modal(window, cx)
    });
    let dialog = cx
        .cx
        .windows()
        .into_iter()
        .find(|window| *window != owner)
        .expect("Open dialog window");
    let mut dialog_cx = VisualTestContext::from_window(dialog, &cx.cx);
    draw(&mut dialog_cx);
    let bounds = dialog_cx.update(|window, _| window.bounds());
    assert_eq!(
        bounds.size,
        gpui::size(px(527.), px(398.)),
        "outer size from M: panel.outer"
    );
    assert!(
        silvestris::window_role::WindowRole::OpenPanel
            .spec()
            .resizable
    );
    let panel = dialog_cx.debug_bounds("file-panel").expect("panel");
    let buttons = dialog_cx.debug_bounds("panel-buttons").expect("buttons");
    let viewport = dialog_cx.update(|window, _| window.viewport_size());
    assert!(
        buttons.bottom() <= viewport.height,
        "the button row is inside the content area: {} <= {}",
        buttons.bottom(),
        viewport.height
    );
    assert_eq!(
        buttons.bottom(),
        panel.bottom(),
        "the button row closes the panel"
    );
}

#[test]
fn the_open_dialog_frame_is_restored_from_the_store() {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use silvestris::window_role::FrameStore;

    #[derive(Default)]
    struct Frames(RefCell<BTreeMap<String, gpui::Bounds<gpui::Pixels>>>);
    impl FrameStore for Frames {
        fn load(&self, key: &str, _: &gpui::App) -> Option<gpui::Bounds<gpui::Pixels>> {
            self.0.borrow().get(key).copied()
        }
        fn save(&self, key: &str, bounds: gpui::Bounds<gpui::Pixels>, _: &mut gpui::App) {
            self.0.borrow_mut().insert(key.to_string(), bounds);
        }
    }

    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    app.update(|cx| cx.set_reduce_motion(true));
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let owner = cx.window_handle();
    let frames = Rc::new(Frames::default());
    let _panel = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider()))
            .directory("/")
            .frame_store(frames.clone())
            .run_modal(window, cx)
    });
    let dialog = cx
        .cx
        .windows()
        .into_iter()
        .find(|window| *window != owner)
        .expect("Open dialog window");
    let mut dialog_cx = VisualTestContext::from_window(dialog, &cx.cx);
    draw(&mut dialog_cx);
    dialog_cx.update(|window, _| window.resize(gpui::size(px(600.), px(500.))));
    draw(&mut dialog_cx);
    dialog_cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let reopened = cx.update(|window, cx| {
        OpenPanel::new(Rc::new(provider()))
            .directory("/")
            .frame_store(frames.clone())
            .run_modal(window, cx)
    });
    let _ = reopened;
    let dialog = cx
        .cx
        .windows()
        .into_iter()
        .find(|window| *window != owner)
        .expect("reopened dialog");
    let mut dialog_cx = VisualTestContext::from_window(dialog, &cx.cx);
    draw(&mut dialog_cx);
    assert_eq!(
        dialog_cx.update(|window, _| window.bounds().size),
        gpui::size(px(600.), px(500.)),
        "the remembered Open window frame is restored"
    );
}

#[test]
fn the_sidebar_uses_devices_places_and_search_groups() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, mut cx) = present(&mut app, false, false, None);
    let headings: Vec<String> = cx.read(|cx| {
        panel
            .read(cx)
            .sidebar_entity()
            .read(cx)
            .state()
            .rows()
            .iter()
            .filter(|row| row.section)
            .map(|row| row.label.to_uppercase())
            .collect()
    });
    assert_eq!(headings, ["DEVICES", "PLACES", "SEARCH FOR"]);
    let sidebar = cx.debug_bounds("panel-sidebar").expect("sidebar");
    assert!(f32::from(sidebar.size.width) >= 150.0);
}

#[test]
fn selecting_a_file_in_columns_shows_the_preview_column() {
    let mut app = TestAppContext::single();
    let (_panel, _outcome, mut cx) = present(&mut app, false, false, None);
    let row = cx.debug_bounds("table-row-note").expect("note row");
    cx.simulate_click(row.center(), Modifiers::default());
    draw(&mut cx);
    assert!(
        cx.debug_bounds("browser-preview").is_some(),
        "the column browser reserves a preview column for a leaf"
    );
    assert!(
        cx.debug_bounds("panel-preview").is_some(),
        "the Open panel fills it with the selected file's metadata"
    );
}
