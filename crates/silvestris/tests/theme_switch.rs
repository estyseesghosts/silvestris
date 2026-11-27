//! Theme switching keeps window and view state alive.
//!
//! The core fixture tests cover token isolation; these GPUI tests cover the
//! live-switch path that refreshes windows without rebuilding views.

use gpui::{div, prelude::*, Context, FocusHandle, Render, TestAppContext, Window};
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
