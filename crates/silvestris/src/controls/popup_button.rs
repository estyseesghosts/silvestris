//! Pop-up button (HIG pp. 279-281): a button showing the current choice that
//! opens a menu of mutually exclusive choices.
//!
//! - The open menu places the current item over the button, as 10.6 does;
//!   GPUI `anchored` snaps it inside the window when that would overflow.
//! - Mouse: click opens; click a row chooses; click outside dismisses.
//! - Keyboard (focused): Space, Return, Up or Down opens. While open, Up/Down
//!   move the highlight, Return/Space choose, Escape/Tab dismiss. Focus
//!   returns to the button.
//! - Disabled buttons are not focusable and do not open.
//!
//! Body uses the push-button bevel; the right cap uses the default-button
//! bevel with an up/down arrow pair (enabled 10.6 pop-ups are blue-capped;
//! M: popup.disabled/popup.disabled.right measure only the disabled state, so
//! the enabled cap colour is an estimate). Rows use the menu tokens.
use std::rc::Rc;

use gpui::{
    anchored, canvas, deferred, div, point, prelude::*, px, App, Bounds, ElementId, FocusHandle,
    KeyDownEvent, MouseButton, Pixels, SharedString, Window,
};

use crate::controls::ControlSize;
use crate::convert::hsla;
use crate::{paint, ActiveTheme};

/// Width of the arrow cap (estimate from the disabled capture's right cap).
const CAP: f32 = 18.0;

type ChooseFn = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

#[derive(Default)]
struct State {
    open: bool,
    highlight: usize,
    bounds: Bounds<Pixels>,
    menu_focus: Option<FocusHandle>,
}

#[derive(IntoElement)]
pub struct PopUpButton {
    id: ElementId,
    label: SharedString,
    items: Vec<SharedString>,
    selected: Option<usize>,
    disabled: bool,
    size: ControlSize,
    min_width: f32,
    arrow_only: bool,
    on_choose: Option<ChooseFn>,
}

impl PopUpButton {
    /// `label` is the accessible name; the button shows the selected item.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        items: Vec<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items,
            selected: None,
            disabled: false,
            size: ControlSize::Small,
            min_width: 0.0,
            arrow_only: false,
            on_choose: None,
        }
    }
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }
    pub(super) fn arrow_only(mut self) -> Self {
        self.arrow_only = true;
        self
    }

    pub fn on_choose(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_choose = Some(Rc::new(f));
        self
    }
}

fn open(state: &gpui::Entity<State>, start: usize, window: &mut Window, cx: &mut App) {
    let focus = state.update(cx, |s, cx| {
        s.open = true;
        s.highlight = start;
        s.menu_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone()
    });
    window.focus(&focus, cx);
}

fn close(state: &gpui::Entity<State>, button: &FocusHandle, window: &mut Window, cx: &mut App) {
    state.update(cx, |s, _| s.open = false);
    window.focus(button, cx);
    window.refresh();
}

