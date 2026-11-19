//! Disclosure triangle (2009 HIG pp. 329-331; issue #128).
//! Consumer owns expansion and label; activation requests expansion only.
//! Enter/Space use GPUI's keyboard click path; Left/Right collapse/expand.
use std::rc::Rc;

use crate::{convert::hsla, paint, ActiveTheme};
use gpui::{canvas, div, point, prelude::*, px, App, ElementId, PathBuilder, SharedString, Window};

type ToggleHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct Disclosure {
    id: ElementId,
    label: SharedString,
    expanded: bool,
    disabled: bool,
    on_toggle: Option<ToggleHandler>,
}

impl Disclosure {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            expanded: false,
            disabled: false,
            on_toggle: None,
        }
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Requests the new expansion state. Does not select a containing row.
    pub fn on_toggle(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Disclosure {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let ring_focus = focus.clone();
        let mouse_focus = focus.clone();
        let expanded = self.expanded;
        let enabled = !self.disabled;
        let color = if enabled {
            theme.colors.text_secondary
        } else {
            theme.colors.text_disabled
        };
        let ring = theme.colors.focus_ring;
        let profile = theme.metrics.focus_ring;
        let click = self.on_toggle.clone();
        let arrow = self.on_toggle;
        div()
            .id(self.id)
            .role(gpui::Role::Button)
            .aria_label(self.label)
            .aria_expanded(expanded)
            .relative()
            .flex_none()
            .size(px(theme.metrics.disclosure_size))
            .when(enabled, |el| {
                el.track_focus(&focus).on_mouse_down(
                    gpui::MouseButton::Left,
                    move |_, window, cx| {
                        window.focus(&mouse_focus, cx);
                        cx.stop_propagation();
                    },
                )
            })
            .when_some(click.filter(|_| enabled), |el, handler| {
                el.on_click(move |_, window, cx| {
                    cx.stop_propagation();
                    handler(&!expanded, window, cx);
                })
            })
            .when_some(arrow.filter(|_| enabled), |el, handler| {
                el.on_key_down(move |event, window, cx| {
                    if event.keystroke.modifiers.modified() {
                        return;
                    }
                    let next = match event.keystroke.key.as_str() {
                        "left" => false,
                        "right" => true,
                        _ => return,
                    };
                    cx.stop_propagation();
                    if next != expanded {
                        handler(&next, window, cx);
                    }
                })
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        // Estimated inset silhouette; HIG specifies the 13 px control
                        // bounds and direction, but no usable close-up of this state.
                        let vertices = if expanded {
                            [(0.18, 0.32), (0.82, 0.32), (0.50, 0.72)]
                        } else {
                            [(0.32, 0.18), (0.72, 0.50), (0.32, 0.82)]
                        };
                        let at = |(x, y)| {
                            point(
                                bounds.origin.x + bounds.size.width * x,
                                bounds.origin.y + bounds.size.height * y,
                            )
                        };
                        let mut path = PathBuilder::fill();
                        path.move_to(at(vertices[0]));
                        path.line_to(at(vertices[1]));
                        path.line_to(at(vertices[2]));
                        path.close();
                        if let Ok(path) = path.build() {
                            window.paint_path(path, hsla(color));
                        }
                        if enabled
                            && ring_focus.is_focused(window)
                            && window.last_input_was_keyboard()
                        {
                            paint::focus_ring(window, bounds, 2.0, ring, profile);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}
