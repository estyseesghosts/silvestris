//! Shortcut recorder (#66): a field that captures a key chord.
//!
//! Drop it in a preferences pane, give it the current chord and an
//! `on_record` callback. Clicking starts recording; the next chord is reported
//! in the same spelling `CommandRegistry::rebind` accepts. Escape cancels,
//! Delete or Backspace clears, and a modifier-only press is ignored. While the
//! recorder is not recording it does not intercept anything, so a chord typed
//! into a focused text field still reaches the field.

use std::rc::Rc;

use gpui::{
    div, prelude::*, px, App, ElementId, FocusHandle, KeyDownEvent, MouseButton, SharedString,
    Window,
};
use silvestris_core::command::{display_shortcut, Platform};

use crate::{convert::hsla, paint, ActiveTheme};

type Record = Rc<dyn Fn(Option<SharedString>, &mut Window, &mut App)>;

#[derive(Default)]
struct State {
    recording: bool,
}

#[derive(IntoElement)]
pub struct ShortcutRecorder {
    id: ElementId,
    label: SharedString,
    shortcut: Option<SharedString>,
    focus: Option<FocusHandle>,
    on_record: Option<Record>,
    disabled: bool,
}

impl ShortcutRecorder {
    /// `label` is the accessible name. `shortcut` is the current binding in
    /// GPUI spelling (`"secondary-k"`), or `None` for unbound.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            focus: None,
            on_record: None,
            disabled: false,
        }
    }

    pub fn shortcut(mut self, shortcut: Option<SharedString>) -> Self {
        self.shortcut = shortcut;
        self
    }

    pub fn focus_handle(mut self, focus: FocusHandle) -> Self {
        self.focus = Some(focus);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Called with the captured chord, or `None` when the user clears it.
    pub fn on_record(
        mut self,
        f: impl Fn(Option<SharedString>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_record = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for ShortcutRecorder {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let field = theme.controls.field;
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        let focus = self.focus.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(
                    ElementId::Name(format!("{}-focus", self.id).into()),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone()
        });
        let recording = state.read(cx).recording;
        let shown = self
            .shortcut
            .as_deref()
            .map(|shortcut| display_shortcut(shortcut, Platform::current()))
            .unwrap_or_else(|| "—".into());
        let text = if recording {
            "Type a shortcut…".to_string()
        } else {
            shown.clone()
        };
        let color = if self.disabled {
            theme.colors.text_disabled
        } else if recording {
            field.caret
        } else if self.shortcut.is_some() {
            field.text
        } else {
            field.placeholder
        };
        let focus_for_ring = focus.clone();
        let focus_color = theme.colors.focus_ring;
        let focus_profile = m.focus_ring;
        let input_focus = focus.clone();
        let record_for_keys = self.on_record;
        let state_for_click = state.clone();
        let state_for_keys = state.clone();
        let enabled = !self.disabled;
        let id = self.id.clone();
        let selector = format!("shortcut-recorder-{}", self.label);

        div()
            .id(id)
            .debug_selector(move || selector.clone())
            .role(gpui::Role::Button)
            .aria_label(format!("{}: {shown}", self.label))
            .aria_description(if recording {
                "Recording a shortcut. Escape cancels, Delete clears."
            } else {
                "Click and press a key chord to record it."
            })
            .track_focus(&focus)
            .flex()
            .items_center()
            .h(px(m.field_height))
            .px(px(6.))
            .rounded(px(0.0))
            .text_size(px(theme.type_.size_regular))
            .text_color(hsla(color))
            .cursor(gpui::CursorStyle::PointingHand)
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !enabled {
                    return;
                }
                cx.stop_propagation();
                window.focus(&input_focus, cx);
                state_for_click.update(cx, |state, cx| {
                    state.recording = true;
                    cx.notify();
                });
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if !state_for_keys.read(cx).recording {
                    return;
                }
                cx.stop_propagation();
                let key = event.keystroke.key.as_str();
                match key {
                    "escape" => {
                        state_for_keys.update(cx, |state, cx| {
                            state.recording = false;
                            cx.notify();
                        });
                    }
                    "backspace" | "delete" => {
                        state_for_keys.update(cx, |state, cx| {
                            state.recording = false;
                            cx.notify();
                        });
                        if let Some(record) = &record_for_keys {
                            record(None, window, cx);
                        }
                    }
                    "shift" | "control" | "alt" | "platform" | "fn" => {}
                    _ => {
                        let chord = event.keystroke.unparse(cx.platform_info().os);
                        state_for_keys.update(cx, |state, cx| {
                            state.recording = false;
                            cx.notify();
                        });
                        if let Some(record) = &record_for_keys {
                            record(Some(chord.into()), window, cx);
                        }
                    }
                }
            })
            .child(
                gpui::canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        paint::field(window, bounds, 0.0, &field, enabled);
                        if focus_for_ring.is_focused(window) {
                            paint::focus_ring(window, bounds, 0.0, focus_color, focus_profile);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .relative()
            .child(div().relative().truncate().child(text))
            .child(
                div()
                    .relative()
                    .ml_auto()
                    .text_size(px(theme.type_.size_small))
                    .text_color(hsla(theme.colors.text_secondary))
                    .child(self.label),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui::{Focusable, Modifiers, TestAppContext, VisualTestContext};

    use super::*;
    use crate::controls::TextField;

    struct Harness {
        field: gpui::Entity<TextField>,
        recorded: Rc<RefCell<Vec<Option<String>>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let recorded = self.recorded.clone();
            div()
                .size_full()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(div().w(px(200.)).child(self.field.clone()))
                .child(
                    div().w(px(240.)).child(
                        ShortcutRecorder::new("recorder", "Search shortcut")
                            .shortcut(Some("secondary-f".into()))
                            .on_record(move |chord, _, _| {
                                recorded.borrow_mut().push(chord.map(|c| c.to_string()))
                            }),
                    ),
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

    fn harness(app: &mut TestAppContext) -> (gpui::Entity<Harness>, &mut VisualTestContext) {
        app.add_window_view(|window, cx| {
            let field = cx.new(|cx| TextField::new(window, cx));
            Harness {
                field,
                recorded: Rc::new(RefCell::new(Vec::new())),
            }
        })
    }

    #[test]
    fn a_clicked_recorder_captures_one_chord_and_escape_cancels() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (view, cx) = harness(&mut app);
        draw(cx);

        let bounds = cx
            .debug_bounds("shortcut-recorder-Search shortcut")
            .expect("recorder");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        cx.simulate_keystrokes("secondary-k");
        draw(cx);
        let expected = if cfg!(target_os = "macos") {
            "cmd-k"
        } else {
            "ctrl-k"
        };
        assert_eq!(
            view.read_with(cx, |view, _| view.recorded.borrow().clone()),
            vec![Some(expected.to_string())]
        );

        let bounds = cx
            .debug_bounds("shortcut-recorder-Search shortcut")
            .expect("recorder");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        cx.simulate_keystrokes("escape");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, _| view.recorded.borrow().len()),
            1,
            "Escape records nothing"
        );

        let bounds = cx
            .debug_bounds("shortcut-recorder-Search shortcut")
            .expect("recorder");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        cx.simulate_keystrokes("delete");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, _| view.recorded.borrow().clone()),
            vec![Some(expected.to_string()), None]
        );
    }

    #[test]
    fn typing_into_a_focused_text_field_does_not_reach_the_recorder() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (view, cx) = harness(&mut app);
        draw(cx);
        let focus = view.read_with(cx, |view, cx| view.field.read(cx).focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        draw(cx);
        cx.simulate_input("aq");
        draw(cx);
        assert_eq!(
            view.read_with(cx, |view, cx| view.field.read(cx).text().to_string()),
            "aq"
        );
        assert!(
            view.read_with(cx, |view, _| view.recorded.borrow().is_empty()),
            "a focused field keeps its keystrokes"
        );
    }
}
