//! Table view (#32): selection semantics, keyboard, type-select, sorting by
//! id, column resize/reorder/autosave, in-place editing, context menus and
//! virtualisation.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, App, AppContext, Context, Entity, Modifiers, MouseButton,
    SharedString, TestAppContext, VisualTestContext, Window,
};
use silvestris::binding::{ItemId, ListController};
use silvestris::controls::{
    ColumnStore, MemoryColumns, SortDirection, TableColumn, TableEvent, TableView,
};
use silvestris::{ThemeId, ThemePreference};

#[derive(Clone, Debug)]
struct File {
    id: u32,
    name: SharedString,
    size: u32,
}

fn files() -> Vec<File> {
    ["Delta", "Alpha", "Charlie", "Bravo", "Echo"]
        .iter()
        .enumerate()
        .map(|(i, name)| File {
            id: i as u32,
            name: (*name).into(),
            size: (10 - i) as u32,
        })
        .collect()
}

type Events = Rc<RefCell<Vec<TableEvent>>>;

struct Fixture<'a> {
    table: Entity<TableView<File>>,
    controller: Entity<ListController<File>>,
    events: Events,
    cx: &'a mut VisualTestContext,
}

fn columns() -> Vec<TableColumn<File>> {
    vec![
        TableColumn::text("name", "Name", |f: &File| f.name.clone())
            .width(120.)
            .sort_by_text(),
        TableColumn::text("size", "Size", |f: &File| f.size.to_string().into())
            .width(60.)
            .sortable(|a: &File, b: &File| a.size.cmp(&b.size)),
    ]
}

fn open(app: &mut TestAppContext, items: Vec<File>) -> Fixture<'_> {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let controller = app.update(|cx| {
        cx.new(|_| {
            let mut c = ListController::new(|f: &File| f.id.to_string().into());
            c.set_items(items);
            c
        })
    });
    let events: Events = Rc::default();
    let sink = events.clone();
    let model = controller.clone();
    let (table, cx) = app.add_window_view(move |_, cx| {
        let table = TableView::new(model, columns(), cx);
        cx.subscribe_self(move |_, e: &TableEvent, _| sink.borrow_mut().push(e.clone()))
            .detach();
        table
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    Fixture {
        table,
        controller,
        events,
        cx,
    }
}

impl Fixture<'_> {
    fn row(&mut self, id: &str) -> gpui::Bounds<gpui::Pixels> {
        self.cx
            .debug_bounds(Box::leak(format!("table-row-{id}").into_boxed_str()))
            .unwrap_or_else(|| panic!("row {id} rendered"))
    }
    fn click(&mut self, id: &str, modifiers: Modifiers) {
        let at = self.row(id).center();
        self.cx
            .simulate_mouse_down(at, MouseButton::Left, modifiers);
        self.cx.simulate_mouse_up(at, MouseButton::Left, modifiers);
        self.cx.run_until_parked();
    }
    fn selected(&mut self) -> Vec<String> {
        let controller = self.controller.clone();
        self.cx.read(|cx| {
            controller
                .read(cx)
                .selected_ids()
                .iter()
                .map(|s| s.to_string())
                .collect()
        })
    }
    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }
    fn names(&mut self) -> Vec<String> {
        let controller = self.controller.clone();
        self.cx.read(|cx| {
            controller
                .read(cx)
                .visible()
                .map(|f| f.name.to_string())
                .collect()
        })
    }
}

fn secondary(cx: &mut VisualTestContext) -> Modifiers {
    cx.update(|_, cx: &mut App| Modifiers::secondary_key(cx))
}

#[test]
fn click_shift_and_secondary_click_follow_the_anchor() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    f.click("0", Modifiers::default());
    assert_eq!(f.selected(), ["0"]);
    // Visible order is the item order: Delta(0) Alpha(1) Charlie(2) ...
    f.click("2", Modifiers::shift());
    assert_eq!(
        f.selected(),
        ["0", "1", "2"],
        "Shift extends from the anchor"
    );
    let toggle = secondary(f.cx);
    f.click("1", toggle);
    assert_eq!(f.selected(), ["0", "2"], "Command/Control toggles one row");
    f.click("4", toggle);
    assert_eq!(f.selected(), ["0", "2", "4"]);
    assert_eq!(
        f.events.borrow().len(),
        4,
        "one SelectionChanged per user change"
    );
}

