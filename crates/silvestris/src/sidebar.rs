//! Persistent sidebar and divider (#130, handbook 13).
//! The consumer owns preferences and supplies expanded and rail content.
//! Native GPUI drag routing keeps the divider active outside its hitbox.
use crate::{convert::hsla, ActiveTheme};
use gpui::{div, prelude::*, px, AnyElement, App, ElementId, MouseButton, Pixels, Window};
use silvestris_core::layout::{self, Divider, ShellGeometry, SidebarPreference};
use std::{cell::Cell, rc::Rc};

type ChangeHandler = Rc<dyn Fn(&SidebarPreference, &mut Window, &mut App)>;

#[derive(Clone)]
struct SidebarState {
    focus: gpui::FocusHandle,
    start_x: Rc<Cell<Pixels>>,
}

#[derive(Clone)]
struct SidebarDrag {
    owner: ElementId,
    start_x: Rc<Cell<Pixels>>,
    width: f32,
    preference: SidebarPreference,
}

#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    preference: SidebarPreference,
    geometry: ShellGeometry,
    effective: Option<(f32, bool)>,
    content: AnyElement,
    rail: AnyElement,
    footer: Option<AnyElement>,
    on_change: Option<ChangeHandler>,
}

impl Sidebar {
    pub fn new(
        id: impl Into<ElementId>,
        preference: SidebarPreference,
        content: impl IntoElement,
        rail: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            preference,
            geometry: ShellGeometry::LOTUS_LEGACY,
            effective: None,
            content: content.into_any_element(),
            rail: rail.into_any_element(),
            footer: None,
            on_change: None,
        }
    }

    pub fn geometry(mut self, geometry: ShellGeometry) -> Self {
        self.geometry = geometry;
        self
    }

    /// Feed ShellPlan's effective presentation without changing saved preference.
    pub fn effective(mut self, width: f32, expanded: bool) -> Self {
        self.effective = Some((width, expanded));
        self
    }

    pub fn footer(mut self, footer: impl IntoElement) -> Self {
        self.footer = Some(footer.into_any_element());
        self
    }

    pub fn on_change(
        mut self,
        f: impl Fn(&SidebarPreference, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let g = self.geometry;
        let pref = self.preference;
        let (width, expanded) = self.effective.unwrap_or((
            if pref.expanded {
                pref.width.clamp(g.sidebar_min, g.sidebar_max)
            } else {
                g.sidebar_rail
            },
            pref.expanded,
        ));
        let state = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| SidebarState {
                focus: cx.focus_handle().tab_stop(true),
                start_x: Rc::new(Cell::new(px(0.))),
            })
            .read(cx)
            .clone();
        let focus = state.focus;
        let start_x = state.start_x;
        let down_x = start_x.clone();
        let drag = SidebarDrag {
            owner: self.id.clone(),
            start_x,
            width,
            preference: pref,
        };
        let owner = self.id.clone();
        let change = self.on_change.clone();
        let keyboard = self.on_change;
        let focus_down = focus.clone();
        let ring_focus = focus.clone();
        let ring = theme.colors.focus_ring;
        let profile = theme.metrics.focus_ring;
        div()
            .id(self.id)
            .relative()
            .flex_none()
            .w(px(width.max(0.)))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(hsla(if window.is_window_active() {
                theme.colors.sidebar
            } else {
                theme.colors.sidebar_inactive
            }))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(if expanded { self.content } else { self.rail }),
            )
            .when_some(self.footer, |el, footer| {
                el.child(div().flex_none().child(footer))
            })
            .child(
                div()
                    .id("divider")
                    .debug_selector(|| "sidebar-divider".into())
                    .role(gpui::Role::Splitter)
                    .aria_label("Sidebar width")
                    .track_focus(&focus)
                    .absolute()
                    .right_0()
                    .top_0()
                    .bottom_0()
                    // Four logical px is a designed minimum target; visible seam is one px.
                    .w(px(theme.metrics.splitter_hot_zone.max(4.)))
                    .cursor(gpui::CursorStyle::ResizeLeftRight)
                    .child(
                        gpui::canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                let divider = theme.colors.divider;
                                crate::paint::vline(
                                    window,
                                    bounds.right() - crate::convert::hairline(window),
                                    bounds.origin.y,
                                    bounds.size.height,
                                    divider,
                                );
                            },
                        )
                        .absolute()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .w(crate::convert::hairline(window)),
                    )
                    .child(
                        gpui::canvas(
                            |_, _, _| (),
                            move |bounds, _, window, _| {
                                if ring_focus.is_focused(window) && window.last_input_was_keyboard()
                                {
                                    crate::paint::focus_ring(window, bounds, 0., ring, profile);
                                }
                            },
                        )
                        .absolute()
                        .inset_0(),
                    )
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        down_x.set(event.position.x);
                        window.focus(&focus_down, cx);
                        cx.stop_propagation();
                    })
                    .on_drag(drag, |_, _, _, cx| cx.new(|_| gpui::EmptyView))
                    .on_drag_move::<SidebarDrag>(move |event, window, cx| {
                        let drag = event.drag(cx);
                        if drag.owner != owner
                            || event.event.pressed_button != Some(MouseButton::Left)
                        {
                            return;
                        }
                        let proposed =
                            drag.width + (event.event.position.x - drag.start_x.get()).as_f32();
                        let mut next = drag.preference;
                        match layout::drag(Divider::Sidebar, &g, proposed, f32::MAX, 0.) {
                            Some(width) => {
                                next.expanded = true;
                                next.width = width;
                            }
                            None => next.expanded = false,
                        }
                        if let Some(f) = &change {
                            f(&next, window, cx);
                        }
                        cx.stop_propagation();
                    })
                    .on_key_down(move |event, window, cx| {
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        let mut next = pref;
                        match event.keystroke.key.as_str() {
                            "enter" | "space" => next.expanded = !pref.expanded,
                            "home" => {
                                next.expanded = true;
                                next.width = g.sidebar_min;
                            }
                            "end" => {
                                next.expanded = true;
                                next.width = g.sidebar_max;
                            }
                            "left" | "right" => {
                                let delta = if event.keystroke.key == "left" {
                                    -10.
                                } else {
                                    10.
                                };
                                match layout::drag(
                                    Divider::Sidebar,
                                    &g,
                                    width + delta,
                                    f32::MAX,
                                    0.,
                                ) {
                                    Some(width) => {
                                        next.expanded = true;
                                        next.width = width;
                                    }
                                    None => next.expanded = false,
                                }
                            }
                            _ => return,
                        }
                        cx.stop_propagation();
                        if let Some(f) = &keyboard {
                            f(&next, window, cx);
                        }
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, Context, Modifiers, Render, TestAppContext, VisualTestContext};

    struct Harness {
        pref: SidebarPreference,
        effective: Option<(f32, bool)>,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let sidebar = Sidebar::new(
                "sidebar",
                self.pref,
                div()
                    .debug_selector(|| "expanded-content".into())
                    .child("Documents"),
                div().debug_selector(|| "collapsed-rail".into()).child("D"),
            );
            let sidebar = if let Some((width, expanded)) = self.effective {
                sidebar.effective(width, expanded)
            } else {
                sidebar
            };
            div()
                .size_full()
                .flex()
                .child(sidebar.on_change(cx.listener(|h, pref, _, cx| {
                    h.pref = *pref;
                    cx.notify();
                })))
                .child(
                    div()
                        .flex_1()
                        .debug_selector(|| "workspace".into())
                        .child("Workspace"),
                )
        }
    }
    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    fn pref(h: &gpui::Entity<Harness>, cx: &mut VisualTestContext) -> SidebarPreference {
        cx.update(|_, cx| h.read(cx).pref)
    }
    #[test]
    fn divider_drag_resizes_outside_hitbox_and_stops_on_release() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|_, _| Harness {
            pref: SidebarPreference {
                expanded: true,
                width: 264.,
            },
            effective: None,
        });
        draw(cx);
        let bounds = cx.debug_bounds("sidebar-divider").unwrap();
        let start = bounds.center();
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x + px(20.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        cx.simulate_mouse_move(
            point(start.x + px(60.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(pref(&h, cx).width, 324.);
        cx.simulate_mouse_up(
            point(start.x + px(60.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        cx.simulate_mouse_move(
            point(start.x + px(120.), start.y),
            None,
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(pref(&h, cx).width, 324.);
        let start = cx.debug_bounds("sidebar-divider").unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x - px(20.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        cx.simulate_mouse_move(
            point(start.x - px(250.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(
            pref(&h, cx),
            SidebarPreference {
                expanded: false,
                width: 324.
            }
        );
        cx.simulate_mouse_up(
            point(start.x - px(250.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(cx.debug_bounds("workspace").unwrap().origin.x, px(52.));
    }

    #[test]
    fn effective_demotion_and_theme_changes_preserve_saved_preference() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|_, _| Harness {
            pref: SidebarPreference {
                expanded: true,
                width: 264.,
            },
            effective: Some((52., false)),
        });
        for theme in crate::ThemeId::ALL {
            cx.update(|_, cx| {
                crate::set_theme_preference(crate::ThemePreference::explicit(theme), cx)
            });
            draw(cx);
            assert_eq!(
                pref(&h, cx),
                SidebarPreference {
                    expanded: true,
                    width: 264.
                }
            );
            assert_eq!(cx.debug_bounds("workspace").unwrap().origin.x, px(52.));
        }
        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.effective = None;
                cx.notify();
            })
        });
        draw(cx);
        assert_eq!(cx.debug_bounds("workspace").unwrap().origin.x, px(264.));
    }
    #[test]
    fn divider_keyboard_collapse_preserves_width_and_reserved_rail() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|_, _| Harness {
            pref: SidebarPreference {
                expanded: true,
                width: 264.,
            },
            effective: None,
        });
        draw(cx);
        let bounds = cx.debug_bounds("sidebar-divider").unwrap();
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        cx.simulate_keystrokes("enter");
        draw(cx);
        assert_eq!(
            pref(&h, cx),
            SidebarPreference {
                expanded: false,
                width: 264.
            }
        );
        assert!(cx.debug_bounds("collapsed-rail").is_some());
        assert_eq!(cx.debug_bounds("workspace").unwrap().origin.x, px(52.));
        cx.simulate_keystrokes("enter");
        draw(cx);
        assert_eq!(
            pref(&h, cx),
            SidebarPreference {
                expanded: true,
                width: 264.
            }
        );
        assert_eq!(cx.debug_bounds("workspace").unwrap().origin.x, px(264.));
        cx.simulate_keystrokes("end");
        draw(cx);
        assert_eq!(pref(&h, cx).width, 440.);
    }
}
