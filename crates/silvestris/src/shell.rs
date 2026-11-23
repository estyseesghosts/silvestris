//! Measured application shell (#130-#132; handbook 13).
//! GPUI container_query supplies the actual allocated logical size before
//! child layout, so overlay presentation never changes workspace geometry.
use crate::{
    controls::{Button, ButtonStyle, ControlSize},
    convert::hsla,
    sidebar::Sidebar,
    ActiveTheme,
};
use gpui::{
    container_query, div, prelude::*, px, AnyElement, App, ElementId, FocusHandle, MouseButton,
    Pixels, Window,
};
use silvestris_core::layout::{
    self, InspectorPreference, PlanInput, Presentation, ShellGeometry, ShellPlan, SidebarPreference,
};
use std::{cell::Cell, rc::Rc};

type InspectorHandler = Rc<dyn Fn(&InspectorPreference, &mut Window, &mut App)>;
type SidebarHandler = Rc<dyn Fn(&SidebarPreference, &mut Window, &mut App)>;
type InspectorKeyHandler = Rc<dyn Fn(&gpui::KeyDownEvent, &mut Window, &mut App) -> bool>;
type ContentBuilder =
    Box<dyn FnOnce(&ShellPlan, gpui::Size<Pixels>, &mut Window, &mut App) -> ShellContent>;

/// App-owned content built from the shell's measured plan. Use this when
/// workspace children (for example fitted previews) need their allocated size.
pub struct ShellContent {
    pub sidebar: AnyElement,
    pub sidebar_rail: AnyElement,
    pub workspace: AnyElement,
    pub inspector: Option<AnyElement>,
    pub inspector_title: gpui::SharedString,
    pub inspector_rail: Option<AnyElement>,
}

struct Runtime {
    previous: Option<ShellPlan>,
    inspector_focus: FocusHandle,
    divider_focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    drag_x: Rc<Cell<Pixels>>,
}
#[derive(Clone)]
struct InspectorDrag {
    owner: ElementId,
    start_x: Rc<Cell<Pixels>>,
    width: f32,
    preference: InspectorPreference,
}

