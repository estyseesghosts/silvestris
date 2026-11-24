//! Menus (#16): row geometry, submenu placement and key columns against the
//! 10.6 contextual captures (M: menu.context.*), and programmatic
//! highlighting that never invokes a command.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    deferred, div, point, prelude::*, px, size, Context, Entity, Render, TestAppContext,
    VisualTestContext, Window,
};
use silvestris::command::CommandRegistry;
use silvestris::core::command::{CommandDescriptor, MenuEntry, MenuModel};
use silvestris::menu::{split_shortcut, PopupMenu};
use silvestris::{ThemeId, ThemePreference};

struct Harness {
    menu: Entity<PopupMenu>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(deferred(self.menu.clone()).priority(1))
    }
}

/// The Finder Action menu from `right-click-context-menu-nested.png`, with a
/// hit counter on every command.
fn arrange_by(
    window: &mut Window,
    cx: &mut Context<Harness>,
    hits: Rc<Cell<usize>>,
) -> Entity<PopupMenu> {
    let registry = cx.new(|_| CommandRegistry::default());
    let mut command = |id: &str, label: &str, shortcut: Option<&str>| {
        let count = hits.clone();
        let mut descriptor = CommandDescriptor::action(id, label);
        if let Some(keys) = shortcut {
            descriptor = descriptor.shortcut(keys);
        }
        registry.update(cx, |registry, cx| {
            registry
                .register(descriptor, move |_, _, _| count.set(count.get() + 1), cx)
                .expect("unique command");
        });
        MenuEntry::Command(id.into())
    };
    let sort = vec![
        command("sort.name", "Name", None),
        command("sort.modified", "Date Modified", None),
        command("sort.kind", "Kind", None),
    ];
    let entries = vec![
        command("file.new_folder", "New Folder", Some("secondary-shift-n")),
        command("file.open", "Open", Some("secondary-o")),
        MenuEntry::Separator,
        command("file.info", "Get Info", Some("secondary-i")),
        MenuEntry::Submenu {
            label: "Arrange By".into(),
            entries: sort,
        },
    ];
    let model = MenuModel {
        label: "Action".into(),
        entries,
    };
    cx.new(|cx| PopupMenu::new(model, registry, window, cx))
}

/// Open the menu as a pop-down at `position` in a window of `viewport`, with
/// Arrange By highlighted and its submenu open, then run `check`.
fn with_menu(
    position: (f32, f32),
    viewport: (f32, f32),
    check: impl FnOnce(&Entity<Harness>, &mut VisualTestContext, &Cell<usize>),
) {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let hits = Rc::new(Cell::new(0));
    let count = hits.clone();
    let (view, cx) = app.add_window_view(move |window, cx| Harness {
        menu: arrange_by(window, cx, count),
    });
    cx.simulate_resize(size(px(viewport.0), px(viewport.1)));
    view.update_in(cx, |h, window, cx| {
        h.menu.update(cx, |menu, cx| {
            menu.open_pop_down(point(px(position.0), px(position.1)), window, cx);
            menu.highlight_path(&[4], window, cx);
        });
    });
    cx.run_until_parked();
    check(&view, cx, &hits);
}

#[test]
fn rows_and_separators_use_the_contextual_heights() {
    with_menu((40., 30.), (800., 600.), |_, cx, _| {
        let frame = cx.debug_bounds("menu-panel-0").expect("menu frame");
        let first = cx.debug_bounds("menu-item-0-0").expect("first row");
        let second = cx.debug_bounds("menu-item-0-1").expect("second row");
        let after_separator = cx
            .debug_bounds("menu-item-0-3")
            .expect("row after separator");
        // M: menu.context.rows: edge row, 4 px pad, 18 px items, 12 px separators.
        assert_eq!(first.origin.y - frame.origin.y, px(5.));
        assert_eq!(first.size.height, px(18.));
        assert_eq!(second.origin.y - first.origin.y, px(18.));
        assert_eq!(after_separator.origin.y - second.origin.y, px(18. + 12.));
        assert_eq!(frame.origin, point(px(40.), px(30.)));
    });
}