#[test]
fn programmatic_selection_sends_no_event() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    let controller = f.controller.clone();
    f.cx.update(|_, cx| {
        controller.update(cx, |c, cx| {
            c.select("3");
            cx.notify();
        })
    });
    f.cx.run_until_parked();
    assert_eq!(f.selected(), ["3"]);
    assert!(f.events.borrow().is_empty());
}

#[test]
fn arrows_home_end_and_shift_arrows_move_the_lead() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    f.click("1", Modifiers::default());
    f.keys("down");
    assert_eq!(f.selected(), ["2"]);
    f.keys("shift-down shift-down");
    assert_eq!(f.selected(), ["2", "3", "4"]);
    f.keys("home");
    assert_eq!(f.selected(), ["0"]);
    f.keys("end");
    assert_eq!(f.selected(), ["4"]);
    f.keys("up");
    assert_eq!(f.selected(), ["3"]);
}

#[test]
fn header_click_sorts_and_keeps_the_selection_by_id() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    f.click("2", Modifiers::default()); // Charlie
    let header = f.cx.debug_bounds("table-header-name").expect("name header");
    f.cx.simulate_click(header.center(), Modifiers::default());
    f.cx.run_until_parked();
    assert_eq!(f.names(), ["Alpha", "Bravo", "Charlie", "Delta", "Echo"]);
    assert_eq!(f.selected(), ["2"], "still Charlie after sorting");
    f.cx.simulate_click(header.center(), Modifiers::default());
    f.cx.run_until_parked();
    assert_eq!(f.names(), ["Echo", "Delta", "Charlie", "Bravo", "Alpha"]);
    let sort = f.cx.read(|cx| f.table.read(cx).sort());
    assert_eq!(sort, Some(("name".into(), SortDirection::Descending)));
    assert!(f.events.borrow().iter().any(|e| matches!(
        e,
        TableEvent::Sorted {
            direction: SortDirection::Descending,
            ..
        }
    )));
}

#[test]
fn type_select_matches_a_prefix_and_resets_after_a_pause() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    f.click("0", Modifiers::default());
    f.keys("c h");
    assert_eq!(f.selected(), ["2"], "\"ch\" finds Charlie");
    f.cx.executor()
        .advance_clock(silvestris::controls::table_view::TYPE_SELECT_RESET);
    f.cx.run_until_parked();
    f.keys("b");
    assert_eq!(
        f.selected(),
        ["3"],
        "a fresh prefix after the pause finds Bravo"
    );
}

#[test]
fn dragging_a_divider_resizes_and_autosaves_the_column() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    let store: Rc<MemoryColumns> = Rc::default();
    let table = f.table.clone();
    let dyn_store: Rc<dyn ColumnStore> = store.clone();
    f.cx.update(|_, cx| table.update(cx, |t, cx| t.set_autosave(dyn_store, "files", cx)));
    let header = f.cx.debug_bounds("table-header-name").expect("name header");
    let divider = point(header.right() - px(1.), header.center().y);
    f.cx.simulate_mouse_down(divider, MouseButton::Left, Modifiers::default());
    f.cx.simulate_mouse_move(
        point(divider.x + px(30.), divider.y),
        Some(MouseButton::Left),
        Modifiers::default(),
    );
    f.cx.simulate_mouse_up(
        point(divider.x + px(30.), divider.y),
        MouseButton::Left,
        Modifiers::default(),
    );
    f.cx.run_until_parked();
    let width = f.cx.read(|cx| f.table.read(cx).column_width("name"));
    assert_eq!(width, Some(150.));
    let saved =
        f.cx.read(|cx| store.load("files", cx))
            .expect("saved layout");
    assert_eq!(saved.widths.get("name"), Some(&150.));
    assert!(
        f.names().first().is_some_and(|n| n == "Delta"),
        "a resize does not sort"
    );
}