#[derive(IntoElement)]
pub struct Shell {
    id: ElementId,
    sidebar: SidebarPreference,
    inspector: InspectorPreference,
    geometry: ShellGeometry,
    sidebar_content: AnyElement,
    sidebar_rail: AnyElement,
    workspace: AnyElement,
    inspector_content: Option<AnyElement>,
    inspector_title: gpui::SharedString,
    inspector_rail: Option<AnyElement>,
    on_sidebar: Option<SidebarHandler>,
    on_inspector: Option<InspectorHandler>,
    content_builder: Option<ContentBuilder>,
    rail_enabled: bool,
    inspector_key: Option<InspectorKeyHandler>,
}
impl Shell {
    pub fn new(
        id: impl Into<ElementId>,
        sidebar: SidebarPreference,
        inspector: InspectorPreference,
        sidebar_content: impl IntoElement,
        sidebar_rail: impl IntoElement,
        workspace: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            sidebar,
            inspector,
            geometry: ShellGeometry::LOTUS_LEGACY,
            sidebar_content: sidebar_content.into_any_element(),
            sidebar_rail: sidebar_rail.into_any_element(),
            workspace: workspace.into_any_element(),
            inspector_content: None,
            inspector_title: "Inspector".into(),
            inspector_rail: None,
            on_sidebar: None,
            on_inspector: None,
            content_builder: None,
            rail_enabled: false,
            inspector_key: None,
        }
    }

    /// Build app content after measurement, before child layout. The callback
    /// runs once per render; it must not persist effective layout as preference.
    /// `rail_enabled` is explicit because rail content is built after planning.
    pub fn measured(
        id: impl Into<ElementId>,
        sidebar: SidebarPreference,
        inspector: InspectorPreference,
        rail_enabled: bool,
        build: impl FnOnce(&ShellPlan, gpui::Size<Pixels>, &mut Window, &mut App) -> ShellContent
            + 'static,
    ) -> Self {
        let mut shell = Self::new(id, sidebar, inspector, div(), div(), div());
        shell.content_builder = Some(Box::new(build));
        shell.rail_enabled = rail_enabled;
        shell
    }
    pub fn geometry(mut self, geometry: ShellGeometry) -> Self {
        self.geometry = geometry;
        self
    }
    pub fn inspector(
        mut self,
        title: impl Into<gpui::SharedString>,
        content: impl IntoElement,
    ) -> Self {
        self.inspector_title = title.into();
        self.inspector_content = Some(content.into_any_element());
        self
    }
    pub fn inspector_rail(mut self, content: impl IntoElement) -> Self {
        self.inspector_rail = Some(content.into_any_element());
        self.rail_enabled = true;
        self
    }
    pub fn on_sidebar_change(
        mut self,
        f: impl Fn(&SidebarPreference, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_sidebar = Some(Rc::new(f));
        self
    }
    pub fn on_inspector_change(
        mut self,
        f: impl Fn(&InspectorPreference, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_inspector = Some(Rc::new(f));
        self
    }

    /// Handle app shortcuts or dismiss an app popup before shell Escape.
    /// Return true when handled. Remaining keys stay inside the inspector.
    pub fn on_inspector_key_down(
        mut self,
        f: impl Fn(&gpui::KeyDownEvent, &mut Window, &mut App) -> bool + 'static,
    ) -> Self {
        self.inspector_key = Some(Rc::new(f));
        self
    }
}
impl RenderOnce for Shell {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, cx| Runtime {
            previous: None,
            inspector_focus: cx.focus_handle(),
            divider_focus: cx.focus_handle().tab_stop(true),
            return_focus: None,
            drag_x: Rc::new(Cell::new(px(0.))),
        });
        container_query(move |size, window, cx| {
            let (plan, focus, divider_focus, start_x) = runtime.update(cx, |state, cx| {
                let plan = layout::plan(PlanInput {
                    viewport: size.width.as_f32(),
                    geometry: self.geometry,
                    sidebar: self.sidebar,
                    inspector: self.inspector,
                    rail_enabled: self.rail_enabled,
                    previous: state.previous.as_ref(),
                });
                let was_open = state
                    .previous
                    .is_some_and(|p| p.presentation != Presentation::Closed);
                let is_open = plan.presentation != Presentation::Closed;
                if is_open && !was_open {
                    let previous = window.focused(cx);
                    state.return_focus = previous.clone();
                    let inspector = state.inspector_focus.clone();
                    // The toolbar may already have reported focus to AccessKit during
                    // this frame's prepaint. Transfer focus after the frame is built.
                    window.defer(cx, move |window, cx| {
                        if window.focused(cx) == previous {
                            window.focus(&inspector, cx);
                        }
                    });
                } else if was_open && !is_open {
                    let target = state.return_focus.take();
                    if state.inspector_focus.contains_focused(window, cx) {
                        let previous = window.focused(cx);
                        window.defer(cx, move |window, cx| {
                            if window.focused(cx) == previous {
                                if let Some(target) = target {
                                    window.focus(&target, cx);
                                } else {
                                    window.blur(cx);
                                }
                            }
                        });
                    }
                }
                state.previous = Some(plan);
                (
                    plan,
                    state.inspector_focus.clone(),
                    state.divider_focus.clone(),
                    state.drag_x.clone(),
                )
            });
            if let Some(build) = self.content_builder.take() {
                let content = build(&plan, size, window, cx);
                self.sidebar_content = content.sidebar;
                self.sidebar_rail = content.sidebar_rail;
                self.workspace = content.workspace;
                self.inspector_content = content.inspector;
                self.inspector_title = content.inspector_title;
                self.inspector_rail = content.inspector_rail;
            }
            let theme = cx.theme().clone();
            let mut root = div()
                .id(self.id.clone())
                .relative()
                .size_full()
                .overflow_hidden()
                .font_family(theme.type_.ui)
                .text_size(px(theme.type_.size_regular))
                .text_color(hsla(theme.colors.text))
                .when(plan.sidebar.width > 0.0, |root| {
                    root.child(
                        div()
                            .absolute()
                            .left_0()
                            .top_0()
                            .bottom_0()
                            .w(px(plan.sidebar.width))
                            .child(
                                Sidebar::new(
                                    "sidebar",
                                    self.sidebar,
                                    self.sidebar_content,
                                    self.sidebar_rail,
                                )
                                .geometry(self.geometry)
                                .effective(plan.sidebar.width, plan.sidebar_expanded)
                                .on_change({
                                    let callback = self.on_sidebar;
                                    move |p, w, cx| {
                                        if let Some(f) = &callback {
                                            f(p, w, cx);
                                        }
                                    }
                                }),
                            ),
                    )
                })
                .child(
                    div()
                        .id("workspace")
                        .debug_selector(|| "shell-workspace".into())
                        .absolute()
                        .left(px(plan.workspace.x))
                        .top_0()
                        .bottom_0()
                        .w(px(plan.workspace.width))
                        .overflow_hidden()
                        .child(self.workspace),
                );
            let preference = self.inspector;
            if plan.presentation == Presentation::Overlay {
                let callback = self.on_inspector.clone();
                let right_callback = callback.clone();
                root = root.child(
                    div()
                        .id("inspector-shield")
                        .absolute()
                        .inset_0()
                        .on_mouse_down(MouseButton::Left, move |_, w, cx| {
                            cx.stop_propagation();
                            if let Some(f) = &callback {
                                f(
                                    &InspectorPreference {
                                        visible: false,
                                        ..preference
                                    },
                                    w,
                                    cx,
                                );
                            }
                        })
                        .on_mouse_down(MouseButton::Right, move |_, w, cx| {
                            cx.stop_propagation();
                            if let Some(f) = &right_callback {
                                f(
                                    &InspectorPreference {
                                        visible: false,
                                        ..preference
                                    },
                                    w,
                                    cx,
                                );
                            }
                        })
                        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                        .on_mouse_move(|_, _, cx| cx.stop_propagation())
                        .on_scroll(|_, _, cx| cx.stop_propagation()),
                );
            }
            if let (Some(span), Some(rail)) = (plan.rail, self.inspector_rail) {
                root = root.child(
                    div()
                        .id("inspector-rail")
                        .debug_selector(|| "shell-inspector-rail".into())
                        .absolute()
                        .left(px(span.x))
                        .top_0()
                        .bottom_0()
                        .w(px(span.width))
                        .bg(hsla(theme.colors.sidebar))
                        .child(rail),
                );
            }
            if let Some(span) = plan.dock.or(plan.overlay) {
                let pin = self.on_inspector.clone();
                let close = self.on_inspector.clone();
                let escape = self.on_inspector.clone();
                let resize = self.on_inspector;
                let key_resize = resize.clone();
                let down_x = start_x.clone();
                let owner = self.id.clone();
                let geometry = self.geometry;
                let pointer_focus = focus.clone();
                let keyboard_focus = focus.clone();
                let inspector_key = self.inspector_key;
                let drag = InspectorDrag {
                    owner: owner.clone(),
                    start_x,
                    width: span.width,
                    preference,
                };
                root = root.child(
                    div()
                        .id("inspector")
                        .debug_selector(|| "shell-inspector".into())
                        .role(gpui::Role::Group)
                        .aria_label(self.inspector_title.clone())
                        .absolute()
                        .left(px(span.x))
                        .top_0()
                        .bottom_0()
                        .w(px(span.width))
                        .flex()
                        .flex_col()
                        .overflow_hidden()
                        .track_focus(&focus)
                        .key_context("SilvestrisInspector")
                        .bg(hsla(theme.colors.window))
                        .border_l_1()
                        .border_color(hsla(theme.colors.divider))
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            if !pointer_focus.contains_focused(window, cx) {
                                window.focus(&pointer_focus, cx);
                            }
                            cx.stop_propagation();
                        })
                        .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_move(|_, _, cx| cx.stop_propagation())
                        .on_scroll(|_, _, cx| cx.stop_propagation())
                        .on_key_down(move |event, w, cx| {
                            // GPUI delivers text input after key propagation. A focused
                            // TextField needs printable keys to reach its input handler.
                            if event.keystroke.key_char.is_some()
                                && w.context_stack().iter().any(|context| {
                                    context.contains(crate::controls::text_field::KEY_CONTEXT)
                                })
                            {
                                return;
                            }
                            if inspector_key.as_ref().is_some_and(|f| f(event, w, cx)) {
                                cx.stop_propagation();
                                return;
                            }
                            if event.keystroke.key == "tab" {
                                let mut first = None;
                                loop {
                                    if event.keystroke.modifiers.shift {
                                        w.focus_prev(cx);
                                    } else {
                                        w.focus_next(cx);
                                    }
                                    if plan.presentation != Presentation::Overlay
                                        || keyboard_focus.contains_focused(w, cx)
                                    {
                                        break;
                                    }
                                    let current = w.focused(cx);
                                    if first.as_ref() == Some(&current) {
                                        w.focus(&keyboard_focus, cx);
                                        break;
                                    }
                                    first.get_or_insert(current);
                                }
                            }
                            if event.keystroke.key == "escape"
                                && plan.presentation == Presentation::Overlay
                            {
                                if let Some(f) = &escape {
                                    f(
                                        &InspectorPreference {
                                            visible: false,
                                            ..preference
                                        },
                                        w,
                                        cx,
                                    );
                                }
                            }
                            cx.stop_propagation();
                        })
                        .child(
                            div()
                                .flex()
                                .flex_none()
                                .items_center()
                                .gap(px(6.))
                                .p(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .child(self.inspector_title),
                                )
                                .child(
                                    Button::new(
                                        "pin",
                                        if preference.pinned { "Unpin" } else { "Pin" },
                                    )
                                    .size(ControlSize::Small)
                                    .style(ButtonStyle::Textured)
                                    .on_click(
                                        move |_, w, cx| {
                                            if let Some(f) = &pin {
                                                f(
                                                    &InspectorPreference {
                                                        pinned: !preference.pinned,
                                                        ..preference
                                                    },
                                                    w,
                                                    cx,
                                                );
                                            }
                                        },
                                    ),
                                )
                                .child(
                                    Button::new("close", "Close")
                                        .size(ControlSize::Small)
                                        .style(ButtonStyle::Textured)
                                        .on_click(move |_, w, cx| {
                                            if let Some(f) = &close {
                                                f(
                                                    &InspectorPreference {
                                                        visible: false,
                                                        ..preference
                                                    },
                                                    w,
                                                    cx,
                                                );
                                            }
                                        }),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .overflow_hidden()
                                .children(self.inspector_content),
                        )
                        .child(
                            div()
                                .id("inspector-divider")
                                .debug_selector(|| "inspector-divider".into())
                                .role(gpui::Role::Splitter)
                                .aria_label("Inspector width")
                                .track_focus(&divider_focus)
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(theme.metrics.splitter_hot_zone.max(4.)))
                                .cursor(gpui::CursorStyle::ResizeLeftRight)
                                .on_mouse_down(MouseButton::Left, move |e, window, cx| {
                                    down_x.set(e.position.x);
                                    window.focus(&divider_focus, cx);
                                    cx.stop_propagation();
                                })
                                .on_key_down(move |e, window, cx| {
                                    if e.keystroke.modifiers.modified() {
                                        return;
                                    }
                                    let width = match e.keystroke.key.as_str() {
                                        "left" => preference.width + 10.,
                                        "right" => preference.width - 10.,
                                        "home" => geometry.inspector_min,
                                        "end" => geometry.inspector_max,
                                        _ => return,
                                    }
                                    .clamp(geometry.inspector_min, geometry.inspector_max);
                                    cx.stop_propagation();
                                    if let Some(f) = &key_resize {
                                        f(
                                            &InspectorPreference {
                                                width,
                                                ..preference
                                            },
                                            window,
                                            cx,
                                        );
                                    }
                                })
                                .on_drag(drag, |_, _, _, cx| cx.new(|_| gpui::EmptyView))
                                .on_drag_move::<InspectorDrag>(move |event, w, cx| {
                                    let drag = event.drag(cx);
                                    if drag.owner != owner
                                        || event.event.pressed_button != Some(MouseButton::Left)
                                    {
                                        return;
                                    }
                                    let proposed = drag.width
                                        - (event.event.position.x - drag.start_x.get()).as_f32();
                                    let mut next = drag.preference;
                                    match layout::drag(
                                        layout::Divider::Inspector,
                                        &geometry,
                                        0.,
                                        proposed,
                                        0.,
                                    ) {
                                        Some(width) => next.width = width,
                                        None => next.visible = false,
                                    }
                                    if let Some(f) = &resize {
                                        f(&next, w, cx);
                                    }
                                    cx.stop_propagation();
                                }),
                        ),
                );
            }
            root
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{
        point, size, Context, Focusable, Modifiers, Render, TestAppContext, VisualTestContext,
    };
    struct Harness {
        sidebar: SidebarPreference,
        inspector: InspectorPreference,
        focus: FocusHandle,
        clicks: usize,
        popup_open: bool,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .p(px(24.))
                .track_focus(&self.focus)
                .on_mouse_down(MouseButton::Left, cx.listener(|h, _, _, _| h.clicks += 1))
                .child(
                    Shell::new(
                        "shell",
                        self.sidebar,
                        self.inspector,
                        div().child("Sources"),
                        div().child("S"),
                        div().size_full().child("Guest workspace"),
                    )
                    .inspector("Details", div().child("Inspector properties"))
                    .inspector_rail(div().child("I"))
                    .on_inspector_key_down({
                        let owner = cx.entity();
                        move |event, _, cx| {
                            owner.update(cx, |h, cx| {
                                if event.keystroke.key == "escape" && h.popup_open {
                                    h.popup_open = false;
                                    cx.notify();
                                    true
                                } else {
                                    false
                                }
                            })
                        }
                    })
                    .on_sidebar_change(cx.listener(|h, pref, _, cx| {
                        h.sidebar = *pref;
                        cx.notify();
                    }))
                    .on_inspector_change(cx.listener(|h, pref, _, cx| {
                        h.inspector = *pref;
                        cx.notify();
                    })),
                )
        }
    }
    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }
    fn update(
        h: &gpui::Entity<Harness>,
        cx: &mut VisualTestContext,
        inspector: InspectorPreference,
    ) {
        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.inspector = inspector;
                cx.notify();
            })
        });
        draw(cx);
    }
    const CLOSED: InspectorPreference = InspectorPreference {
        visible: false,
        pinned: false,
        width: 420.,
    };
    #[test]
    fn measured_content_uses_the_same_plan_as_shell_placement() {
        struct Measured {
            inspector: InspectorPreference,
        }
        impl Render for Measured {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().p(px(24.)).child(Shell::measured(
                    "measured",
                    SidebarPreference {
                        expanded: true,
                        width: 264.,
                    },
                    self.inspector,
                    true,
                    |plan, size, _, _| ShellContent {
                        sidebar: div().into_any_element(),
                        sidebar_rail: div().into_any_element(),
                        workspace: div()
                            .id("fitted-preview")
                            .debug_selector(|| "fitted-preview".into())
                            .w(px(plan.workspace.width))
                            .h(size.height)
                            .into_any_element(),
                        inspector: plan.dock.or(plan.overlay).map(|span| {
                            div()
                                .id("fitted-inspector")
                                .debug_selector(|| "fitted-inspector".into())
                                .w(px(span.width))
                                .h_full()
                                .into_any_element()
                        }),
                        inspector_title: "Measured".into(),
                        inspector_rail: Some(div().into_any_element()),
                    },
                ))
            }
        }
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|_, _| Measured { inspector: CLOSED });
        for viewport in [1200., 750., 600., 1200.] {
            cx.simulate_resize(size(px(viewport), px(650.)));
            for (visible, pinned) in [(false, false), (true, false), (true, true)] {
                cx.update(|_, cx| {
                    h.update(cx, |h, cx| {
                        h.inspector = InspectorPreference {
                            visible,
                            pinned,
                            ..CLOSED
                        };
                        cx.notify();
                    })
                });
                draw(cx);
                let workspace = cx.debug_bounds("shell-workspace").unwrap();
                assert_eq!(cx.debug_bounds("fitted-preview").unwrap(), workspace);
                if visible {
                    assert_eq!(
                        cx.debug_bounds("fitted-inspector").unwrap().size.width,
                        cx.debug_bounds("shell-inspector").unwrap().size.width
                    );
                }
            }
        }
    }
    #[test]
    fn measured_overlay_preserves_workspace_and_pin_reserves_width_in_all_themes() {
        for theme in crate::ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(theme), cx));
            let (h, cx) = app.add_window_view(|window, cx| {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                Harness {
                    sidebar: SidebarPreference {
                        expanded: true,
                        width: 264.,
                    },
                    inspector: CLOSED,
                    focus,
                    clicks: 0,
                    popup_open: true,
                }
            });
            cx.simulate_resize(size(px(1200.), px(600.)));
            draw(cx);
            let closed = cx.debug_bounds("shell-workspace").unwrap();
            for scale in [1.25, 1.5, 2.] {
                cx.simulate_scale_factor_change(scale);
                draw(cx);
                assert_eq!(cx.debug_bounds("shell-workspace").unwrap(), closed);
            }
            // The planner uses the padded container width, not the 1200 px window.
            assert_eq!(closed.size.width, px(1200. - 48. - 264. - 44.));
            update(
                &h,
                cx,
                InspectorPreference {
                    visible: true,
                    ..CLOSED
                },
            );
            assert_eq!(cx.debug_bounds("shell-workspace").unwrap(), closed);
            assert!(cx.debug_bounds("shell-inspector").is_some());
            update(
                &h,
                cx,
                InspectorPreference {
                    visible: true,
                    pinned: true,
                    ..CLOSED
                },
            );
            assert_eq!(
                cx.debug_bounds("shell-workspace").unwrap().size.width,
                closed.size.width - px(420.)
            );
            update(
                &h,
                cx,
                InspectorPreference {
                    visible: true,
                    ..CLOSED
                },
            );
            assert_eq!(cx.debug_bounds("shell-workspace").unwrap(), closed);
            let before_tab = cx.update(|w, cx| w.focused(cx));
            cx.simulate_keystrokes("tab");
            draw(cx);
            let first_tab = cx.update(|w, cx| w.focused(cx));
            assert_ne!(first_tab, before_tab, "Tab reaches inspector controls");
            for _ in 0..3 {
                cx.simulate_keystrokes("tab");
                draw(cx);
            }
            assert_eq!(
                cx.update(|w, cx| w.focused(cx)),
                first_tab,
                "overlay cycles Pin, Close and divider only"
            );
            // A guest can take focus while a dock/overlay stays visible. Clicking
            // its empty content must restore inspector focus before typing.
            cx.update(|window, cx| {
                let focus = h.read(cx).focus.clone();
                window.focus(&focus, cx);
            });
            let inspector_center = cx.debug_bounds("shell-inspector").unwrap().center();
            cx.simulate_click(inspector_center, Modifiers::default());
            draw(cx);
            cx.update(|window, cx| assert!(!h.read(cx).focus.is_focused(window)));
            cx.simulate_keystrokes("escape");
            draw(cx);
            cx.update(|_, cx| {
                assert!(!h.read(cx).popup_open);
                assert!(h.read(cx).inspector.visible, "app popup closes first");
            });
            cx.simulate_keystrokes("escape");
            draw(cx);
            assert_eq!(cx.debug_bounds("shell-workspace").unwrap(), closed);
            cx.update(|window, cx| {
                assert!(!h.read(cx).inspector.visible);
                assert!(h.read(cx).focus.is_focused(window));
            });
        }
    }
    #[test]
    fn narrow_pinned_inspector_demotes_without_changing_preference_and_outside_click_is_shielded() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|window, cx| {
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            Harness {
                sidebar: SidebarPreference {
                    expanded: true,
                    width: 264.,
                },
                inspector: InspectorPreference {
                    visible: true,
                    pinned: true,
                    ..CLOSED
                },
                focus,
                clicks: 0,
                popup_open: false,
            }
        });
        cx.simulate_resize(size(px(1200.), px(600.)));
        draw(cx);
        let docked = cx.debug_bounds("shell-workspace").unwrap().size.width;
        cx.simulate_resize(size(px(750.), px(600.)));
        draw(cx);
        cx.update(|_, cx| assert!(h.read(cx).inspector.pinned));
        assert!(cx.debug_bounds("shell-inspector-rail").is_some());
        cx.simulate_resize(size(px(1200.), px(600.)));
        draw(cx);
        assert_eq!(
            cx.debug_bounds("shell-workspace").unwrap().size.width,
            docked
        );
        update(
            &h,
            cx,
            InspectorPreference {
                visible: true,
                ..CLOSED
            },
        );
        let workspace = cx.debug_bounds("shell-workspace").unwrap();
        cx.simulate_click(
            point(workspace.left() + px(10.), workspace.center().y),
            Modifiers::default(),
        );
        draw(cx);
        cx.update(|_, cx| {
            assert!(!h.read(cx).inspector.visible);
            assert_eq!(h.read(cx).clicks, 0);
        });
    }
    #[test]
    fn inspector_resize_uses_leading_edge_and_retains_pin_flag() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|_, cx| Harness {
            sidebar: SidebarPreference {
                expanded: true,
                width: 264.,
            },
            inspector: InspectorPreference {
                visible: true,
                pinned: true,
                ..CLOSED
            },
            focus: cx.focus_handle(),
            clicks: 0,
            popup_open: false,
        });
        cx.simulate_resize(size(px(1400.), px(600.)));
        draw(cx);
        let start = cx.debug_bounds("inspector-divider").unwrap().center();
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.simulate_mouse_move(
            point(start.x - px(20.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        cx.simulate_mouse_move(
            point(start.x - px(60.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        cx.simulate_mouse_up(
            point(start.x - px(60.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        cx.update(|_, cx| {
            assert_eq!(h.read(cx).inspector.width, 480.);
            assert!(h.read(cx).inspector.pinned);
        });
        cx.simulate_keystrokes("left");
        draw(cx);
        cx.update(|_, cx| assert_eq!(h.read(cx).inspector.width, 490.));
        cx.simulate_keystrokes("home");
        draw(cx);
        cx.update(|_, cx| assert_eq!(h.read(cx).inspector.width, 300.));
    }

    struct FocusHarness {
        inspector: InspectorPreference,
        doc: FocusHandle,
        toolbar: FocusHandle,
        defer_toolbar: bool,
        doc_focused_at_prepaint: Rc<std::cell::Cell<Option<bool>>>,
    }
    impl Render for FocusHarness {
        fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            if self.defer_toolbar {
                self.defer_toolbar = false;
                let toolbar = self.toolbar.clone();
                // A frame can report focus to another view after the Shell has
                // asked for the inspector. The deferred transfer must not win.
                window.defer(cx, move |window, cx| window.focus(&toolbar, cx));
            }
            div()
                .size_full()
                .p(px(24.))
                .track_focus(&self.doc)
                .child(
                    Shell::new(
                        "focus-shell",
                        SidebarPreference {
                            expanded: true,
                            width: 264.,
                        },
                        self.inspector,
                        div().child("Sources"),
                        div().child("S"),
                        div().size_full().child("Guest workspace"),
                    )
                    .inspector("Details", div().child("Inspector properties"))
                    .on_inspector_change(cx.listener(|h, pref, _, cx| {
                        h.inspector = *pref;
                        cx.notify();
                    })),
                )
                .child({
                    let focused = self.doc_focused_at_prepaint.clone();
                    let doc = self.doc.clone();
                    gpui::canvas(
                        move |_, window, _| {
                            if focused.get().is_none() {
                                focused.set(Some(doc.is_focused(window)));
                            }
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full()
                })
        }
    }
    fn focused(cx: &mut VisualTestContext, handle: &FocusHandle) -> bool {
        cx.update(|window, _| handle.is_focused(window))
    }
    #[test]
    fn deferred_inspector_focus_does_not_override_a_toolbar() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|window, cx| {
            let doc = cx.focus_handle();
            window.focus(&doc, cx);
            FocusHarness {
                inspector: CLOSED,
                doc,
                toolbar: cx.focus_handle(),
                defer_toolbar: false,
                doc_focused_at_prepaint: Rc::new(std::cell::Cell::new(None)),
            }
        });
        cx.simulate_resize(size(px(1200.), px(650.)));
        draw(cx);
        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.inspector = InspectorPreference {
                    visible: true,
                    pinned: false,
                    ..CLOSED
                };
                h.defer_toolbar = true;
                cx.notify();
            })
        });
        draw(cx);
        draw(cx);
        cx.update(|window, cx| {
            let h = h.read(cx);
            assert!(
                h.toolbar.is_focused(window),
                "a focus change from the frame must not be overridden by the inspector"
            );
            assert!(!h.doc.is_focused(window));
        });
    }
    #[test]
    fn inspector_focus_moves_after_the_frame_and_returns_on_close() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|window, cx| {
            let doc = cx.focus_handle();
            window.focus(&doc, cx);
            FocusHarness {
                inspector: CLOSED,
                doc,
                toolbar: cx.focus_handle(),
                defer_toolbar: false,
                doc_focused_at_prepaint: Rc::new(std::cell::Cell::new(None)),
            }
        });
        cx.simulate_resize(size(px(1200.), px(650.)));
        draw(cx);
        let doc = cx.update(|_, cx| h.read(cx).doc.clone());
        let toolbar = cx.update(|_, cx| h.read(cx).toolbar.clone());
        let prepaint = cx.update(|_, cx| h.read(cx).doc_focused_at_prepaint.clone());
        cx.update(|_, cx| {
            prepaint.set(None);
            h.update(cx, |h, cx| {
                h.inspector = InspectorPreference {
                    visible: true,
                    pinned: false,
                    ..CLOSED
                };
                cx.notify();
            })
        });
        assert_eq!(
            prepaint.get(),
            Some(true),
            "focus must not move until after the frame that opened the inspector"
        );
        draw(cx);
        assert!(!focused(cx, &doc), "opening the inspector moves focus");
        assert!(!focused(cx, &toolbar));
        cx.update(|_, cx| {
            h.update(cx, |h, cx| {
                h.inspector = CLOSED;
                cx.notify();
            })
        });
        draw(cx);
        draw(cx);
        assert!(
            focused(cx, &doc),
            "closing the inspector returns focus to the previous handle"
        );
    }

    struct KeyHarness {
        field: gpui::Entity<crate::controls::TextField>,
        hits: Rc<std::cell::Cell<usize>>,
    }
    impl Render for KeyHarness {
        fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let hits = self.hits.clone();
            div().size_full().p(px(24.)).child(
                Shell::new(
                    "key-shell",
                    SidebarPreference {
                        expanded: true,
                        width: 264.,
                    },
                    InspectorPreference {
                        visible: true,
                        pinned: false,
                        ..CLOSED
                    },
                    div().child("Sources"),
                    div().child("S"),
                    div().size_full().child("Guest workspace"),
                )
                .inspector("Details", div().child(self.field.clone()))
                .on_inspector_key_down(move |_, _, _| {
                    hits.set(hits.get() + 1);
                    true
                }),
            )
        }
    }
    #[test]
    fn printable_keys_reach_a_focused_text_field_through_the_shell() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|window, cx| {
            let field = cx.new(|cx| crate::controls::TextField::new(window, cx));
            KeyHarness {
                field,
                hits: Rc::new(std::cell::Cell::new(0)),
            }
        });
        cx.simulate_resize(size(px(1200.), px(650.)));
        draw(cx);
        let focus = cx.update(|_, cx| h.read(cx).field.read(cx).focus_handle(cx));
        cx.update(|window, cx| window.focus(&focus, cx));
        draw(cx);

        cx.simulate_keystrokes("a");
        draw(cx);
        cx.update(|_, cx| {
            let h = h.read(cx);
            assert_eq!(h.field.read(cx).text(), "a");
            assert_eq!(
                h.hits.get(),
                0,
                "the shell key handler must not see printable keys while a field has focus"
            );
        });

        cx.simulate_keystrokes("escape");
        draw(cx);
        cx.update(|_, cx| {
            assert_eq!(
                h.read(cx).hits.get(),
                1,
                "non-printable keys still reach the shell key handler"
            );
        });
    }
}
