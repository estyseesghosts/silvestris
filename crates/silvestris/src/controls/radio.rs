//! Mutually exclusive choices (HIG pp. 279–283).
//! Tab enters the group once. Unmodified arrows wrap through enabled options.

use std::rc::Rc;

use gpui::{canvas, div, prelude::*, px, App, ElementId, MouseButton, SharedString, Window};

use crate::{convert::hsla, paint, ActiveTheme};

pub struct RadioOption {
    pub label: SharedString,
    pub disabled: bool,
}

impl RadioOption {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            disabled: false,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

type Select = Rc<dyn Fn(&usize, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct RadioGroup {
    id: ElementId,
    label: SharedString,
    items: Vec<RadioOption>,
    selected: Option<usize>,
    on_select: Option<Select>,
}

impl RadioGroup {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        items: Vec<RadioOption>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items,
            selected: None,
            on_select: None,
        }
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    pub fn on_select(mut self, f: impl Fn(&usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for RadioGroup {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let enabled: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| (!item.disabled).then_some(i))
            .collect();
        let active = enabled.iter().position(|i| Some(*i) == self.selected);
        let cursor = active
            .map(|i| enabled[i])
            .or_else(|| enabled.first().copied());
        let key_select = self.on_select.clone();
        let selected = self.selected;
        div()
            .id(self.id)
            .role(gpui::Role::RadioGroup)
            .aria_label(self.label)
            .flex()
            .flex_col()
            .gap(px(4.0))
            .when(!enabled.is_empty(), |el| {
                el.track_focus(&focus)
                    .on_key_down(move |event, window, cx| {
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        let next = match event.keystroke.key.as_str() {
                            "right" | "down" => {
                                active.map(|i| (i + 1) % enabled.len()).unwrap_or(0)
                            }
                            "left" | "up" => active
                                .map(|i| (i + enabled.len() - 1) % enabled.len())
                                .unwrap_or(enabled.len() - 1),
                            "space" | "enter" => active.unwrap_or(0),
                            _ => return,
                        };
                        cx.stop_propagation();
                        if let Some(select) = &key_select {
                            select(&enabled[next], window, cx);
                        }
                    })
            })
            .children(self.items.into_iter().enumerate().map(|(i, item)| {
                let checked = selected == Some(i);
                let states = theme.controls.checkbox;
                let bevel = if item.disabled {
                    states.disabled
                } else if checked {
                    states.selected
                } else {
                    states.normal
                };
                let dimension = theme.metrics.checkbox_size;
                let color = if item.disabled {
                    theme.colors.text_disabled
                } else {
                    theme.colors.text
                };
                let pointer_focus = focus.clone();
                let ring_focus = focus.clone();
                let ring = theme.colors.focus_ring;
                let profile = theme.metrics.focus_ring;
                let select = self.on_select.clone();
                div()
                    .id(("radio", i))
                    .role(gpui::Role::RadioButton)
                    .aria_label(item.label.clone())
                    .aria_toggled(if checked {
                        gpui::Toggled::True
                    } else {
                        gpui::Toggled::False
                    })
                    .occlude()
                    .flex()
                    .items_center()
                    .gap(px(theme.metrics.checkbox_label_gap))
                    .font_family(theme.type_.ui)
                    .text_color(hsla(color))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        if !item.disabled {
                            window.focus(&pointer_focus, cx);
                        }
                        cx.stop_propagation();
                    })
                    .when(!item.disabled, |el| {
                        el.on_click(move |_, window, cx| {
                            if let Some(select) = &select {
                                select(&i, window, cx);
                            }
                        })
                    })
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                paint::bevel(window, bounds, dimension / 2.0, &bevel);
                                if checked {
                                    let dot = paint::inset(bounds, px(dimension * 0.32));
                                    window.paint_quad(
                                        gpui::fill(dot, hsla(bevel.text))
                                            .corner_radii(px(dimension)),
                                    );
                                }
                                if cursor == Some(i)
                                    && ring_focus.is_focused(window)
                                    && window.last_input_was_keyboard()
                                {
                                    paint::focus_ring(
                                        window,
                                        bounds,
                                        dimension / 2.0,
                                        ring,
                                        profile,
                                    );
                                }
                            },
                        )
                        .size(px(dimension)),
                    )
                    .child(item.label)
            }))
    }
}