impl RenderOnce for PopUpButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let enabled = !self.disabled && !self.items.is_empty();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        let focus: FocusHandle = window
            .use_keyed_state(
                ElementId::Name(format!("{}-focus", self.id).into()),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let height = match self.size {
            ControlSize::Regular => m.push_height,
            ControlSize::Small => m.push_small_height,
            ControlSize::Mini => m.push_mini_height,
        };
        let text_size = self.size.label_size(&theme);
        let radius = m.push_radius.min(height / 2.0);
        let body = if enabled {
            theme.controls.push.normal
        } else {
            theme.controls.push.disabled
        };
        let cap = if enabled {
            theme.controls.push_default
        } else {
            theme.controls.push.disabled
        };
        let current = self
            .selected
            .unwrap_or(0)
            .min(self.items.len().saturating_sub(1));
        let shown = self
            .selected
            .and_then(|i| self.items.get(i).cloned())
            .unwrap_or_default();
        let ring_focus = focus.clone();
        let ring = theme.colors.focus_ring;
        let profile = m.focus_ring;
        let items = Rc::new(self.items);
        let on_choose = self.on_choose;
        let is_open = state.read(cx).open && enabled;

        let key_state = state.clone();
        let key_items = items.len();
        let mut button = div()
            .debug_selector(|| self.id.to_string())
            .id(self.id.clone())
            .role(gpui::Role::ComboBox)
            .aria_label(format!("{}: {}", self.label, shown))
            .occlude()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(height))
            .when(self.arrow_only, |el| el.w(px(CAP + 2.0)))
            .when(!self.arrow_only, |el| {
                el.min_w(px(self.min_width.max(CAP + 24.0)))
                    .pl(px(8.0))
                    .pr(px(CAP + 6.0))
            })
            .text_size(px(text_size))
            .text_color(hsla(body.text))
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| state.update(cx, |s, _| s.bounds = bounds)
                    },
                    move |bounds, _, window, _| {
                        paint::bevel(window, bounds, radius, &body);
                        let cap_bounds = Bounds::new(
                            point(bounds.right() - px(CAP), bounds.origin.y),
                            gpui::size(px(CAP), bounds.size.height),
                        );
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: cap_bounds }),
                            |window| {
                                paint::bevel(window, bounds, radius, &cap);
                            },
                        );
                        if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                            paint::focus_ring(window, bounds, radius, ring, profile);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when(!self.arrow_only, |el| {
                el.child(div().relative().truncate().child(shown))
            })
            .child(
                // Up/down arrow pair (U+25B4/U+25BE small triangles).
                div()
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    .w(px(CAP))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .text_size(px(7.0))
                    .line_height(px(6.0))
                    .text_color(hsla(cap.text))
                    .child("\u{25B2}")
                    .child("\u{25BC}"),
            );
        if enabled {
            let click_state = state.clone();
            let mouse_focus = focus.clone();
            button = button
                .track_focus(&focus)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.focus(&mouse_focus, cx);
                    cx.stop_propagation();
                })
                .on_click(move |_, window, cx| open(&click_state, current, window, cx))
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(
                        event.keystroke.key.as_str(),
                        "space" | "enter" | "up" | "down"
                    ) && key_items > 0
                    {
                        cx.stop_propagation();
                        open(&key_state, current, window, cx);
                    }
                });
        }
        let menu = is_open.then(|| {
            let s = state.read(cx);
            let menu_focus = s.menu_focus.clone().unwrap_or_else(|| cx.focus_handle());
            let bounds = s.bounds;
            let highlight = s.highlight;
            let row = m.menu_item_height;
            // Current row over the button (10.6 pop-up placement).
            let origin = point(
                bounds.origin.x,
                bounds.origin.y + (bounds.size.height - px(row)) / 2.0
                    - px(m.menu_padding)
                    - px(row) * current as f32,
            );
            let sel = theme.lists.menu;
            let menu_text = theme.colors.menu_text;
            let mut panel = div()
                .id("popup-panel")
                .debug_selector(|| "popup-panel".into())
                .role(gpui::Role::ListBox)
                .flex()
                .flex_col()
                .min_w(bounds.size.width)
                .py(px(m.menu_padding))
                .bg(hsla(theme.colors.menu))
                .rounded(px(m.menu_radius))
                .shadow_lg()
                .text_size(px(theme.type_.size_menu))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());
            for (index, item) in items.iter().enumerate() {
                let choose = on_choose.clone();
                let row_state = state.clone();
                let row_focus = focus.clone();
                let hover_state = state.clone();
                let hot = index == highlight;
                panel = panel.child(
                    div()
                        .id(("popup-item", index))
                        .debug_selector(move || format!("popup-item-{index}"))
                        .role(gpui::Role::ListBoxOption)
                        .relative()
                        .h(px(row))
                        .flex()
                        .items_center()
                        .pl(px(20.0))
                        .pr(px(16.0))
                        .text_color(hsla(if hot { sel.text } else { menu_text }))
                        .when(hot, |el| {
                            el.child(
                                canvas(
                                    |_, _, _| (),
                                    move |b, _, window, _| paint::selection(window, b, &sel),
                                )
                                .absolute()
                                .inset_0(),
                            )
                        })
                        .when(index == current, |el| {
                            el.child(div().absolute().left(px(6.0)).child("\u{2713}"))
                        })
                        .on_mouse_move(move |_, _, cx| {
                            hover_state.update(cx, |s, cx| {
                                if s.highlight != index {
                                    s.highlight = index;
                                    cx.notify();
                                }
                            })
                        })
                        .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                            close(&row_state, &row_focus, window, cx);
                            if let Some(f) = &choose {
                                f(&index, window, cx);
                            }
                        })
                        .child(div().relative().child(item.clone())),
                );
            }
            let shield_state = state.clone();
            let shield_focus = focus.clone();
            let key_state = state.clone();
            let key_focus = focus.clone();
            let key_choose = on_choose.clone();
            let count = items.len();
            deferred(
                div()
                    .id("popup-shield")
                    .debug_selector(|| "popup-shield".into())
                    .absolute()
                    .top(-bounds.origin.y)
                    .left(-bounds.origin.x)
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .occlude()
                    .track_focus(&menu_focus)
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        cx.stop_propagation();
                        close(&shield_state, &shield_focus, window, cx);
                    })
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        cx.stop_propagation();
                        let mut chosen = None;
                        match event.keystroke.key.as_str() {
                            "up" => key_state.update(cx, |s, cx| {
                                s.highlight = s.highlight.saturating_sub(1);
                                cx.notify();
                            }),
                            "down" => key_state.update(cx, |s, cx| {
                                s.highlight = (s.highlight + 1).min(count - 1);
                                cx.notify();
                            }),
                            "enter" | "space" => chosen = Some(key_state.read(cx).highlight),
                            "escape" | "tab" => close(&key_state, &key_focus, window, cx),
                            _ => {}
                        }
                        if let Some(index) = chosen {
                            close(&key_state, &key_focus, window, cx);
                            if let Some(f) = &key_choose {
                                f(&index, window, cx);
                            }
                        }
                        window.refresh();
                    })
                    .child(
                        anchored()
                            .position(origin)
                            .snap_to_window_with_margin(px(4.))
                            .child(panel),
                    ),
            )
            .with_priority(2)
        });
        div().relative().flex_none().child(button).children(menu)
    }
}
