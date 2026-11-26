//! Toolbar items retain measured widths and route overflow actions through the
//! same command/menu infrastructure as their visible counterparts.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    actions, div, prelude::*, px, size, Context, Entity, KeyBinding, Modifiers, Render,
    TestAppContext, VisualTestContext, Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::command::CommandRegistry;
use silvestris::core::command::CommandDescriptor;
use silvestris::toolbar::{Toolbar, ToolbarDisplayMode, ToolbarItem, ToolbarSegment};
use silvestris::{ThemeId, ThemePreference};

actions!(toolbar_test, [FocusNext, FocusPrev]);

struct Host {
    registry: Entity<CommandRegistry>,
    segment_hits: Rc<Cell<usize>>,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar = Toolbar::new(
            "toolbar-test",
            "Test Toolbar",
            self.registry.clone(),
            vec![
                ToolbarItem::command("important", "Important", "test.important").priority(9),
                ToolbarItem::command("first", "First command", "test.first").priority(1),
                ToolbarItem::command("second", "Second command", "test.second").priority(1),
                ToolbarItem::segmented(
                    "modes",
                    "Modes",
                    vec![ToolbarSegment::new("mode-a", "Mode A")],
                    None::<&str>,
                    {
                        let segment_hits = self.segment_hits.clone();
                        move |_, _, _| segment_hits.set(segment_hits.get() + 1)
                    },
                )
                .priority(0),
            ],
        );
        div()
            .key_context("ToolbarTest")
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            .child(
                WindowFrame::new("Toolbar test")
                    .toolbar_component(toolbar, cx)
                    .child(div().size_full()),
            )
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
    });
    cx.run_until_parked();
}

fn setup(
    app: &mut TestAppContext,
    hits: Rc<Cell<usize>>,
) -> (Entity<Host>, &mut VisualTestContext, Rc<Cell<usize>>) {
    let hit_count = hits.clone();
    let segment_hits = Rc::new(Cell::new(0));
    let scenario_segment_hits = segment_hits.clone();
    app.update(|cx| {
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
    });
    let (view, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        for (id, label) in [
            ("test.important", "Important"),
            ("test.first", "First command"),
            ("test.second", "Second command"),
        ] {
            let count = hit_count.clone();
            registry.update(cx, |registry, cx| {
                registry
                    .register(
                        CommandDescriptor::action(id, label),
                        move |_, _, _| count.set(count.get() + 1),
                        cx,
                    )
                    .expect("unique command");
            });
        }
        Host {
            registry,
            segment_hits: segment_hits.clone(),
        }
    });
    (view, cx, scenario_segment_hits)
}

#[test]
fn display_modes_choose_the_standard_toolbar_height() {
    let app = TestAppContext::single();
    app.update(|cx| {
        silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx);
        let registry = cx.new(|_| CommandRegistry::default());
        let icon_only = Toolbar::new("icon-only", "Document", registry.clone(), Vec::new())
            .display_mode(ToolbarDisplayMode::IconOnly);
        let icon_and_label =
            Toolbar::new("icon-and-label", "Document", registry.clone(), Vec::new())
                .display_mode(ToolbarDisplayMode::IconAndLabel);
        let text_only = Toolbar::new("text-only", "Document", registry, Vec::new())
            .display_mode(ToolbarDisplayMode::TextOnly);
        assert_eq!(icon_only.height(cx), 32.0);
        assert_eq!(icon_and_label.height(cx), 49.0);
        assert_eq!(text_only.height(cx), 49.0);
    });
}

#[test]
fn narrow_toolbar_keeps_priority_order_and_moves_hidden_commands_to_menu() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let hits = Rc::new(Cell::new(0));
    let (_view, cx, segment_hits) = setup(&mut app, hits.clone());
    cx.simulate_resize(size(px(190.), px(480.)));
    draw(cx);

    assert!(cx.debug_bounds("important").is_some());
    assert!(cx.debug_bounds("toolbar-overflow").is_some());
    assert!(cx.debug_bounds("first").is_none());
    assert!(cx.debug_bounds("second").is_none());

    let important = cx.debug_bounds("important").expect("visible command");
    cx.simulate_click(important.center(), Modifiers::default());
    draw(cx);
    cx.simulate_keystrokes("space");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").expect("space key"),
    });
    draw(cx);
    assert_eq!(hits.get(), 2, "Space activates the focused toolbar item");

    cx.simulate_keystrokes("tab");
    cx.simulate_keystrokes("space");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").expect("space key"),
    });
    draw(cx);
    assert!(
        cx.debug_bounds("menu-panel-0").is_some(),
        "Tab reaches the overflow control and Space opens it"
    );
    assert!(cx.debug_bounds("menu-item-0-0").is_some());
    assert!(cx.debug_bounds("menu-item-0-1").is_some());
    assert!(cx.debug_bounds("menu-item-0-2").is_some());

    let first = cx
        .debug_bounds("menu-item-0-0")
        .expect("first overflow command");
    cx.simulate_click(first.center(), Modifiers::default());
    draw(cx);
    assert_eq!(hits.get(), 3);

    let overflow = cx
        .debug_bounds("toolbar-overflow")
        .expect("overflow control remains available");
    cx.simulate_click(overflow.center(), Modifiers::default());
    draw(cx);
    let segment = cx
        .debug_bounds("menu-item-0-2")
        .expect("segmented overflow action");
    cx.simulate_click(segment.center(), Modifiers::default());
    draw(cx);
    assert_eq!(segment_hits.get(), 1);
}
