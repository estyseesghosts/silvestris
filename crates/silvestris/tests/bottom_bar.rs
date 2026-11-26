//! Bottom bars keep command controls at their standard size while status text
//! truncates, and route activations through the command/status APIs (#75).

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, size, Context, Entity, Modifiers, Render, TestAppContext,
    VisualTestContext, Window,
};
use silvestris::bottom_bar::{
    BottomBar, BottomBarControlStyle, BottomBarItem, BottomBarSize, BottomBarStatus,
};
use silvestris::chrome::WindowFrame;
use silvestris::command::CommandRegistry;
use silvestris::core::command::CommandDescriptor;
use silvestris::icons::TemplateImage;
use silvestris::{ThemeId, ThemePreference};

struct Host {
    registry: Entity<CommandRegistry>,
    status_hits: Rc<Cell<usize>>,
    size: BottomBarSize,
}

impl Render for Host {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hits = self.status_hits.clone();
        let bar = BottomBar::new(
            "test-bottom-bar",
            "Test bottom bar",
            vec![
                BottomBarItem::status(
                    BottomBarStatus::new(
                        "test-status",
                        "Waiting for the host to answer a request before continuing",
                    )
                    .on_activate(move |_, _| hits.set(hits.get() + 1)),
                ),
                BottomBarItem::flexible_space("test-gap"),
                BottomBarItem::command("test-add", "Add", "test.add")
                    .image(TemplateImage::Add)
                    .style(BottomBarControlStyle::Gradient),
                BottomBarItem::command("test-remove", "Remove", "test.remove")
                    .image(TemplateImage::Remove)
                    .style(BottomBarControlStyle::Gradient),
                BottomBarItem::command("test-info", "Info", "test.info"),
            ],
        )
        .registry(self.registry.clone())
        .size(self.size);
        div().size_full().child(
            WindowFrame::new("Bottom bar test")
                .bottom_bar_component(bar, cx)
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

struct Hits {
    add: Rc<Cell<usize>>,
    status: Rc<Cell<usize>>,
}

fn setup(
    app: &mut TestAppContext,
    size: BottomBarSize,
) -> (Entity<Host>, &mut VisualTestContext, Hits) {
    let add_hits = Rc::new(Cell::new(0));
    let scenario_add_hits = add_hits.clone();
    let status_hits = Rc::new(Cell::new(0));
    let scenario_status_hits = status_hits.clone();
    let (view, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        let add = add_hits.clone();
        registry.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::action("test.add", "Add"),
                    move |_, _, _| add.set(add.get() + 1),
                    cx,
                )
                .expect("unique command");
            registry
                .register(
                    CommandDescriptor::action("test.remove", "Remove").enabled(false),
                    |_, _, _| {},
                    cx,
                )
                .expect("unique command");
            registry
                .register(
                    CommandDescriptor::action("test.info", "Info"),
                    |_, _, _| {},
                    cx,
                )
                .expect("unique command");
        });
        Host {
            registry,
            status_hits: status_hits.clone(),
            size,
        }
    });
    (
        view,
        cx,
        Hits {
            add: scenario_add_hits,
            status: scenario_status_hits,
        },
    )
}

#[test]
fn status_text_truncates_before_controls_shrink() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let (_view, cx, _hits) = setup(&mut app, BottomBarSize::Regular);
    cx.simulate_resize(size(px(640.), px(400.)));
    draw(cx);
    let wide_status = cx
        .debug_bounds("test-status")
        .expect("status value rendered")
        .size
        .width;
    let wide_add = cx
        .debug_bounds("test-add")
        .expect("gradient add rendered")
        .size
        .width;
    let wide_info = cx
        .debug_bounds("test-info")
        .expect("rectangular info rendered")
        .size
        .width;

    cx.simulate_resize(size(px(230.), px(400.)));
    draw(cx);
    let narrow_status = cx
        .debug_bounds("test-status")
        .expect("status value remains")
        .size
        .width;
    let narrow_add = cx
        .debug_bounds("test-add")
        .expect("add remains reachable")
        .size
        .width;
    let narrow_info = cx
        .debug_bounds("test-info")
        .expect("info remains reachable")
        .size
        .width;

    assert!(narrow_status < wide_status, "status text truncates first");
    assert_eq!(narrow_add, wide_add, "gradient control keeps its width");
    assert_eq!(
        narrow_info, wide_info,
        "rectangular control keeps its width"
    );
    let bar = cx.debug_bounds("test-bottom-bar").expect("bottom bar");
    let info = cx.debug_bounds("test-info").expect("info control");
    assert!(
        info.right() <= bar.right() - px(14.),
        "the bar keeps the resize-grip hit region clear"
    );
}

#[test]
fn status_activation_and_disabled_command_are_respected() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let (_view, cx, hits) = setup(&mut app, BottomBarSize::Regular);
    cx.simulate_resize(size(px(640.), px(400.)));
    draw(cx);

    let status = cx.debug_bounds("test-status").expect("status value");
    cx.simulate_click(status.center(), Modifiers::default());
    draw(cx);
    assert_eq!(hits.status.get(), 1, "status activation runs its callback");

    let add = cx.debug_bounds("test-add").expect("add control");
    cx.simulate_click(add.center(), Modifiers::default());
    draw(cx);
    assert_eq!(hits.add.get(), 1, "enabled command dispatches");

    let remove = cx.debug_bounds("test-remove").expect("remove control");
    cx.simulate_click(remove.center(), Modifiers::default());
    draw(cx);
    assert_eq!(hits.add.get(), 1, "disabled command does not dispatch");
}

#[test]
fn small_bar_uses_its_standard_height_and_control_size() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let (_view, cx, _hits) = setup(&mut app, BottomBarSize::Small);
    cx.simulate_resize(size(px(640.), px(400.)));
    draw(cx);

    let bar = cx.debug_bounds("test-bottom-bar").expect("bottom bar");
    assert_eq!(bar.size.height.as_f32(), 22.0);
    let add = cx.debug_bounds("test-add").expect("add control");
    assert_eq!(add.size.height.as_f32(), 19.0);
    let info = cx.debug_bounds("test-info").expect("info control");
    assert_eq!(info.size.height.as_f32(), 18.0);
}
