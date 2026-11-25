//! Column browser (#34): late children, path movement, selection by id,
//! virtualisation and the preview column.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window,
};
use silvestris::binding::ItemId;
use silvestris::controls::{BrowserEvent, BrowserNode, ColumnBrowser};
use silvestris::{ThemeId, ThemePreference};

struct BrowserView {
    browser: Entity<ColumnBrowser>,
}

impl gpui::Render for BrowserView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.browser.clone())
    }
}

type Events = Rc<RefCell<Vec<BrowserEvent>>>;

struct Fixture<'a> {
    browser: Entity<ColumnBrowser>,
    events: Events,
    cx: &'a mut VisualTestContext,
}

fn open(app: &mut TestAppContext, roots: Vec<BrowserNode>) -> Fixture<'_> {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let events: Events = Rc::default();
    let sink = events.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let browser = cx.new(|cx| ColumnBrowser::new(roots, cx));
        cx.subscribe(&browser, move |_, _, event: &BrowserEvent, _| {
            sink.borrow_mut().push(event.clone());
        })
        .detach();
        BrowserView { browser }
    });
    let browser = cx.update(|_, cx| view.read(cx).browser.clone());
    cx.run_until_parked();
    Fixture {
        browser,
        events,
        cx,
    }
}

impl Fixture<'_> {
    fn click(&mut self, id: &str) {
        let bounds = self
            .cx
            .debug_bounds(Box::leak(format!("table-row-{id}").into_boxed_str()))
            .unwrap_or_else(|| panic!("row {id} rendered"));
        self.cx
            .simulate_click(bounds.center(), Modifiers::default());
        self.cx.run_until_parked();
    }

    fn keys(&mut self, keys: &str) {
        self.cx.simulate_keystrokes(keys);
        self.cx.run_until_parked();
    }

    fn path(&mut self) -> Vec<String> {
        let browser = self.browser.clone();
        self.cx.read(|cx| {
            browser
                .read(cx)
                .path(cx)
                .iter()
                .map(ToString::to_string)
                .collect()
        })
    }

    fn columns(&mut self) -> usize {
        let browser = self.browser.clone();
        self.cx.read(|cx| browser.read(cx).column_count())
    }

    fn set_children(&mut self, parent: &str, nodes: Vec<BrowserNode>) {
        let parent = parent.to_string();
        let browser = self.browser.clone();
        self.cx.update(|_, cx| {
            browser.update(cx, |browser, cx| browser.set_children(&parent, nodes, cx))
        });
        self.cx.run_until_parked();
    }
}

fn leaf(id: &str) -> BrowserNode {
    BrowserNode::leaf(id, id.to_uppercase())
}

fn branch(id: &str) -> BrowserNode {
    BrowserNode::branch(id, id.to_uppercase())
}

#[test]
fn selecting_a_branch_requests_children_and_accepts_a_late_answer() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, vec![branch("alpha"), leaf("bravo")]);
    f.click("alpha");
    assert!(
        f.events
            .borrow()
            .contains(&BrowserEvent::NeedsChildren(ItemId::from("alpha"))),
        "the browser asks the app for the branch's children"
    );
    assert_eq!(f.columns(), 2);
    assert!(!f.cx.read(|cx| f.browser.read(cx).is_loaded(1)));

    f.set_children("alpha", vec![leaf("alpha-1"), leaf("alpha-2")]);
    assert!(f.cx.read(|cx| f.browser.read(cx).is_loaded(1)));
    assert_eq!(f.columns(), 2);
    f.click("alpha-1");
    assert_eq!(f.path(), ["alpha", "alpha-1"]);
    assert_eq!(f.columns(), 2, "a leaf opens no column");
}

#[test]
fn arrow_keys_move_between_columns_and_left_clears_the_column_it_leaves() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, vec![branch("alpha"), leaf("bravo")]);
    f.click("alpha");
    f.set_children("alpha", vec![leaf("alpha-1"), leaf("alpha-2")]);
    f.keys("right");
    assert_eq!(f.path(), ["alpha", "alpha-1"]);
    f.keys("down");
    assert_eq!(f.path(), ["alpha", "alpha-2"]);
    f.keys("left");
    assert_eq!(f.path(), ["alpha"], "Left clears the column it leaves");
    assert_eq!(f.columns(), 2, "the cleared column stays open");
    f.keys("right");
    assert_eq!(
        f.path(),
        ["alpha", "alpha-1"],
        "Right re-enters the first child"
    );
}

