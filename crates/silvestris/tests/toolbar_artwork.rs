//! Standalone pictorial toolbar commands and consumer glyphs: geometry,
//! activation, display modes, disabled state and overflow (#26/#48).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    actions, div, prelude::*, px, size, Context, Entity, KeyBinding, Modifiers, Render,
    TestAppContext, VisualTestContext, Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::command::CommandRegistry;
use silvestris::core::command::CommandDescriptor;
use silvestris::core::Rgba8;
use silvestris::icons::{ConsumerArtwork, ConsumerTemplateImage, PictorialImage, TemplateImage};
use silvestris::toolbar::{
    Toolbar, ToolbarDisplayMode, ToolbarGlyph, ToolbarItem, ToolbarPictorial, ToolbarSegment,
};
use silvestris::{ThemeId, ThemePreference};

actions!(toolbar_artwork_test, [FocusNext, FocusPrev]);

struct Host {
    registry: Entity<CommandRegistry>,
    mode: ToolbarDisplayMode,
}

fn consumer_template() -> ConsumerTemplateImage {
    ConsumerTemplateImage::new("Details", |bounds, ink, window| {
        window.paint_quad(gpui::fill(bounds, silvestris::convert::hsla(ink)));
    })
}

fn consumer_artwork() -> ConsumerArtwork {
    ConsumerArtwork::with_state("Primary", |state, _, _| {
        let ink = if !state.enabled {
            0x9a9a9a
        } else if state.pressed {
            0x1f4f7a
        } else {
            0x3f8fcc
        };
        div()
            .size(px(32.))
            .rounded(px(6.))
            .bg(silvestris::convert::hsla(Rgba8::hex(ink)))
    })
}

fn items() -> Vec<ToolbarItem> {
    vec![
        ToolbarItem::pictorial_command(
            "pictorial",
            "New",
            "test.pictorial",
            ToolbarPictorial::Silvestris(PictorialImage::New),
        )
        .priority(0),
        ToolbarItem::pictorial_command(
            "pictorial-consumer",
            "Primary",
            "test.primary",
            ToolbarPictorial::from(consumer_artwork()),
        )
        .priority(8),
        ToolbarItem::command("details", "Details", "test.details")
            .glyph(ToolbarGlyph::Consumer(consumer_template()))
            .priority(7),
        ToolbarItem::command("stop", "Stop", "test.stop")
            .image(TemplateImage::Stop)
            .priority(6),
        ToolbarItem::pictorial_toggle(
            "checked",
            "Checked",
            "test.checked",
            ToolbarPictorial::Silvestris(PictorialImage::Preferences),
        )
        .priority(5),
        ToolbarItem::pictorial_command(
            "unavailable",
            "Unavailable",
            "test.unavailable",
            ToolbarPictorial::Silvestris(PictorialImage::Save),
        )
        .priority(1),
    ]
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let toolbar = Toolbar::new("artwork-toolbar", "Artwork", self.registry.clone(), items())
            .display_mode(self.mode);
        div()
            .key_context("ToolbarArtworkTest")
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            .child(
                WindowFrame::new("Artwork")
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
    mode: ToolbarDisplayMode,
) -> (
    Rc<Cell<usize>>,
    Entity<CommandRegistry>,
    &mut VisualTestContext,
) {
    app.update(|cx| {
        silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx);
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
    });
    let hits = Rc::new(Cell::new(0));
    let hit_count = hits.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        for (id, label, enabled, checked) in [
            ("test.pictorial", "New", true, false),
            ("test.primary", "Primary", true, false),
            ("test.details", "Details", true, false),
            ("test.stop", "Stop", true, false),
            ("test.checked", "Checked", true, true),
            ("test.unavailable", "Unavailable", false, false),
        ] {
            let count = hit_count.clone();
            let descriptor = if checked {
                CommandDescriptor::toggle(id, label, checked)
            } else {
                CommandDescriptor::action(id, label)
            };
            registry.update(cx, |registry, cx| {
                registry
                    .register(
                        descriptor.enabled(enabled),
                        move |_, _, _| count.set(count.get() + 1),
                        cx,
                    )
                    .expect("unique command");
            });
        }
        Host { registry, mode }
    });
    draw(cx);
    let registry = cx.update(|_, cx| view.read(cx).registry.clone());
    (hits, registry, cx)
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let bounds = cx
        .debug_bounds(selector)
        .unwrap_or_else(|| panic!("{selector} rendered"));
    cx.simulate_click(bounds.center(), Modifiers::default());
    draw(cx);
}

