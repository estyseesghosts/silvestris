//! Theme switching keeps window and view state alive.
//!
//! The core fixture tests cover token isolation; these GPUI tests cover the
//! live-switch path that refreshes windows without rebuilding views.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, Context, Entity, FocusHandle, Focusable, Render, ScrollHandle,
    TestAppContext, Window,
};
use silvestris::command::CommandRegistry;
use silvestris::controls::scroll_source::ScrollSource;
use silvestris::controls::{ScrollAxes, ScrollView, TextField};
use silvestris::core::command::CommandDescriptor;
use silvestris::toolbar::{Toolbar, ToolbarDisplayMode, ToolbarItem, ToolbarLayout};
use silvestris::{ActiveTheme, ThemeId, ThemePreference};

struct Harness {
    focus: FocusHandle,
    counter: usize,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .child(format!("counter {}", self.counter))
    }
}

#[test]
fn a_full_theme_cycle_preserves_the_view_and_its_focus() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let (view, cx) = app.add_window_view(|_, cx| Harness {
        focus: cx.focus_handle(),
        counter: 7,
    });
    cx.update(|window, cx| {
        let focus = view.read(cx).focus.clone();
        window.focus(&focus, cx);
        window.draw(cx).clear(cx);
    });
    let identity = view.entity_id();

    for id in [
        ThemeId::GEIST_DARK,
        ThemeId::GEIST_LIGHT,
        ThemeId::AQUA_DARK,
        ThemeId::AQUA_LIGHT,
    ] {
        cx.update(|_, cx| {
            silvestris::theme::set_theme_preference(ThemePreference::explicit(id), cx)
        });
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.update(|window, cx| {
            assert_eq!(cx.theme().id, id, "theme did not switch");
            assert_eq!(view.entity_id(), identity, "the view was recreated");
            assert_eq!(view.read(cx).counter, 7, "view state was reset");
            assert!(
                view.read(cx).focus.is_focused(window),
                "focus was lost after switching to {id:?}"
            );
        });
    }
}

struct FieldHarness {
    field: gpui::Entity<TextField>,
}

impl Render for FieldHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(200.)).child(self.field.clone())
    }
}

#[test]
fn a_theme_switch_keeps_a_focused_text_field_and_its_contents() {
    let mut app = TestAppContext::single();
    app.update(|cx| {
        silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx);
        silvestris::controls::text_field::bind_keys(cx);
    });
    let (view, cx) = app.add_window_view(|window, cx| {
        let field = cx.new(|cx| TextField::new(window, cx));
        let focus = field.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        FieldHarness { field }
    });
    cx.run_until_parked();
    cx.simulate_input("ABC");
    let field = view.read_with(cx, |view, _| view.field.clone());
    for id in [
        ThemeId::GEIST_DARK,
        ThemeId::GEIST_LIGHT,
        ThemeId::AQUA_LIGHT,
    ] {
        cx.update(|_, cx| {
            silvestris::theme::set_theme_preference(ThemePreference::explicit(id), cx)
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            assert_eq!(field.read(cx).text(), "ABC", "{id:?} lost the field text");
            assert!(
                field.read(cx).focus_handle(cx).is_focused(window),
                "{id:?} lost field focus"
            );
        });
    }
}

struct ScrollHarness {
    scroll: ScrollHandle,
}

impl Render for ScrollHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let rows = (0..40).map(|i| div().h(px(20.)).child(format!("Row {i}")));
        div().size(px(200.)).child(
            ScrollView::new("theme-scroll", &self.scroll)
                .axes(ScrollAxes::Vertical)
                .child(
                    div()
                        .id("theme-scroll-content")
                        .size_full()
                        .flex()
                        .flex_col()
                        .overflow_y_scroll()
                        .track_scroll(&self.scroll)
                        .children(rows),
                ),
        )
    }
}

#[test]
fn a_theme_switch_keeps_the_scroll_position_halfway_through_content() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::GEIST_DARK), cx));
    let (view, cx) = app.add_window_view(|_, _| ScrollHarness {
        scroll: ScrollHandle::new(),
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let scroll = view.read_with(cx, |view, _| view.scroll.clone());
    let max = scroll.max_position().y.as_f32();
    assert!(max > 0.0, "the harness must overflow");
    cx.update(|_, _| {
        silvestris::controls::scroll_source::scroll_to(&scroll, gpui::Axis::Vertical, px(max / 2.));
    });
    let before = scroll.position().y;
    for id in [
        ThemeId::GEIST_LIGHT,
        ThemeId::AQUA_DARK,
        ThemeId::GEIST_DARK,
    ] {
        cx.update(|_, cx| {
            silvestris::theme::set_theme_preference(ThemePreference::explicit(id), cx)
        });
        cx.run_until_parked();
        assert_eq!(
            scroll.position().y,
            before,
            "{id:?} moved the scroll position"
        );
    }
}

struct ToolbarHarness {
    registry: Entity<CommandRegistry>,
    layout: Rc<RefCell<ToolbarLayout>>,
}

impl Render for ToolbarHarness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let items = vec![
            ToolbarItem::command("keep", "Keep", "theme.keep"),
            ToolbarItem::command("opt-a", "Option A", "theme.opt-a").optional(true),
            ToolbarItem::command("opt-b", "Option B", "theme.opt-b").optional(true),
        ];
        div().size(px(480.)).child(
            Toolbar::new(
                "theme-toolbar",
                "Theme toolbar",
                self.registry.clone(),
                items,
            )
            .layout(self.layout.borrow().clone())
            .display_mode(ToolbarDisplayMode::IconAndLabel),
        )
    }
}

#[test]
fn a_theme_switch_keeps_a_modified_toolbar_arrangement() {
    let mut app = TestAppContext::single();
    app.update(|cx| silvestris::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
    let layout = Rc::new(RefCell::new(ToolbarLayout {
        order: vec!["opt-b".into(), "keep".into(), "opt-a".into()],
        hidden: vec!["keep".into()],
        mode: Some(ToolbarDisplayMode::IconOnly),
    }));
    let initial = layout.borrow().clone();
    let host_layout = layout.clone();
    let (_, cx) = app.add_window_view(move |_, cx| {
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            for id in ["theme.keep", "theme.opt-a", "theme.opt-b"] {
                registry
                    .register(CommandDescriptor::action(id, id), |_, _, _| {}, cx)
                    .expect("unique command");
            }
        });
        ToolbarHarness {
            registry,
            layout: host_layout,
        }
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));

    for id in [
        ThemeId::GEIST_DARK,
        ThemeId::GEIST_LIGHT,
        ThemeId::AQUA_DARK,
    ] {
        cx.update(|_, cx| {
            silvestris::theme::set_theme_preference(ThemePreference::explicit(id), cx)
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
            assert_eq!(*layout.borrow(), initial, "{id:?} changed the arrangement");
        });
    }
}

#[test]
fn system_appearance_still_resolves_when_the_preference_follows_it() {
    let app = TestAppContext::single();
    app.update(|cx| {
        silvestris::theme::install(
            ThemePreference {
                family: silvestris::ThemeFamily::Aqua,
                appearance: None,
            },
            cx,
        );
        silvestris::theme::system_appearance_changed(gpui::WindowAppearance::Dark, cx);
    });
    app.update(|cx| {
        assert_eq!(cx.theme().id, ThemeId::AQUA_DARK);
        assert_eq!(
            cx.theme_preference(),
            ThemePreference {
                family: silvestris::ThemeFamily::Aqua,
                appearance: None,
            }
        );
    });
}
