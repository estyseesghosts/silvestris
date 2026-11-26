//! Outline view (#33): flattened hierarchical rows through the table
//! contract. A branch discloses through the consumer's expansion set, selection
//! and type-select use stable ids, and a refresh keeps the selected id.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{prelude::*, App, Entity, Modifiers, TestAppContext, VisualTestContext};
use silvestris::binding::{ItemId, ListController};
use silvestris::controls::{OutlineEvent, OutlineRow, OutlineView, TableColumn};
use silvestris::{ThemeId, ThemePreference};

#[derive(Clone, Debug, PartialEq)]
struct Node {
    name: &'static str,
    kind: &'static str,
}

fn rows(expanded: bool) -> Vec<OutlineRow<Node>> {
    vec![
        OutlineRow::new(
            "root",
            0,
            Node {
                name: "Root",
                kind: "Folder",
            },
        )
        .branch(expanded),
        OutlineRow::new(
            "child",
            1,
            Node {
                name: "Child Note",
                kind: "Document",
            },
        ),
        OutlineRow::new(
            "sibling",
            0,
            Node {
                name: "Sibling",
                kind: "Folder",
            },
        ),
    ]
}

struct Fixture<'a> {
    view: Entity<OutlineView<Node>>,
    controller: Entity<ListController<OutlineRow<Node>>>,
    toggles: Rc<RefCell<Vec<(String, bool)>>>,
    events: Rc<RefCell<Vec<OutlineEvent>>>,
    cx: &'a mut VisualTestContext,
}

fn open(app: &mut TestAppContext) -> Fixture<'_> {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let toggles: Rc<RefCell<Vec<(String, bool)>>> = Rc::default();
    let events: Rc<RefCell<Vec<OutlineEvent>>> = Rc::default();
    let toggle_sink = toggles.clone();
    let event_sink = events.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let controller = cx.new(|_| {
            let mut controller = ListController::new(|row: &OutlineRow<Node>| row.id.clone());
            controller.set_items(rows(true));
            controller
        });
        let view = OutlineView::new(
            controller,
            |node: &Node| node.name.into(),
            Some(|_: &Node, _: &str, _: &mut App| false),
            move |id, next, _, _| toggle_sink.borrow_mut().push((id.to_string(), next)),
            vec![TableColumn::text(
                "kind",
                "Kind",
                |row: &OutlineRow<Node>| row.item.kind.into(),
            )],
            cx,
        );
        cx.subscribe_self(move |_, event: &OutlineEvent, _| {
            event_sink.borrow_mut().push(event.clone());
        })
        .detach();
        view
    });
    let view = view.clone();
    let controller = cx.read(|cx| view.read(cx).controller());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    Fixture {
        view,
        controller,
        toggles,
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

    fn click(&mut self, id: &str) {
        let at = self.row(id).center();
        self.cx
            .simulate_mouse_down(at, gpui::MouseButton::Left, Modifiers::default());
        self.cx
            .simulate_mouse_up(at, gpui::MouseButton::Left, Modifiers::default());
        self.cx.run_until_parked();
    }

    fn selected(&self) -> Vec<String> {
        let controller = self.controller.clone();
        self.cx.read(|cx| {
            controller
                .read(cx)
                .selected_ids()
                .iter()
                .map(ToString::to_string)
                .collect()
        })
    }

    fn set_rows(&mut self, rows: Vec<OutlineRow<Node>>) {
        let view = self.view.clone();
        self.cx
            .update(|_, cx| view.update(cx, |view, cx| view.set_rows(rows, cx)));
        self.cx.run_until_parked();
    }
}

#[test]
fn rows_render_through_table_with_depth_and_columns() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    assert!(fixture.cx.debug_bounds("table-view").is_some());
    assert!(fixture.cx.debug_bounds("outline-disclosure-root").is_some());
    assert!(fixture
        .cx
        .debug_bounds("outline-disclosure-child")
        .is_none());
    assert!(fixture.row("child").size.width > gpui::px(0.));
}

#[test]
fn disclosure_reports_the_requested_state_without_selecting() {
    let mut app = TestAppContext::single();
    let fixture = open(&mut app);
    let triangle = fixture
        .cx
        .debug_bounds("outline-disclosure-root")
        .expect("branch disclosure");
    fixture
        .cx
        .simulate_click(triangle.center(), Modifiers::default());
    fixture.cx.run_until_parked();
    assert_eq!(
        fixture.toggles.borrow().as_slice(),
        [("root".into(), false)]
    );
    assert!(fixture.selected().is_empty());
}

#[test]
fn clicking_and_type_select_use_stable_ids() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    fixture.click("child");
    assert_eq!(fixture.selected(), ["child"]);
    assert!(fixture
        .events
        .borrow()
        .iter()
        .any(|event| matches!(event, OutlineEvent::SelectionChanged(ids) if ids == &[ItemId::from("child")])));

    fixture.cx.simulate_keystrokes("s");
    fixture.cx.run_until_parked();
    assert_eq!(fixture.selected(), ["sibling"]);
}

#[test]
fn a_refresh_keeps_selection_when_the_id_still_exists() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    fixture.click("sibling");
    fixture.set_rows(vec![OutlineRow::new(
        "sibling",
        0,
        Node {
            name: "Sibling Renamed",
            kind: "Folder",
        },
    )
    .branch(false)]);
    assert_eq!(fixture.selected(), ["sibling"]);
    assert!(fixture
        .cx
        .debug_bounds("outline-disclosure-sibling")
        .is_some());
}

#[test]
fn inline_rename_commits_through_the_consumer() {
    let mut app = TestAppContext::single();
    app.update(silvestris::controls::text_field::bind_keys);
    let renamed: Rc<RefCell<Vec<String>>> = Rc::default();
    let sink = renamed.clone();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let (view, cx) = app.add_window_view(move |_, cx| {
        let controller = cx.new(|_| {
            let mut controller = ListController::new(|row: &OutlineRow<Node>| row.id.clone());
            controller.set_items(rows(true));
            controller
        });
        OutlineView::new(
            controller,
            |node: &Node| node.name.into(),
            Some(move |_: &Node, text: &str, _: &mut App| {
                sink.borrow_mut().push(text.to_string());
                true
            }),
            |_, _, _, _| {},
            vec![],
            cx,
        )
    });
    cx.run_until_parked();
    let row = cx.debug_bounds("table-row-child").expect("row");
    cx.simulate_click(row.center(), Modifiers::default());
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(cx.read(|cx| view.read(cx).is_editing(cx)));
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("Renamed");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!cx.read(|cx| view.read(cx).is_editing(cx)));
    assert_eq!(renamed.borrow().as_slice(), ["Renamed"]);
}