#[test]
fn pictorial_icon_and_label_geometry_is_stacked_and_centred() {
    let mut app = TestAppContext::single();
    let (_hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    let item = cx.debug_bounds("pictorial").expect("pictorial item");
    let icon = cx
        .debug_bounds("pictorial-icon")
        .expect("pictorial artwork");
    let label = cx.debug_bounds("pictorial-label").expect("pictorial label");
    assert!((icon.center().x - item.center().x).as_f32().abs() <= 0.5);
    assert!(
        (label.center().x - icon.center().x).as_f32().abs() <= 1.0,
        "label horizontally centred under the artwork"
    );
    assert!(
        label.origin.y >= icon.bottom() - px(0.5),
        "label below artwork"
    );
    assert!(
        label.bottom() <= item.bottom() + px(0.5),
        "item inside toolbar"
    );
    assert!(item.origin.y >= px(0.0));
    assert!(item.size.height.as_f32() <= 49.5);
    assert_eq!(icon.size.width.as_f32(), 32.0);
}

#[test]
fn display_modes_switch_the_pictorial_presentation() {
    let mut app = TestAppContext::single();
    let (_hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconOnly);
    assert!(cx.debug_bounds("pictorial-icon").is_some());
    assert!(cx.debug_bounds("pictorial-label").is_none());

    let mut app = TestAppContext::single();
    let (_hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::TextOnly);
    assert!(cx.debug_bounds("pictorial-icon").is_none());
    assert!(cx.debug_bounds("pictorial-label").is_some());

    let mut app = TestAppContext::single();
    let (_hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    assert!(cx.debug_bounds("pictorial-icon").is_some());
    assert!(cx.debug_bounds("pictorial-label").is_some());
}

#[test]
fn global_and_consumer_artwork_dispatch_the_same_commands() {
    let mut app = TestAppContext::single();
    let (hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    click(cx, "pictorial");
    assert_eq!(hits.get(), 1, "global pictorial command");
    click(cx, "pictorial-consumer");
    assert_eq!(hits.get(), 2, "consumer pictorial command");
    click(cx, "details");
    assert_eq!(hits.get(), 3, "consumer template glyph command");
    click(cx, "stop");
    assert_eq!(hits.get(), 4, "global template glyph command");
    click(cx, "unavailable");
    assert_eq!(hits.get(), 4, "disabled pictorial command stays inert");
}

#[test]
fn space_activates_a_focused_pictorial_command() {
    let mut app = TestAppContext::single();
    let (hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    let bounds = cx.debug_bounds("pictorial").expect("pictorial item");
    cx.simulate_click(bounds.center(), Modifiers::default());
    draw(cx);
    assert_eq!(hits.get(), 1);
    cx.simulate_keystrokes("space");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").expect("space key"),
    });
    draw(cx);
    assert_eq!(hits.get(), 2, "Space activates the focused item");
}

#[test]
fn a_checked_pictorial_toggle_dispatches_like_any_command() {
    let mut app = TestAppContext::single();
    let (hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    assert!(cx.debug_bounds("checked").is_some());
    click(cx, "checked");
    assert_eq!(hits.get(), 1, "checked pictorial toggle dispatches");
}

#[test]
fn overflow_uses_the_pictorial_command_label_and_dispatches() {
    let mut app = TestAppContext::single();
    let (hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    cx.simulate_resize(size(px(150.), px(480.)));
    draw(cx);
    assert!(cx.debug_bounds("toolbar-overflow").is_some());
    assert!(cx.debug_bounds("pictorial").is_none());
    let overflow = cx.debug_bounds("toolbar-overflow").expect("overflow");
    cx.simulate_click(overflow.center(), Modifiers::default());
    draw(cx);
    assert!(cx.debug_bounds("menu-panel-0").is_some());
    let first = cx
        .debug_bounds("menu-item-0-0")
        .expect("first overflow entry");
    cx.simulate_click(first.center(), Modifiers::default());
    draw(cx);
    assert_eq!(hits.get(), 1, "overflow dispatches the same command");
}

struct SegmentHost {
    registry: Entity<CommandRegistry>,
    selections: Rc<Cell<usize>>,
}

impl Render for SegmentHost {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let selections = self.selections.clone();
        let items = vec![ToolbarItem::segmented(
            "view-switch",
            "View",
            vec![
                ToolbarSegment::new("primary", "Primary")
                    .glyph(ToolbarGlyph::Consumer(consumer_template())),
                ToolbarSegment::new("secondary", "Secondary")
                    .glyph(ToolbarGlyph::Silvestris(TemplateImage::ListView)),
            ],
            None::<&str>,
            move |_, _, _| selections.set(selections.get() + 1),
        )];
        div()
            .w(px(480.))
            .h(px(60.))
            .child(Toolbar::new(
                "segment-toolbar",
                "View",
                self.registry.clone(),
                items,
            ))
            .child(div().size_full())
    }
}

#[test]
fn consumer_glyphs_render_in_a_toolbar_segmented_control() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let selections = Rc::new(Cell::new(0));
    let scenario = selections.clone();
    let (_view, cx) = app.add_window_view(move |_, cx| SegmentHost {
        registry: cx.new(|_| CommandRegistry::default()),
        selections: scenario.clone(),
    });
    draw(cx);
    let primary = cx
        .debug_bounds("segment-primary")
        .expect("consumer segment");
    let secondary = cx
        .debug_bounds("segment-secondary")
        .expect("global segment");
    assert!(primary.size.width.as_f32() > 0.0);
    assert_eq!(primary.size.width, secondary.size.width);
    cx.simulate_click(secondary.center(), Modifiers::default());
    draw(cx);
    assert_eq!(selections.get(), 1);
    cx.simulate_click(primary.center(), Modifiers::default());
    draw(cx);
    assert_eq!(selections.get(), 2);
}

#[test]
fn a_disabled_pictorial_command_re_enables_through_the_registry() {
    let mut app = TestAppContext::single();
    let (hits, registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    click(cx, "unavailable");
    assert_eq!(hits.get(), 0, "disabled command is inert");
    cx.update(|_, cx| {
        registry.update(cx, |registry, cx| {
            registry
                .publish(
                    CommandDescriptor::action("test.unavailable", "Unavailable"),
                    cx,
                )
                .expect("publish enabled state");
        });
    });
    draw(cx);
    click(cx, "unavailable");
    assert_eq!(hits.get(), 1, "re-enabled command dispatches");
}

#[test]
fn tab_moves_focus_between_pictorial_commands_and_space_activates_the_next() {
    let mut app = TestAppContext::single();
    let (hits, _registry, cx) = setup(&mut app, ToolbarDisplayMode::IconAndLabel);
    click(cx, "stop");
    assert_eq!(hits.get(), 1, "pointer focuses and activates Stop");
    cx.simulate_keystrokes("tab");
    draw(cx);
    cx.simulate_keystrokes("space");
    cx.simulate_event(gpui::KeyUpEvent {
        keystroke: gpui::Keystroke::parse("space").expect("space key"),
    });
    draw(cx);
    assert_eq!(
        hits.get(),
        2,
        "Tab reaches the next pictorial control and Space activates it"
    );
}
