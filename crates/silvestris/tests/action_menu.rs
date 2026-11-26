//! Action menus, command pop-downs and scope bars (#46): both menus contain
//! commands and invoke them through the registry; a scope bar reports only the
//! chosen scope.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{div, prelude::*, Context, Entity, Modifiers, Render, TestAppContext, Window};
use silvestris::command::CommandRegistry;
use silvestris::controls::{ActionMenu, CommandPopDown, Scope, ScopeBar};
use silvestris::core::command::{CommandDescriptor, MenuEntry, MenuModel};
use silvestris::{ThemeId, ThemePreference};

struct MenuHarness {
    registry: Entity<CommandRegistry>,
    model: MenuModel,
    action: bool,
}

impl Render for MenuHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(if self.action {
            ActionMenu::new(
                "gallery-action",
                "Action",
                self.model.clone(),
                self.registry.clone(),
            )
            .into_any_element()
        } else {
            CommandPopDown::new(
                "gallery-more",
                "More",
                self.model.clone(),
                self.registry.clone(),
            )
            .into_any_element()
        })
    }
}

struct ScopeHarness {
    chosen: Rc<RefCell<Vec<String>>>,
}

impl Render for ScopeHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let chosen = self.chosen.clone();
        div().size_full().child(
            ScopeBar::new(
                "gallery-scope",
                "Search scope",
                vec![
                    Scope::new("all", "All"),
                    Scope::new("docs", "Documents"),
                    Scope::new("images", "Images"),
                ],
            )
            .caption("Search:")
            .selected("all")
            .on_select(move |id, _, _| chosen.borrow_mut().push(id.to_string())),
        )
    }
}

fn menu(
    app: &mut TestAppContext,
    action: bool,
) -> (
    Entity<MenuHarness>,
    Rc<Cell<usize>>,
    &mut gpui::VisualTestContext,
) {
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let hits = Rc::new(Cell::new(0));
    let count = hits.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::action("file.new_folder", "New Folder")
                        .shortcut("secondary-shift-n"),
                    move |_, _, _| count.set(count.get() + 1),
                    cx,
                )
                .expect("unique command");
        });
        MenuHarness {
            registry,
            model: MenuModel {
                label: if action { "Action" } else { "More" }.into(),
                entries: vec![
                    MenuEntry::Command("file.new_folder".into()),
                    MenuEntry::Separator,
                    MenuEntry::Heading("Folder actions".into()),
                ],
            },
            action,
        }
    });
    (view, hits, cx)
}

fn click(cx: &mut gpui::VisualTestContext, selector: &'static str) -> gpui::Bounds<gpui::Pixels> {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} rendered"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    cx.run_until_parked();
    bounds
}

#[test]
fn action_menu_opens_a_command_menu_and_invokes_it() {
    let mut app = TestAppContext::single();
    let (_view, hits, cx) = menu(&mut app, true);
    click(cx, "gallery-action");
    let row = cx.debug_bounds("menu-item-0-0").expect("Action menu row");
    cx.simulate_click(row.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(hits.get(), 1);
    assert!(cx.debug_bounds("menu-panel-0").is_none(), "menu dismissed");
}

#[test]
fn command_pop_down_opens_the_same_command_menu() {
    let mut app = TestAppContext::single();
    let (_view, hits, cx) = menu(&mut app, false);
    click(cx, "gallery-more");
    let row = cx
        .debug_bounds("menu-item-0-0")
        .expect("command pop-down row");
    cx.simulate_click(row.center(), Modifiers::default());
    cx.run_until_parked();
    assert_eq!(hits.get(), 1);
    assert!(cx.debug_bounds("menu-panel-0").is_none(), "menu dismissed");
}

#[test]
fn scope_bar_reports_only_the_chosen_scope() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let chosen: Rc<RefCell<Vec<String>>> = Rc::default();
    let sink = chosen.clone();
    let (_view, cx) = app.add_window_view(move |_, _| ScopeHarness { chosen: sink });
    cx.run_until_parked();
    assert!(cx.debug_bounds("scope-bar").is_some());
    click(cx, "segment-docs");
    assert_eq!(chosen.borrow().as_slice(), ["docs"]);
}