#[test]
fn dragging_a_header_reorders_and_a_saved_layout_restores() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    let name = f.cx.debug_bounds("table-header-name").expect("name header");
    let size = f.cx.debug_bounds("table-header-size").expect("size header");
    let from = name.center();
    f.cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::default());
    f.cx.simulate_mouse_move(size.center(), Some(MouseButton::Left), Modifiers::default());
    f.cx.simulate_mouse_up(size.center(), MouseButton::Left, Modifiers::default());
    f.cx.run_until_parked();
    let order = f.cx.read(|cx| f.table.read(cx).column_order());
    assert_eq!(order, [SharedString::from("size"), "name".into()]);
    assert_eq!(f.names()[0], "Delta", "a reorder is not a sort click");

    let saved = f.cx.read(|cx| f.table.read(cx).saved_columns());
    let store: Rc<MemoryColumns> = Rc::default();
    f.cx.update(|_, cx| store.save("files", saved, cx));
    let mut second = TestAppContext::single();
    let g = open(&mut second, files());
    let table = g.table.clone();
    let dyn_store: Rc<dyn ColumnStore> = store.clone();
    g.cx.update(|_, cx| table.update(cx, |t, cx| t.set_autosave(dyn_store, "files", cx)));
    let restored = g.cx.read(|cx| g.table.read(cx).column_order());
    assert_eq!(restored, [SharedString::from("size"), "name".into()]);
}

#[test]
fn return_edits_in_place_and_escape_cancels() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::controls::text_field::bind_keys);
    let renamed: Rc<RefCell<Vec<(u32, String)>>> = Rc::default();
    let log = renamed.clone();
    let controller = app.update(|cx| {
        cx.new(|_| {
            let mut c = ListController::new(|f: &File| f.id.to_string().into());
            c.set_items(files());
            c
        })
    });
    let model = controller.clone();
    let (table, cx) = app.add_window_view(move |_, cx| {
        let columns = vec![TableColumn::text("name", "Name", |f: &File| f.name.clone())
            .width(120.)
            .editable(move |f: &File, text: &str, _| {
                log.borrow_mut().push((f.id, text.to_string()));
                true
            })];
        TableView::new(model, columns, cx)
    });
    cx.run_until_parked();
    let row = cx.debug_bounds("table-row-1").expect("row");
    cx.simulate_click(row.center(), Modifiers::default());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.read(|cx| table.read(cx).is_editing()));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!cx.read(|cx| table.read(cx).is_editing()));
    assert!(renamed.borrow().is_empty(), "Escape commits nothing");

    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Zulu");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!cx.read(|cx| table.read(cx).is_editing()));
    assert_eq!(renamed.borrow().as_slice(), [(1, "Zulu".to_string())]);
}

#[test]
fn secondary_click_selects_the_row_and_reports_a_context_menu() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, files());
    f.click("0", Modifiers::default());
    let at = f.row("3").center();
    f.cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::default());
    f.cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::default());
    f.cx.run_until_parked();
    assert_eq!(f.selected(), ["3"]);
    let last = f.events.borrow().last().cloned();
    assert_eq!(
        last,
        Some(TableEvent::ContextMenu {
            ids: vec![ItemId::from("3")],
            position: at
        })
    );
}

#[test]
fn a_hundred_thousand_rows_render_only_the_viewport_and_survive_live_updates() {
    let mut app = TestAppContext::single();
    let many: Vec<File> = (0..100_000)
        .map(|i| File {
            id: i,
            name: format!("File {i}").into(),
            size: i,
        })
        .collect();
    let mut f = open(&mut app, many.clone());
    assert!(f.cx.debug_bounds("table-row-0").is_some());
    assert!(
        f.cx.debug_bounds("table-row-500").is_none(),
        "rows past the viewport are not built"
    );
    f.click("3", Modifiers::default());
    f.keys("end");
    assert_eq!(f.selected(), ["99999"]);
    assert!(
        f.cx.debug_bounds("table-row-99999").is_some(),
        "End reveals the last row"
    );
    let position = f.cx.read(|cx| {
        use silvestris::controls::ScrollSource;
        f.table.read(cx).scroll_handle().position()
    });
    // A live reload with changed values keeps the selection and the scroll.
    let controller = f.controller.clone();
    f.cx.update(|_, cx| {
        controller.update(cx, |c, cx| {
            let mut next = many;
            next[99_999].size = 7;
            c.set_items(next);
            cx.notify();
        })
    });
    f.cx.run_until_parked();
    assert_eq!(f.selected(), ["99999"]);
    let after = f.cx.read(|cx| {
        use silvestris::controls::ScrollSource;
        f.table.read(cx).scroll_handle().position()
    });
    assert_eq!(after, position);
}

