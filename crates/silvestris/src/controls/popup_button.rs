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
    anchored, canvas, deferred, div, point, prelude::*, px, App, Bounds, ElementId, Entity,
    FocusHandle, KeyDownEvent, MouseButton, Pixels, SharedString, Window,
};
use silvestris_core::tokens::{Bevel, PopUp, Theme};

use crate::controls::ControlSize;
use crate::convert::hsla;
use crate::transient::{self, SurfaceId, TransientKind, TransientLayer};
use crate::{paint, ActiveTheme};

/// Body or capsule/textured pop-up (HIG pp. 257-261; #1 vs #2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopupStyle {
    /// Window-body bezel with a blue arrow cap.
    #[default]
    Rounded,
    /// Unified-toolbar capsule.
    Textured,
}

type ChooseFn = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

#[derive(Default)]
struct State {
    surface: Option<SurfaceId>,
    highlight: usize,
    bounds: Bounds<Pixels>,
    menu_focus: Option<FocusHandle>,
    pressed: bool,
}

#[derive(IntoElement)]
pub struct PopUpButton {
    id: ElementId,
    label: SharedString,
    items: Vec<SharedString>,
    selected: Option<usize>,
    disabled: bool,
    size: ControlSize,
    style: PopupStyle,
    min_width: f32,
    arrow_only: bool,
    on_choose: Option<ChooseFn>,
    layer: Option<Entity<TransientLayer>>,
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
            style: PopupStyle::Rounded,
            min_width: 0.0,
            arrow_only: false,
            on_choose: None,
            layer: None,
        }
    }

    /// Use the window's shared transient layer. Without one the button creates
    /// a private layer, which is enough for a single pop-up but does not
    /// coordinate with a menu elsewhere in the window.
    pub fn layer(mut self, layer: Entity<TransientLayer>) -> Self {
        self.layer = Some(layer);
        self
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
    /// Rounded window-body pop-up or textured toolbar capsule.
    pub fn bezel(mut self, style: PopupStyle) -> Self {
        self.style = style;
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

fn open(
    state: &gpui::Entity<State>,
    layer: &Entity<TransientLayer>,
    start: usize,
    window: &mut Window,
    cx: &mut App,
) {
    let (focus, bounds) = state.update(cx, |s, cx| {
        s.highlight = start;
        let focus = s
            .menu_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        (focus, s.bounds)
    });
    let handle = layer.update(cx, |layer, cx| {
        layer.push(
            crate::transient::SurfaceSpec::new(TransientKind::Popover).anchor(bounds),
            window,
            cx,
        )
    });
    state.update(cx, |s, cx| {
        s.surface = Some(handle.id());
        cx.notify();
    });
    window.focus(&focus, cx);
}

fn close(
    state: &gpui::Entity<State>,
    layer: &Entity<TransientLayer>,
    button: &FocusHandle,
    window: &mut Window,
    cx: &mut App,
) {
    let id = state.update(cx, |s, _| s.surface.take());
    if let Some(id) = id {
        layer.update(cx, |layer, cx| layer.close(id, true, window, cx));
    }
    window.focus(button, cx);
    window.refresh();
}

fn measure(window: &Window, theme: &Theme, size: ControlSize, text: &str) -> f32 {
    let run = gpui::TextRun {
        len: text.len(),
        font: gpui::font(theme.type_.ui),
        color: crate::convert::hsla(theme.colors.text),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.into(), px(size.label_size(theme)), &[run], None)
        .width
        .as_f32()
}

fn dim_toward(mut bevel: Bevel, disabled: Bevel, amount: f32) -> Bevel {
    let mix = |a: silvestris_core::Rgba8, b: silvestris_core::Rgba8| a.mix(b, amount);
    bevel.border = silvestris_core::Gradient::new(
        mix(bevel.border.from, disabled.border.from),
        mix(bevel.border.to, disabled.border.to),
    );
    bevel.upper = silvestris_core::Gradient::new(
        mix(bevel.upper.from, disabled.upper.from),
        mix(bevel.upper.to, disabled.upper.to),
    );
    bevel.lower = silvestris_core::Gradient::new(
        mix(bevel.lower.from, disabled.lower.from),
        mix(bevel.lower.to, disabled.lower.to),
    );
    bevel.emboss = mix(bevel.emboss, disabled.emboss);
    bevel.text = mix(bevel.text, disabled.text);
    bevel
}

impl RenderOnce for PopUpButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let enabled = !self.disabled && !self.items.is_empty();
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| State::default());
        let layer = self.layer.clone().unwrap_or_else(|| {
            window
                .use_keyed_state(
                    ElementId::Name(format!("{}-transient", self.id).into()),
                    cx,
                    |window, cx| cx.new(|cx| TransientLayer::new(window, cx)),
                )
                .read(cx)
                .clone()
        });
        let focus: FocusHandle = window
            .use_keyed_state(
                ElementId::Name(format!("{}-focus", self.id).into()),
                cx,
                |_, cx| cx.focus_handle().tab_stop(true),
            )
            .read(cx)
            .clone();
        let index = match self.size {
            ControlSize::Regular => 0,
            ControlSize::Small => 1,
            ControlSize::Mini => 2,
        };
        let height = match self.size {
            ControlSize::Regular => m.push_height,
            ControlSize::Small => m.push_small_height,
            ControlSize::Mini => m.push_mini_height,
        };
        let text_size = self.size.label_size(&theme);
        let radius = m.push_radius.min(height / 2.0);
        let cap_width = m.popup_cap[index];
        let tokens: PopUp = match self.style {
            PopupStyle::Rounded => theme.controls.popup,
            PopupStyle::Textured => theme.controls.popup_textured,
        };
        let surface = state.read(cx).surface;
        let is_open = enabled && surface.is_some_and(|id| layer.read(cx).is_open(id));
        let is_pressed = state.read(cx).pressed || is_open;
        let inactive = !window.is_window_active();
        let pick = |states: silvestris_core::tokens::BevelStates| {
            let bevel = if !enabled {
                states.disabled
            } else if is_pressed {
                states.pressed
            } else {
                states.normal
            };
            if inactive && enabled {
                dim_toward(bevel, states.disabled, 0.45)
            } else {
                bevel
            }
        };
        let body = pick(tokens.body);
        let cap = pick(tokens.cap);
        let arrow = if !enabled {
            tokens.arrow.mix(body.text, 0.7)
        } else if inactive {
            tokens.arrow.mix(theme.colors.text_disabled, 0.45)
        } else {
            tokens.arrow
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
        // The control is as wide as its longest item, so changing the current
        // item never reflows a toolbar (lotus Settings scope picker).
        let longest = self
            .items
            .iter()
            .map(|item| measure(window, &theme, self.size, item))
            .fold(0.0_f32, f32::max);
        let width = (longest + 8.0 + cap_width + 6.0)
            .max(self.min_width)
            .max(cap_width + 24.0);
        let items = Rc::new(self.items);
        let on_choose = self.on_choose;
        let ink = if inactive && enabled {
            body.text.mix(theme.colors.text_disabled, 0.45)
        } else {
            body.text
        };

        let key_state = state.clone();
        let key_layer = layer.clone();
        let key_items = items.len();
        let mut button = div()
            .debug_selector(|| self.id.to_string())
            .id(self.id.clone())
            .role(gpui::Role::ComboBox)
            .aria_label(format!("{}: {}", self.label, shown))
            .aria_expanded(is_open)
            .occlude()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .h(px(height))
            .when(self.arrow_only, |el| el.w(px(cap_width + 2.0)))
            .when(!self.arrow_only, |el| {
                el.w(px(width)).pl(px(8.0)).pr(px(cap_width + 6.0))
            })
            .text_size(px(text_size))
            .text_color(hsla(ink))
            .child(
                canvas(
                    {
                        let state = state.clone();
                        move |bounds, _, cx| state.update(cx, |s, _| s.bounds = bounds)
                    },
                    move |bounds, _, window, _| {
                        paint::bevel(window, bounds, radius, &body);
                        let cap_bounds = Bounds::new(
                            point(bounds.right() - px(cap_width), bounds.origin.y),
                            gpui::size(px(cap_width), bounds.size.height),
                        );
                        window.with_content_mask(
                            Some(gpui::ContentMask { bounds: cap_bounds }),
                            |window| {
                                paint::bevel(window, bounds, radius, &cap);
                            },
                        );
                        let seam = cap_bounds.origin.x;
                        paint::vline(
                            window,
                            seam - crate::convert::hairline(window) / 2.0,
                            bounds.origin.y + crate::convert::hairline(window),
                            bounds.size.height - crate::convert::hairline(window) * 2.0,
                            tokens.divider,
                        );
                        // Paired triangles, painted as paths so their geometry
                        // is independent of the active font (M: popup-button.regular).
                        let middle = cap_bounds.center().x;
                        let top = cap_bounds.origin.y + cap_bounds.size.height * 0.30;
                        let bottom = cap_bounds.origin.y + cap_bounds.size.height * 0.70;
                        paint::polygon(
                            window,
                            &[
                                point(middle, top - px(2.0)),
                                point(middle - px(3.5), top + px(2.0)),
                                point(middle + px(3.5), top + px(2.0)),
                            ],
                            arrow,
                        );
                        paint::polygon(
                            window,
                            &[
                                point(middle, bottom + px(2.0)),
                                point(middle - px(3.5), bottom - px(2.0)),
                                point(middle + px(3.5), bottom - px(2.0)),
                            ],
                            arrow,
                        );
                        if window.is_window_active()
                            && ring_focus.is_focused(window)
                            && window.last_input_was_keyboard()
                        {
                            paint::focus_ring(window, bounds, radius, ring, profile);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
            .when(!self.arrow_only, |el| {
                el.child(div().relative().truncate().child(shown))
            });
        if enabled {
            let click_state = state.clone();
            let click_layer = layer.clone();
            let mouse_focus = focus.clone();
            let down_state = state.clone();
            let up_state = state.clone();
            button = button
                .track_focus(&focus)
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.focus(&mouse_focus, cx);
                    down_state.update(cx, |s, cx| {
                        s.pressed = true;
                        cx.notify();
                    });
                    cx.stop_propagation();
                })
                .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                    up_state.update(cx, |s, cx| {
                        if s.pressed {
                            s.pressed = false;
                            cx.notify();
                        }
                    });
                    window.refresh();
                })
                .on_click(move |_, window, cx| {
                    click_state.update(cx, |s, cx| {
                        s.pressed = false;
                        cx.notify();
                    });
                    open(&click_state, &click_layer, current, window, cx)
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if matches!(
                        event.keystroke.key.as_str(),
                        "space" | "enter" | "up" | "down"
                    ) && key_items > 0
                    {
                        cx.stop_propagation();
                        open(&key_state, &key_layer, current, window, cx);
                    }
                });
        }
        let menu = is_open.then(|| {
            let s = state.read(cx);
            let menu_focus = s.menu_focus.clone().unwrap_or_else(|| cx.focus_handle());
            let bounds = s.bounds;
            let highlight = s.highlight;
            let row = m.menu_item_height;
            // Current row over the button (10.6 pop-up placement). The frame
            // edge sits one hairline outside the padded body.
            let origin = point(
                bounds.origin.x,
                bounds.origin.y + (bounds.size.height - px(row)) / 2.0
                    - px(m.menu_padding)
                    - px(1.0)
                    - px(row) * current as f32,
            );
            let sel = theme.lists.menu;
            let menu_text = theme.colors.menu_text;
            // Same frame, body and elevation as PopupMenu (one menu material).
            let mut panel = crate::menu::menu_body(&theme, Default::default())
                .min_w(bounds.size.width - px(2.0))
                .text_size(px(theme.type_.size_menu));
            for (index, item) in items.iter().enumerate() {
                let choose = on_choose.clone();
                let row_state = state.clone();
                let row_layer = layer.clone();
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
                            close(&row_state, &row_layer, &row_focus, window, cx);
                            if let Some(f) = &choose {
                                f(&index, window, cx);
                            }
                        })
                        .child(
                            div()
                                .relative()
                                .top(px(m.menu_label_dy))
                                .child(item.clone()),
                        ),
                );
            }
            let key_state = state.clone();
            let key_layer = layer.clone();
            let key_focus = focus.clone();
            let key_choose = on_choose.clone();
            let count = items.len();
            let shield = transient::shield(
                layer,
                surface.expect("checked above"),
                window,
                point(-bounds.origin.x, -bounds.origin.y),
            )
            .track_focus(&menu_focus)
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
                    "escape" => {
                        if key_layer.update(cx, |layer, cx| layer.escape(window, cx)) {
                            key_state.update(cx, |s, _| s.surface = None);
                        }
                    }
                    "tab" => close(&key_state, &key_layer, &key_focus, window, cx),
                    _ => {}
                }
                if let Some(index) = chosen {
                    close(&key_state, &key_layer, &key_focus, window, cx);
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
                    .child(
                        crate::menu::menu_frame(&theme, Default::default())
                            .id("popup-panel")
                            .debug_selector(|| "popup-panel".into())
                            .role(gpui::Role::ListBox)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(panel),
                    ),
            );
            deferred(shield).with_priority(2)
        });
        div().relative().flex_none().child(button).children(menu)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{Modifiers, TestAppContext, VisualTestContext};

    use super::*;
    use crate::transient::TransientLayer;

    struct Harness {
        chosen: Rc<Cell<Option<usize>>>,
        background_hits: Rc<Cell<usize>>,
        layer: Entity<TransientLayer>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let chosen = self.chosen.clone();
            let hits = self.background_hits.clone();
            div()
                .size_full()
                .track_focus(&cx.focus_handle())
                .on_mouse_down(MouseButton::Left, move |_, _, _| hits.set(hits.get() + 1))
                .child(
                    div().absolute().top(px(40.)).left(px(40.)).child(
                        PopUpButton::new(
                            "popup-test",
                            "Choice",
                            vec!["One".into(), "Two".into(), "Three".into()],
                        )
                        .layer(self.layer.clone())
                        .on_choose(move |index, _, _| chosen.set(Some(*index))),
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

    fn harness(app: &mut TestAppContext) -> (Entity<Harness>, &mut VisualTestContext) {
        let chosen = Rc::new(Cell::new(None));
        let background_hits = Rc::new(Cell::new(0));
        app.add_window_view(|window, cx| {
            let layer = cx.new(|cx| TransientLayer::new(window, cx));
            Harness {
                chosen,
                background_hits,
                layer,
            }
        })
    }

    #[test]
    fn escape_and_outside_click_close_the_list_through_the_layer() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (handle, cx) = harness(&mut app);
        draw(cx);

        // Escape closes the list and returns focus to the button.
        let button = cx.debug_bounds("popup-test").expect("popup button");
        cx.simulate_click(button.center(), Modifiers::default());
        draw(cx);
        assert!(cx.debug_bounds("popup-panel").is_some());
        cx.simulate_keystrokes("escape");
        draw(cx);
        assert!(cx.debug_bounds("popup-panel").is_none());
        // An outside click dismisses and does not reach the background.
        let button = cx.debug_bounds("popup-test").expect("popup button");
        cx.simulate_click(button.center(), Modifiers::default());
        draw(cx);
        assert!(cx.debug_bounds("popup-panel").is_some());
        cx.simulate_click(point(px(500.), px(400.)), Modifiers::default());
        draw(cx);
        assert!(cx.debug_bounds("popup-panel").is_none());
        assert_eq!(
            handle.read_with(cx, |view, _| view.background_hits.get()),
            0,
            "the shield consumes the outside click"
        );

        // Choosing a row closes and reports the index.
        let button = cx.debug_bounds("popup-test").expect("popup button");
        cx.simulate_click(button.center(), Modifiers::default());
        draw(cx);
        let row = cx.debug_bounds("popup-item-1").expect("second row");
        cx.simulate_click(row.center(), Modifiers::default());
        draw(cx);
        assert!(cx.debug_bounds("popup-panel").is_none());
        assert_eq!(handle.read_with(cx, |view, _| view.chosen.get()), Some(1));
    }

    #[test]
    fn the_list_is_a_popover_on_the_shared_layer() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (handle, cx) = harness(&mut app);
        draw(cx);
        let button = cx.debug_bounds("popup-test").expect("popup button");
        cx.simulate_click(button.center(), Modifiers::default());
        draw(cx);
        let (depth, kind) = handle.read_with(cx, |view, cx| {
            (view.layer.read(cx).len(), view.layer.read(cx).top_kind())
        });
        assert_eq!(depth, 1);
        assert_eq!(kind, Some(crate::transient::TransientKind::Popover));
    }

    #[test]
    fn width_does_not_change_with_the_selected_item_and_textured_renders() {
        struct PopupHarness {
            selected: usize,
            textured: bool,
        }
        impl Render for PopupHarness {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().child(
                    PopUpButton::new(
                        "popup-stable",
                        "Choice",
                        vec!["One".into(), "A much longer second choice".into()],
                    )
                    .selected(Some(self.selected))
                    .size(ControlSize::Regular)
                    .bezel(if self.textured {
                        PopupStyle::Textured
                    } else {
                        PopupStyle::Rounded
                    }),
                )
            }
        }
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|_, _| PopupHarness {
            selected: 0,
            textured: false,
        });
        draw(cx);
        let first = cx.debug_bounds("popup-stable").unwrap();
        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.selected = 1;
                cx.notify();
            })
        });
        draw(cx);
        let second = cx.debug_bounds("popup-stable").unwrap();
        assert_eq!(first.size.width, second.size.width);
        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.textured = true;
                cx.notify();
            })
        });
        draw(cx);
        assert!(cx.debug_bounds("popup-stable").is_some());
    }
}
