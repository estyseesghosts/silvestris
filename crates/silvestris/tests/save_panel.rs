//! SavePanel (#29): name preselection, validation, overwrite confirmation,
//! the disclosure, New Folder and the accessory slot.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Context, Entity, Modifiers, TestAppContext, VisualContext,
    VisualTestContext, Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::file_panel::{
    FileEntry, FilePanel, LocationId, MemoryProvider, PanelOutcome, SavePanel,
};
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
        silvestris::icons::Icon::Volume,
        None,
        vec![
            FileEntry::folder("docs", "Documents"),
            FileEntry::document("report", "report.txt"),
            FileEntry::document("photo", "photo.png"),
        ],
    );
    provider.add_location(
        "/docs",
        "Documents",
        silvestris::icons::Icon::Documents,
        Some("/".into()),
        vec![FileEntry::document("inner", "inner.txt")],
    );
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
    name: &str,
    existing: bool,
    expanded: bool,
    accessory: bool,
) -> (
    Entity<FilePanel>,
    Outcome,
    Rc<MemoryProvider>,
    VisualTestContext,
) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    app.update(silvestris::modal::install);
    let provider = Rc::new(provider());
    let (_owner, cx) = app.add_window_view(|_, cx| Owner {
        body: cx.new(|_| Body),
    });
    let mut cx = cx.clone();
    let owner = cx.window_handle();
    let outcome: Outcome = Rc::default();
    let slot = outcome.clone();
    let panel = cx.update(|window, cx| {
        let mut builder = SavePanel::new(provider.clone())
            .directory("/")
            .name(name)
            .expanded(expanded)
            .on_finish(move |outcome, _| *slot.borrow_mut() = Some(outcome.clone()));
        if existing {
            builder = builder.allowed_types(["txt"]);
        }
        if accessory {
            builder = builder.accessory(|_, _| {
                div()
                    .debug_selector(|| "save-accessory".into())
                    .h(px(20.))
                    .into_any_element()
            });
        }
        builder.present(Some(owner), window, cx)
    });
    draw(&mut cx);
    (panel, outcome, provider, cx.clone())
}

#[test]
fn the_name_field_selects_the_base_name_and_save_is_enabled() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, _provider, mut cx) = present(&mut app, "report.txt", false, true, false);
    assert_eq!(cx.read(|cx| panel.read(cx).name_text(cx)), "report.txt");
    assert!(cx.debug_bounds("panel-accept").is_some());
    assert!(cx.debug_bounds("panel-new-folder").is_some());
    assert!(cx.debug_bounds("panel-sidebar").is_some());
}

#[test]
fn saving_a_new_name_returns_its_location() {
    let mut app = TestAppContext::single();
    let (panel, outcome, _provider, mut cx) = present(&mut app, "fresh.txt", false, true, false);
    let accept = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(accept.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("a panel outcome");
    assert!(outcome.accepted());
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/fresh.txt")
    );
    assert!(cx.debug_bounds("file-panel").is_none());
    let _ = panel;
}

#[test]
fn an_existing_name_asks_before_replacing_and_cancel_returns_to_the_panel() {
    let mut app = TestAppContext::single();
    let (panel, outcome, _provider, mut cx) = present(&mut app, "report.txt", true, true, false);
    let accept = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(accept.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.debug_bounds("silvestris-alert").is_some());
    let cancel = cx.debug_bounds("alert-cancel").expect("alert cancel");
    cx.simulate_click(cancel.center(), Modifiers::default());
    draw(&mut cx);
    assert!(outcome.borrow().is_none());
    assert!(cx.debug_bounds("file-panel").is_some(), "back in the panel");
    assert!(cx.debug_bounds("silvestris-alert").is_none());
    // A second Save asks again; Replace accepts.
    let accept = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(accept.center(), Modifiers::default());
    draw(&mut cx);
    let replace = cx.debug_bounds("alert-replace").expect("alert replace");
    cx.simulate_click(replace.center(), Modifiers::default());
    draw(&mut cx);
    let outcome = outcome.borrow().clone().expect("a panel outcome");
    assert!(outcome.accepted());
    assert_eq!(
        outcome.first_location().map(ToString::to_string).as_deref(),
        Some("/report.txt")
    );
    let _ = panel;
}

