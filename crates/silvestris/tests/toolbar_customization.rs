//! Toolbar customization (#26): stored order/hidden sets, optional items, the
//! palette toggles, default-set restoration and the explicit non-customizable
//! flag.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Context, Entity, Modifiers, Render, TestAppContext, VisualTestContext,
    Window,
};
use silvestris::command::CommandRegistry;
use silvestris::core::command::CommandDescriptor;
use silvestris::toolbar::{
    MemoryToolbars, Toolbar, ToolbarDisplayMode, ToolbarItem, ToolbarLayout, ToolbarPalette,
    ToolbarStore,
};
use silvestris::{ThemeId, ThemePreference};

type Layout = Rc<RefCell<ToolbarLayout>>;

struct Host {
    registry: Entity<CommandRegistry>,
    layout: Layout,
    palette: bool,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let layout = self.layout.borrow().clone();
        let state = self.layout.clone();
        let toolbar = Toolbar::new(
            "custom-toolbar",
            "Custom toolbar",
            self.registry.clone(),
            items(),
        )
        .layout(layout.clone())
        .display_mode(ToolbarDisplayMode::IconAndLabel);
        let palette = self.palette.then(|| {
            let state = state.clone();
            ToolbarPalette::new("custom-palette", "Customize toolbar", items(), layout).on_change(
                move |next, _, cx| {
                    *state.borrow_mut() = next;
                    cx.refresh_windows();
                },
            )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w(px(640.))
            .child(toolbar)
            .children(palette)
    }
}

struct StoredHost {
    registry: Entity<CommandRegistry>,
    store: Rc<MemoryToolbars>,
}

impl Render for StoredHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(640.)).h(px(60.)).child(
            Toolbar::new("stored-toolbar", "Stored", self.registry.clone(), items())
                .autosave(self.store.clone(), "toolbar"),
        )
    }
}

fn items() -> Vec<ToolbarItem> {
    vec![
        ToolbarItem::command("keep", "Keep", "test.keep"),
        ToolbarItem::command("opt-a", "Option A", "test.opt-a").optional(true),
        ToolbarItem::command("opt-b", "Option B", "test.opt-b").optional(true),
    ]
}

fn layout(order: &[&str], hidden: &[&str]) -> ToolbarLayout {
    ToolbarLayout {
        order: order.iter().map(|id| (*id).to_string()).collect(),
        hidden: hidden.iter().map(|id| (*id).to_string()).collect(),
        mode: None,
    }
}

fn open(
    app: &mut TestAppContext,
    layout: ToolbarLayout,
    palette: bool,
) -> (Layout, Entity<Host>, &mut VisualTestContext) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let state = Rc::new(RefCell::new(layout));
    let host_state = state.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            for id in ["test.keep", "test.opt-a", "test.opt-b"] {
                registry
                    .register(CommandDescriptor::action(id, id), |_, _, _| {}, cx)
                    .expect("unique command");
            }
        });
        Host {
            registry,
            layout: host_state,
            palette,
        }
    });
    cx.run_until_parked();
    (state, view, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} rendered"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
}

#[test]
fn a_layout_reorders_and_hides_optional_items_in_the_toolbar() {
    let mut app = TestAppContext::single();
    let (_state, _view, cx) = open(
        &mut app,
        layout(&["opt-b", "keep", "opt-a"], &["opt-a"]),
        false,
    );
    let keep = cx.debug_bounds("keep").expect("required item");
    let opt_b = cx.debug_bounds("opt-b").expect("optional item");
    assert!(opt_b.origin.x < keep.origin.x, "saved order applies");
    assert!(cx.debug_bounds("opt-a").is_none(), "hidden optional item");
}

#[test]
fn the_palette_toggles_optional_items_and_restores_the_default_set() {
    let mut app = TestAppContext::single();
    let (state, _view, cx) = open(
        &mut app,
        layout(&["keep", "opt-a", "opt-b"], &["opt-a"]),
        true,
    );
    assert!(cx.debug_bounds("opt-a").is_none());
    click(cx, "palette-opt-a");
    assert!(!state.borrow().hidden.iter().any(|id| id == "opt-a"));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("opt-a").is_some(), "palette added the item");

    click(cx, "palette-opt-a");
    assert!(state.borrow().hidden.iter().any(|id| id == "opt-a"));

    click(cx, "custom-palette-default");
    assert!(state.borrow().hidden.is_empty(), "Default Set restores all");
    assert_eq!(state.borrow().order, ["keep", "opt-a", "opt-b"]);
}

#[test]
fn autosave_loads_the_stored_arrangement() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let store = Rc::new(MemoryToolbars::default());
    app.update(|cx| store.save("toolbar", layout(&["opt-b", "keep"], &["opt-a"]), cx));
    let host_store = store.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            for id in ["test.keep", "test.opt-a", "test.opt-b"] {
                registry
                    .register(CommandDescriptor::action(id, id), |_, _, _| {}, cx)
                    .expect("unique command");
            }
        });
        StoredHost {
            registry,
            store: host_store.clone(),
        }
    });
    cx.run_until_parked();
    let keep = cx.debug_bounds("keep").expect("required item");
    let opt_b = cx.debug_bounds("opt-b").expect("optional item");
    assert!(opt_b.origin.x < keep.origin.x);
    assert!(cx.debug_bounds("opt-a").is_none());
    let _ = view;
}

#[test]
fn a_toolbar_can_be_marked_non_customizable() {
    let app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let registry = app.update(|cx| cx.new(|_| CommandRegistry::default()));
    let fixed = Toolbar::new("fixed", "Fixed", registry, items()).customizable(false);
    assert!(!fixed.is_customizable());
    let normal = Toolbar::new(
        "normal",
        "Normal",
        app.update(|cx| cx.new(|_| CommandRegistry::default())),
        items(),
    );
    assert!(normal.is_customizable());
}

#[test]
fn a_saved_icon_only_mode_selects_the_icon_only_height() {
    let app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let registry = app.update(|cx| cx.new(|_| CommandRegistry::default()));
    let mut saved = ToolbarLayout::default_for(&items());
    saved.mode = Some(ToolbarDisplayMode::IconOnly);
    let toolbar = Toolbar::new("saved-mode", "Saved mode", registry, items()).layout(saved);
    assert_eq!(app.update(|cx| toolbar.height(cx)), 32.0);
}
