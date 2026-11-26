//! Icon grid (#35): marquee selection against the painted cells, the
//! label-hugging selection pill, keyboard movement after a responsive column
//! change, and the drag source a consumer can match.

use gpui::{
    point, prelude::*, px, size, AppContext, Entity, Modifiers, MouseButton, SharedString,
    TestAppContext, VisualTestContext,
};
use silvestris::binding::{ItemId, ListController};
use silvestris::controls::{icon_grid, IconGrid};
use silvestris::{ThemeId, ThemePreference};

#[derive(Clone, Debug)]
struct Item {
    id: &'static str,
    name: &'static str,
}

fn items() -> Vec<Item> {
    ["a", "b", "c", "d", "e", "f", "g", "h"]
        .iter()
        .map(|id| Item { id, name: id })
        .collect()
}

struct Fixture<'a> {
    grid: Entity<IconGrid<Item>>,
    controller: Entity<ListController<Item>>,
    cx: &'a mut VisualTestContext,
}

struct GridView {
    grid: Entity<IconGrid<Item>>,
}

impl gpui::Render for GridView {
    fn render(
        &mut self,
        _: &mut gpui::Window,
        _: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        gpui::div().size_full().child(self.grid.clone())
    }
}

fn open(app: &mut TestAppContext) -> Fixture<'_> {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let controller = app.update(|cx| {
        cx.new(|_| {
            let mut controller = ListController::new(|item: &Item| ItemId::from(item.id));
            controller.set_items(items());
            controller
        })
    });
    let grid = app.update(|cx| {
        let controller = controller.clone();
        cx.new(|cx| {
            let mut grid = IconGrid::new(
                controller,
                |_: &Item, _selected, _active, _, _| {
                    gpui::div()
                        .size(px(icon_grid::ICON_SIZE))
                        .into_any_element()
                },
                |item: &Item| SharedString::from(item.name),
                cx,
            );
            grid.set_multiple(true, cx);
            grid.set_draggable(true, cx);
            grid
        })
    });
    let window_grid = grid.clone();
    let (_, cx) = app.add_window_view(move |_, _| GridView {
        grid: window_grid.clone(),
    });
    let controller = cx.read(|cx| grid.read(cx).controller());
    cx.simulate_resize(size(px(600.), px(500.)));
    cx.run_until_parked();
    Fixture {
        grid,
        controller,
        cx,
    }
}

impl Fixture<'_> {
    fn cell(&mut self, id: &str) -> gpui::Bounds<gpui::Pixels> {
        self.cx
            .debug_bounds(Box::leak(format!("icon-cell-{id}").into_boxed_str()))
            .unwrap_or_else(|| panic!("cell {id} rendered"))
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

    fn click(&mut self, id: &str) {
        let at = self.cell(id).center();
        self.cx.simulate_click(at, Modifiers::default());
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }
}

#[test]
fn marquee_selects_intersecting_cells_and_background_click_clears() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    fixture.click("a");
    assert_eq!(fixture.selected(), ["a"]);

    let grid = fixture.cx.debug_bounds("icon-grid").expect("grid");
    let first = fixture.cell("a");
    let start = point(first.right() + px(20.), grid.bottom() - px(20.));
    let end = point(first.left() + px(2.), first.top() + px(1.));
    fixture
        .cx
        .simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    fixture
        .cx
        .simulate_mouse_move(end, Some(MouseButton::Left), Modifiers::default());
    fixture
        .cx
        .simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    fixture.cx.run_until_parked();
    let selected = fixture.selected();
    assert!(selected.contains(&"a".to_string()));
    assert!(selected.contains(&"b".to_string()));
    assert!(selected.contains(&"d".to_string()));
    assert!(!selected.contains(&"c".to_string()));

    fixture.cx.simulate_click(start, Modifiers::default());
    fixture.cx.run_until_parked();
    assert!(fixture.selected().is_empty());
}

#[test]
fn the_selection_pill_hugs_the_label_not_the_whole_cell() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    fixture.click("a");
    let cell = fixture.cell("a");
    let pill = fixture
        .cx
        .debug_bounds("icon-selection-a")
        .expect("label selection pill");
    assert!(pill.size.width < cell.size.width);
    assert!(pill.size.height < cell.size.height);
}

#[test]
fn keyboard_movement_uses_the_currently_measured_column_count() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    fixture.click("a");
    fixture.keys("down");
    assert_eq!(fixture.selected(), ["d"], "three columns at 600 px");
    fixture.cx.simulate_resize(size(px(900.), px(500.)));
    fixture.cx.run_until_parked();
    fixture.keys("home");
    assert_eq!(fixture.selected(), ["a"]);
    fixture.keys("down");
    assert_eq!(fixture.selected(), ["f"], "five columns at 900 px");
}

#[test]
fn a_draggable_cell_does_not_change_selection_rules() {
    let mut app = TestAppContext::single();
    let mut fixture = open(&mut app);
    fixture.click("c");
    fixture.click("a");
    assert_eq!(fixture.selected(), ["a"]);
    let grid = fixture.grid.clone();
    assert!(fixture
        .cx
        .read(|cx| grid.read(cx).is_selected(&ItemId::from("a"), cx)));
}