fn options_fixture(
    app: &mut TestAppContext,
    configure: impl FnOnce(&mut TableView<File>, &mut Context<TableView<File>>),
) -> (Entity<TableView<File>>, &mut VisualTestContext) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let controller = app.update(|cx| {
        cx.new(|_| {
            let mut c = ListController::new(|f: &File| f.id.to_string().into());
            c.set_items(files());
            c
        })
    });
    let (table, cx) = app.add_window_view(move |_, cx| {
        let mut table = TableView::new(controller, columns(), cx);
        configure(&mut table, cx);
        table
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    (table, cx)
}

#[test]
fn hiding_the_header_removes_its_hit_area_and_starts_rows_at_the_top() {
    let mut app = TestAppContext::single();
    let (table, cx) = options_fixture(&mut app, |table, cx| {
        table.set_header_visible(false, cx);
    });
    assert!(cx.debug_bounds("table-header").is_none());
    let view = cx.debug_bounds("table-view").unwrap();
    let row = cx.debug_bounds("table-row-0").unwrap();
    assert_eq!(
        row.top(),
        view.top(),
        "without a header the first row starts at the table top"
    );
    assert!(!cx.read(|cx| table.read(cx).header_visible()));
}

#[test]
fn turning_stripes_off_leaves_a_plain_content_background() {
    let mut app = TestAppContext::single();
    let (table, cx) = options_fixture(&mut app, |table, cx| {
        table.set_alternating(false, cx);
    });
    assert!(!cx.read(|cx| table.read(cx).alternating()));
    assert!(
        cx.debug_bounds("table-row-1").is_some(),
        "rows still render without stripes"
    );
}

#[test]
fn fill_width_stretches_the_last_column_and_hides_the_horizontal_bar() {
    let mut app = TestAppContext::single();
    let (_table, cx) = options_fixture(&mut app, |table, cx| {
        table.set_fill_width(true, cx);
    });
    let view = cx.debug_bounds("table-view").expect("table");
    let row = cx.debug_bounds("table-row-0").expect("row");
    assert!(
        (f32::from(row.size.width) - f32::from(view.size.width)).abs() <= 16.0,
        "row {} should fill the table {} (less the vertical bar)",
        row.size.width,
        view.size.width
    );
    assert!(
        cx.debug_bounds("scrollbar-horizontal").is_none(),
        "fill-width content never shows a horizontal bar"
    );
}

struct TwoTables {
    first: Entity<TableView<File>>,
    second: Entity<TableView<File>>,
}

impl gpui::Render for TwoTables {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().h(px(150.)).child(self.first.clone()))
            .child(div().flex_1().child(self.second.clone()))
    }
}

#[test]
fn programmatic_focus_moves_between_two_tables() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let first_controller = app.update(|cx| {
        cx.new(|_| {
            let mut c = ListController::new(|f: &File| f.id.to_string().into());
            c.set_items(files());
            c
        })
    });
    let second_controller = app.update(|cx| {
        cx.new(|_| {
            let mut c = ListController::new(|f: &File| f.id.to_string().into());
            c.set_items(files());
            c
        })
    });
    let (view, cx) = app.add_window_view(move |_, cx| {
        let first = cx.new(|cx| TableView::new(first_controller, columns(), cx));
        let second = cx.new(|cx| TableView::new(second_controller, columns(), cx));
        TwoTables { first, second }
    });
    cx.run_until_parked();
    cx.update(|window, cx| {
        let first = view.read(cx).first.clone();
        let second = view.read(cx).second.clone();
        use gpui::Focusable;
        window.focus(&first.read(cx).focus_handle(cx), cx);
        assert!(first.read(cx).focus_handle(cx).contains_focused(window, cx));
        assert!(!second
            .read(cx)
            .focus_handle(cx)
            .contains_focused(window, cx));
        window.focus(&second.read(cx).focus_handle(cx), cx);
        assert!(second
            .read(cx)
            .focus_handle(cx)
            .contains_focused(window, cx));
        assert!(!first.read(cx).focus_handle(cx).contains_focused(window, cx));
    });
}
