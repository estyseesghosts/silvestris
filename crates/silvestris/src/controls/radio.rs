//! Mutually exclusive choices (HIG pp. 279–283).
//! Tab enters the group once. Unmodified arrows wrap through enabled options.

use std::rc::Rc;

use gpui::{canvas, div, prelude::*, px, App, ElementId, MouseButton, SharedString, Window};

use crate::{convert::hsla, paint, ActiveTheme};

use super::ControlSize;

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
    size: ControlSize,
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
            size: ControlSize::Regular,
            on_select: None,
        }
    }

    /// Regular, small or mini, matching the checkbox and button sizes (HIG
    /// p. 283). Small and mini use scaled bezel and label sizes.
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
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
        let debug_id = self.id.to_string();
        let bezel_prefix = debug_id.clone();
        div()
            .debug_selector(move || debug_id.clone())
            .id(self.id)
            .role(gpui::Role::RadioGroup)
            .aria_label(self.label)
            .flex()
            .flex_col()
            .gap(px(theme.metrics.radio_row_gap))
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
                let radio = &theme.controls.radio;
                let states = radio.states;
                let bevel = if item.disabled {
                    states.disabled
                } else if checked {
                    states.selected
                } else {
                    states.normal
                };
                let dimension = match self.size {
                    ControlSize::Regular => theme.metrics.radio_size[0],
                    ControlSize::Small => theme.metrics.radio_size[1],
                    ControlSize::Mini => theme.metrics.radio_size[2],
                };
                let dot = dimension * theme.metrics.radio_dot_fraction;
                let dot_color = if item.disabled {
                    theme.colors.text_disabled
                } else {
                    radio.dot
                };
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
                    .group("silvestris-radio")
                    .flex()
                    .items_center()
                    .gap(px(theme.metrics.radio_label_gap))
                    .font_family(theme.type_.ui)
                    .text_size(px(self.size.label_size(&theme)))
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
                        div()
                            .id("bezel")
                            .debug_selector({
                                let name = format!("{bezel_prefix}-bezel-{i}");
                                move || name.clone()
                            })
                            .relative()
                            .flex_none()
                            .size(px(dimension))
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |b, _, window, _| {
                                        paint::bevel(
                                            window,
                                            b,
                                            b.size.height.as_f32() / 2.0,
                                            &bevel,
                                        );
                                        if checked {
                                            let inset = (b.size.width.as_f32() - dot) / 2.0;
                                            let dot_bounds = paint::inset(b, px(inset));
                                            window.paint_quad(
                                                gpui::fill(dot_bounds, hsla(dot_color))
                                                    .corner_radii(px(dot / 2.0)),
                                            );
                                        }
                                    },
                                )
                                .absolute()
                                .inset_0(),
                            )
                            .child(
                                div()
                                    .id("pressed")
                                    .absolute()
                                    .inset_0()
                                    .opacity(0.)
                                    .group_active("silvestris-radio", |s| s.opacity(1.))
                                    .child(
                                        canvas(
                                            |_, _, _| (),
                                            move |b, _, window, _| {
                                                paint::bevel(
                                                    window,
                                                    b,
                                                    b.size.height.as_f32() / 2.0,
                                                    &states.pressed,
                                                );
                                                if checked {
                                                    let inset = (b.size.width.as_f32() - dot) / 2.0;
                                                    let dot_bounds = paint::inset(b, px(inset));
                                                    window.paint_quad(
                                                        gpui::fill(dot_bounds, hsla(dot_color))
                                                            .corner_radii(px(dot / 2.0)),
                                                    );
                                                }
                                            },
                                        )
                                        .size_full(),
                                    ),
                            )
                            .child(
                                canvas(
                                    |_, _, _| (),
                                    move |b, _, window, _| {
                                        if !item.disabled
                                            && cursor == Some(i)
                                            && ring_focus.is_focused(window)
                                            && window.last_input_was_keyboard()
                                        {
                                            paint::focus_ring(
                                                window,
                                                b,
                                                b.size.height.as_f32() / 2.0,
                                                ring,
                                                profile,
                                            );
                                        }
                                    },
                                )
                                .absolute()
                                .inset_0(),
                            ),
                    )
                    .child(item.label)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Render, TestAppContext};

    struct Harness(ControlSize);

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            RadioGroup::new(
                "radio-size",
                "Size",
                vec![RadioOption::new("One"), RadioOption::new("Two")],
            )
            .size(self.0)
            .selected(Some(0))
        }
    }

    #[test]
    fn sizes_descend_and_selected_state_renders_in_every_theme() {
        for theme in crate::ThemeId::ALL {
            let mut height = Vec::new();
            for size in [ControlSize::Regular, ControlSize::Small, ControlSize::Mini] {
                let mut app = TestAppContext::single();
                app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(theme), cx));
                let (_, cx) = app.add_window_view(|_, _| Harness(size));
                cx.run_until_parked();
                cx.update(|window, cx| {
                    window.draw(cx).clear(cx);
                });
                height.push(cx.debug_bounds("radio-size-bezel-0").unwrap().size.width);
            }
            assert!(height[0] > height[1] && height[1] > height[2], "{theme:?}");
        }
    }
}