#[test]
fn a_late_answer_for_a_truncated_column_is_cached_without_reopening_it() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, vec![branch("alpha"), leaf("bravo")]);
    f.click("alpha");
    assert_eq!(f.columns(), 2);
    f.click("bravo");
    assert_eq!(f.columns(), 1, "a leaf truncates the columns to its right");
    f.set_children("alpha", vec![leaf("alpha-1")]);
    assert_eq!(f.columns(), 1, "a late answer does not re-open a column");
    assert_eq!(
        f.cx.read(|cx| f.browser.read(cx).children_of("alpha").map(<[_]>::len)),
        Some(1)
    );
    f.click("alpha");
    assert_eq!(f.columns(), 2, "the cached answer fills the new column");
    assert!(f.cx.debug_bounds("table-row-alpha-1").is_some());
}

#[test]
fn reloading_children_keeps_the_selection_by_id() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, vec![branch("alpha")]);
    f.click("alpha");
    f.set_children("alpha", vec![leaf("alpha-1"), leaf("alpha-2")]);
    f.click("alpha-2");
    assert_eq!(f.path(), ["alpha", "alpha-2"]);
    f.set_children(
        "alpha",
        vec![leaf("alpha-0"), leaf("alpha-1"), leaf("alpha-2")],
    );
    assert_eq!(
        f.path(),
        ["alpha", "alpha-2"],
        "selection is an id, not an index"
    );
    assert!(f.cx.debug_bounds("table-row-alpha-0").is_some());
}

#[test]
fn the_branch_cache_is_bounded_after_visiting_many_branches() {
    let mut app = TestAppContext::single();
    let total = silvestris::controls::column_browser::MAX_CACHED_BRANCHES + 12;
    let roots: Vec<BrowserNode> = (0..total)
        .map(|index| branch(&format!("branch-{index}")))
        .collect();
    let mut f = open(&mut app, roots);
    for index in 0..total {
        let id = format!("branch-{index}");
        f.click(&id);
        f.set_children(&id, vec![leaf("child")]);
    }
    let cached = f.cx.read(|cx| f.browser.read(cx).cached_branch_count());
    assert!(
        cached <= silvestris::controls::column_browser::MAX_CACHED_BRANCHES + 1,
        "cold branch listings are evicted: {cached} cached"
    );
    // The currently open path is still resolved from our answer.
    assert!(f.cx.debug_bounds("table-row-child").is_some());
}

#[test]
fn a_column_of_ten_thousand_rows_builds_only_the_rows_in_view() {
    let mut app = TestAppContext::single();
    let roots: Vec<BrowserNode> = (0..10_000).map(|i| leaf(&format!("row-{i}"))).collect();
    let f = open(&mut app, roots);
    assert!(f.cx.debug_bounds("table-row-row-0").is_some());
    assert!(
        f.cx.debug_bounds("table-row-row-500").is_none(),
        "rows past the viewport are not built"
    );
}

#[test]
fn the_preview_column_appears_for_a_leaf_only() {
    let mut app = TestAppContext::single();
    let mut f = open(&mut app, vec![branch("alpha"), leaf("bravo")]);
    let browser = f.browser.clone();
    f.cx.update(|_, cx| {
        browser.update(cx, |browser, cx| {
            browser.set_preview(
                |node, _, _| {
                    div()
                        .debug_selector(|| "browser-preview-content".into())
                        .child(node.label.clone())
                        .into_any_element()
                },
                cx,
            )
        })
    });
    f.cx.run_until_parked();
    assert!(f.cx.debug_bounds("browser-preview").is_none());

    f.click("bravo");
    assert!(f.cx.debug_bounds("browser-preview").is_some());
    assert!(f.cx.debug_bounds("browser-preview-content").is_some());

    f.click("alpha");
    assert!(
        f.cx.debug_bounds("browser-preview").is_none(),
        "a branch has no preview column"
    );
}