#[test]
fn narrow_menus_have_no_minimum_width() {
    with_menu((40., 30.), (800., 600.), |_, cx, _| {
        let submenu = cx.debug_bounds("menu-panel-1").expect("submenu");
        // The Arrange By submenu is 131 px wide in 10.6, well under the old
        // 160 px floor. Lato is wider than Lucida Grande, so only bound it.
        assert!(submenu.size.width < px(150.), "{:?}", submenu.size);
    });
}

#[test]
fn submenu_overlaps_the_parent_edge_and_aligns_its_first_row() {
    with_menu((40., 30.), (800., 600.), |_, cx, _| {
        let parent = cx.debug_bounds("menu-panel-0").expect("menu frame");
        let row = cx.debug_bounds("menu-item-0-4").expect("Arrange By row");
        let submenu = cx.debug_bounds("menu-panel-1").expect("submenu");
        let first = cx.debug_bounds("menu-item-1-0").expect("first submenu row");
        // M: menu.context.edge: submenu edge column one inside the parent's.
        assert_eq!(submenu.left(), parent.right() - px(2.));
        // M: menu.context.rows: Name sits on the Arrange By row.
        assert_eq!(first.origin.y, row.origin.y);
    });
}

#[test]
fn submenu_flips_left_at_the_window_edge_with_the_same_overlap() {
    with_menu((500., 30.), (640., 600.), |_, cx, _| {
        let parent = cx.debug_bounds("menu-panel-0").expect("menu frame");
        let submenu = cx.debug_bounds("menu-panel-1").expect("submenu");
        assert!(parent.right() + submenu.size.width > px(640.));
        assert_eq!(submenu.right(), parent.left() + px(2.));
    });
}

#[test]
fn key_equivalents_share_one_left_aligned_column() {
    with_menu((40., 30.), (800., 600.), |_, cx, _| {
        let first = cx.debug_bounds("menu-key-0-0").expect("New Folder key");
        let second = cx.debug_bounds("menu-key-0-1").expect("Open key");
        let third = cx.debug_bounds("menu-key-0-3").expect("Get Info key");
        let frame = cx.debug_bounds("menu-panel-0").expect("menu frame");
        assert_eq!(first.left(), second.left());
        assert_eq!(first.left(), third.left());
        // M: menu.bar.key_column: 7 px from the widest key to the body edge.
        assert!(frame.right() - px(1.) - first.right() >= px(7.));
    });
}

#[test]
fn highlighting_a_path_opens_submenus_without_invoking() {
    with_menu((40., 30.), (800., 600.), |view, cx, hits| {
        assert!(cx.debug_bounds("menu-panel-1").is_some());
        view.update_in(cx, |h, window, cx| {
            h.menu
                .update(cx, |menu, cx| menu.highlight_path(&[4, 1], window, cx));
        });
        cx.run_until_parked();
        assert!(cx.debug_bounds("menu-panel-1").is_some());
        assert_eq!(hits.get(), 0, "highlighting never runs a command");
    });
}

#[test]
fn shortcuts_split_into_modifiers_and_key() {
    assert_eq!(split_shortcut("⌘B"), ("⌘", "B"));
    assert_eq!(split_shortcut("⌥⌘C"), ("⌥⌘", "C"));
    assert_eq!(split_shortcut("⇧⌘⌫"), ("⇧⌘", "⌫"));
    assert_eq!(split_shortcut("Ctrl+Alt+C"), ("Ctrl+Alt+", "C"));
    assert_eq!(split_shortcut("Ctrl++"), ("Ctrl+", "+"));
    assert_eq!(split_shortcut("⌘K ⌘S"), ("⌘K ⌘", "S"));
    assert_eq!(split_shortcut("Esc"), ("", "Esc"));
}