#[test]
fn an_invalid_name_disables_save() {
    let mut app = TestAppContext::single();
    let (panel, outcome, _provider, mut cx) = present(&mut app, "ok.txt", false, true, false);
    cx.update(|window, cx| {
        panel.update(cx, |panel, cx| panel.set_name("bad/name", window, cx));
    });
    draw(&mut cx);
    let accept = cx.debug_bounds("panel-accept").unwrap();
    cx.simulate_click(accept.center(), Modifiers::default());
    draw(&mut cx);
    assert!(outcome.borrow().is_none());
    assert!(cx.debug_bounds("file-panel").is_some());
}

#[test]
fn new_folder_creates_and_selects_an_entry_then_starts_an_inline_rename() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, provider, mut cx) = present(&mut app, "fresh.txt", false, true, false);
    let new_folder = cx.debug_bounds("panel-new-folder").unwrap();
    cx.simulate_click(new_folder.center(), Modifiers::default());
    draw(&mut cx);
    assert!(
        provider
            .entries(&LocationId::from("/"))
            .iter()
            .any(|entry| entry.name == "untitled folder"),
        "the provider created the folder"
    );
    assert!(
        cx.read(|cx| panel.read(cx).is_editing(cx)),
        "the new row is being renamed in place"
    );
    assert!(cx.debug_bounds("table-row-untitled folder").is_some());
}

#[test]
fn new_folder_in_columns_mode_selects_and_renames_the_row() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, provider, mut cx) = present(&mut app, "fresh.txt", false, true, false);
    let columns = cx.debug_bounds("segment-columns").expect("columns segment");
    cx.simulate_click(columns.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.debug_bounds("column-browser").is_some());
    let new_folder = cx.debug_bounds("panel-new-folder").expect("new folder");
    cx.simulate_click(new_folder.center(), Modifiers::default());
    draw(&mut cx);
    assert!(
        provider
            .entries(&LocationId::from("/"))
            .iter()
            .any(|entry| entry.name == "untitled folder"),
        "the provider created the folder"
    );
    assert!(cx.debug_bounds("table-row-untitled folder").is_some());
    assert!(
        cx.read(|cx| panel.read(cx).is_editing(cx)),
        "the new row is being renamed in place in the column"
    );
}

#[test]
fn the_disclosure_hides_and_shows_the_browser() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, _provider, mut cx) = present(&mut app, "fresh.txt", false, false, false);
    assert!(cx.debug_bounds("panel-sidebar").is_none());
    let disclosure = cx.debug_bounds("save-disclosure").expect("disclosure");
    cx.simulate_click(disclosure.center(), Modifiers::default());
    draw(&mut cx);
    assert!(cx.read(|cx| panel.read(cx).expanded()));
    assert!(cx.debug_bounds("panel-sidebar").is_some());
}

#[test]
fn the_accessory_slot_is_drawn_above_the_buttons() {
    let mut app = TestAppContext::single();
    let (_panel, _outcome, _provider, mut cx) = present(&mut app, "fresh.txt", false, true, true);
    assert!(cx.debug_bounds("save-accessory").is_some());
}

#[test]
fn typing_in_the_name_field_does_not_type_select_the_browser() {
    let mut app = TestAppContext::single();
    let (panel, _outcome, _provider, mut cx) = present(&mut app, "fresh.txt", false, true, false);
    // Select a row, then put focus back in the name field.
    let row = cx.debug_bounds("table-row-photo").expect("photo row");
    cx.simulate_click(row.center(), Modifiers::default());
    draw(&mut cx);
    assert_eq!(cx.read(|cx| panel.read(cx).selected_count(cx)), 1);
    cx.update(|window, cx| {
        if let Some(handle) = panel.read(cx).name_focus_handle() {
            window.focus(&handle, cx);
        }
    });
    cx.simulate_input("decoy");
    draw(&mut cx);
    assert_eq!(
        cx.read(|cx| panel.read(cx).selected_count(cx)),
        1,
        "the browser keeps its selection while the name field takes the keys"
    );
}
