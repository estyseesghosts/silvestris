//! Reusable split view (#53).
//!
//! `SplitView` is the generic two-or-more-pane splitter, independent of
//! [`crate::shell::Shell`]. Shell keeps its sidebar/workspace/inspector policy;
//! a consumer can put a `SplitView` inside the workspace without teaching
//! Shell about recursive tiling.
//!
//! HIG pp. 336-338 define the whole geometry: a 1 px zero-width splitter with
//! a 2 px hot zone on each side, or a 9 px thick splitter with a circular
//! dimple. A split view never mixes orientations. Pane contents, collapse
//! policy and what a pane means belong to the consumer.
//!
//! The layout is measured with `container_query` before children are built, so
//! pane spans are exact on the first frame. `on_bounds` reports the measured
//! pane rectangles for domain views that must size themselves to their
//! allocation, such as an emulator canvas.

use crate::{
    convert::{hairline, hsla},
    paint::focus_ring,
    theme::ActiveTheme,
};
use gpui::{
    canvas, container_query, div, prelude::*, px, AnyElement, App, Bounds, CursorStyle, ElementId,
    FocusHandle, IntoElement, MouseButton, Pixels, RenderOnce, Window,
};
use silvestris_core::split::{
    self, SplitDividerState, SplitDividerStyle, SplitOrientation, SplitPane, SplitState,
};
use std::{cell::Cell, rc::Rc};

type ResizeHandler = Rc<dyn Fn(&SplitState, &mut Window, &mut App)>;
type BoundsHandler = Rc<dyn Fn(&[Bounds<Pixels>], &mut Window, &mut App)>;

struct Pane {
    id: ElementId,
    spec: SplitPane,
    content: AnyElement,
}

/// One split view. Panes are declared leading to trailing.
#[derive(IntoElement)]
pub struct SplitView {
    id: ElementId,
    orientation: SplitOrientation,
    divider_style: SplitDividerStyle,
    state: SplitState,
    panes: Vec<Pane>,
    on_resize: Option<ResizeHandler>,
    on_bounds: Option<BoundsHandler>,
}

impl SplitView {
    pub fn new(id: impl Into<ElementId>, orientation: SplitOrientation) -> Self {
        Self {
            id: id.into(),
            orientation,
            divider_style: SplitDividerStyle::Thin,
            state: SplitState::new(),
            panes: Vec::new(),
            on_resize: None,
            on_bounds: None,
        }
    }

    /// Thin (1 px) is the 10.5+ standard; thick (9 px) has a dimple.
    pub fn divider_style(mut self, style: SplitDividerStyle) -> Self {
        self.divider_style = style;
        self
    }

    /// Persisted divider positions. Only IDs and positions belong here.
    pub fn state(mut self, state: SplitState) -> Self {
        self.state = state;
        self
    }

    /// Add a pane. Its ID is also the stable ID of the divider on its trailing
    /// edge, so saving [`SplitState`] persists the right boundary when panes
    /// are reordered.
    pub fn pane(
        mut self,
        id: impl Into<ElementId>,
        spec: SplitPane,
        content: impl IntoElement,
    ) -> Self {
        self.panes.push(Pane {
            id: id.into(),
            spec,
            content: content.into_any_element(),
        });
        self
    }

    /// The consumer updates its persisted state here. `SplitView` is
    /// controlled: it never mutates the supplied state on its own.
    pub fn on_resize(
        mut self,
        handler: impl Fn(&SplitState, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// Exact pane rectangles after the split view has laid them out.
    pub fn on_bounds(
        mut self,
        handler: impl Fn(&[Bounds<Pixels>], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_bounds = Some(Rc::new(handler));
        self
    }
}

struct Runtime {
    focus: Vec<FocusHandle>,
    start: Rc<Cell<Pixels>>,
}

#[derive(Clone)]
struct DividerDrag {
    owner: ElementId,
    index: usize,
    orientation: SplitOrientation,
    style: SplitDividerStyle,
    panes: Vec<SplitPane>,
    sizes: Vec<f32>,
    ids: Vec<String>,
    start: Rc<Cell<Pixels>>,
    resize: Option<ResizeHandler>,
}

impl RenderOnce for SplitView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let pane_count = self.panes.len();
        let runtime = window.use_keyed_state(
            ElementId::named_usize(self.id.to_string(), pane_count),
            cx,
            |_, cx| Runtime {
                focus: (0..pane_count.saturating_sub(1))
                    .map(|_| cx.focus_handle().tab_stop(true))
                    .collect(),
                start: Rc::new(Cell::new(px(0.))),
            },
        );
        let runtime = runtime.read(cx);
        let focus = runtime.focus.clone();
        let start = runtime.start.clone();
        let owner = self.id.clone();
        let orientation = self.orientation;
        let style = self.divider_style;
        let state = self.state;
        let panes = self.panes;
        let on_resize = self.on_resize;
        let on_bounds = self.on_bounds;

        let drag_owner = owner.clone();
        let view = container_query(move |size, window, _cx| {
            let extent = if orientation.is_side_by_side() {
                size.width.as_f32()
            } else {
                size.height.as_f32()
            };
            let specs: Vec<SplitPane> = panes.iter().map(|pane| pane.spec).collect();
            let ids: Vec<String> = panes
                .iter()
                .take(pane_count.saturating_sub(1))
                .map(|pane| pane.id.to_string())
                .collect();
            // Resolve saved positions by divider ID, so reordering panes does
            // not move the wrong boundary.
            let ordered = SplitState {
                dividers: ids
                    .iter()
                    .filter_map(|id| {
                        state.position(id).map(|position| SplitDividerState {
                            id: id.clone(),
                            position,
                        })
                    })
                    .collect(),
            };
            let layout = split::layout(extent, &specs, &ordered, style);
            let divider = theme.colors.divider;
            let ring = theme.colors.focus_ring;
            let profile = theme.metrics.focus_ring;
            let hairline = hairline(window);
            let cursor = if orientation.is_side_by_side() {
                CursorStyle::ResizeLeftRight
            } else {
                CursorStyle::ResizeUpDown
            };

            let on_bounds = on_bounds.clone();
            let mut children = Vec::with_capacity(panes.len() * 2);
            let mut overlays = Vec::with_capacity(panes.len().saturating_sub(1));
            for (index, pane) in panes.into_iter().enumerate() {
                let slot = layout.panes[index];
                let mut element = div()
                    .id(pane.id.clone())
                    .debug_selector({
                        let id = pane.id.to_string();
                        move || id.clone()
                    })
                    .flex_none()
                    .overflow_hidden();
                if orientation.is_side_by_side() {
                    element = element.w(px(slot.extent)).h_full();
                } else {
                    element = element.h(px(slot.extent)).w_full();
                }
                children.push(element.child(pane.content).into_any_element());

                let Some(divider_slot) = layout.dividers.get(index).copied() else {
                    continue;
                };
                let Some(focus) = focus.get(index).cloned() else {
                    continue;
                };
                let sizes: Vec<f32> = layout.panes.iter().map(|slot| slot.extent).collect();
                let drag = DividerDrag {
                    owner: owner.clone(),
                    index,
                    orientation,
                    style,
                    panes: specs.clone(),
                    sizes,
                    ids: ids.clone(),
                    start: start.clone(),
                    resize: on_resize.clone(),
                };
                let start_down = start.clone();
                let line_focus = focus.clone();
                let ring_focus = focus.clone();
                let keyboard_sizes: Vec<f32> =
                    layout.panes.iter().map(|slot| slot.extent).collect();
                let keyboard_panes = specs.clone();
                let keyboard_ids = ids.clone();
                let keyboard_resize = on_resize.clone();
                let keyboard_extent = extent;

                // The visible seam is thin (1 px) or thick (9 px). Only the
                // thin style needs an overflowing 2 px hot zone on each side.
                let (width, height) = if orientation.is_side_by_side() {
                    (divider_slot.extent, size.height.as_f32())
                } else {
                    (size.width.as_f32(), divider_slot.extent)
                };
                let mut seam = div().relative().flex_none().w(px(width)).h(px(height));
                if style == SplitDividerStyle::Thick {
                    seam = seam
                        .bg(hsla(theme.colors.window))
                        .border_color(hsla(divider))
                        .when(orientation.is_side_by_side(), |el| {
                            el.border_l_1().border_r_1()
                        })
                        .when(!orientation.is_side_by_side(), |el| {
                            el.border_t_1().border_b_1()
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex_none()
                                .size(px(SplitDividerStyle::THICK_DIMPLE))
                                .rounded_full()
                                .bg(hsla(divider)),
                        );
                } else {
                    seam = seam.child(
                        div()
                            .flex_none()
                            .bg(hsla(divider))
                            .when(orientation.is_side_by_side(), |el| el.w(hairline).h_full())
                            .when(!orientation.is_side_by_side(), |el| el.h(hairline).w_full()),
                    );
                }
                // The hot zone owns interaction: the 1 px seam is too small to
                // hit reliably, and a thick seam's whole 9 px is hot. It
                // overlays neighbouring panes instead of consuming space.
                let hot = style.hot_zone();
                let mut hit = div()
                    .id(ElementId::named_usize(format!("{owner}-divider"), index))
                    .debug_selector({
                        let id = format!("{owner}-divider-{index}");
                        move || id.clone()
                    })
                    .absolute()
                    .role(gpui::Role::Splitter)
                    .aria_label("Split view divider")
                    .track_focus(&focus)
                    .cursor(cursor)
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        start_down.set(if orientation.is_side_by_side() {
                            event.position.x
                        } else {
                            event.position.y
                        });
                        window.focus(&line_focus, cx);
                        cx.stop_propagation();
                    })
                    .on_drag(drag, |_, _, _, cx| cx.new(|_| gpui::EmptyView))
                    .on_key_down(move |event, window, cx| {
                        if event.keystroke.modifiers.modified() {
                            return;
                        }
                        let delta = match (orientation, event.keystroke.key.as_str()) {
                            (SplitOrientation::Vertical, "left") => -10.0,
                            (SplitOrientation::Vertical, "right") => 10.0,
                            (SplitOrientation::Horizontal, "up") => -10.0,
                            (SplitOrientation::Horizontal, "down") => 10.0,
                            (_, "home") => -keyboard_extent,
                            (_, "end") => keyboard_extent,
                            _ => return,
                        };
                        let Some(positions) =
                            split::drag(&keyboard_panes, &keyboard_sizes, style, index, delta)
                        else {
                            return;
                        };
                        let next =
                            SplitState::with_dividers(keyboard_ids.iter().cloned().zip(positions));
                        cx.stop_propagation();
                        if let Some(handler) = &keyboard_resize {
                            handler(&next, window, cx);
                        }
                    });
                // Place the hot zone as a root overlay, not inside the 1 px
                // seam, so its hitbox is not clipped by the seam.
                hit = if orientation.is_side_by_side() {
                    hit.left(px(divider_slot.start - hot))
                        .top_0()
                        .bottom_0()
                        .w(px(divider_slot.extent + hot * 2.0))
                } else {
                    hit.top(px(divider_slot.start - hot))
                        .left_0()
                        .right_0()
                        .h(px(divider_slot.extent + hot * 2.0))
                };
                hit = hit.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                                focus_ring(window, bounds, 0., ring, profile);
                            }
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
                children.push(seam.into_any_element());
                overlays.push(hit.into_any_element());
            }
            children.extend(overlays);

            let mut root = div().relative().size_full().flex().overflow_hidden();
            root = if orientation.is_side_by_side() {
                root.flex_row()
            } else {
                root.flex_col()
            };
            root = root.on_children_prepainted(move |bounds, window, cx| {
                if let Some(handler) = &on_bounds {
                    let panes: Vec<Bounds<Pixels>> =
                        bounds.into_iter().take(pane_count * 2).step_by(2).collect();
                    handler(&panes, window, cx);
                }
            });
            root.children(children)
        });

        // The drag-move listener lives outside `container_query`, next to the
        // element that container_query measures. It receives pointer moves for
        // the whole split view while a divider drag owns the pointer.
        div()
            .size_full()
            .on_drag_move::<DividerDrag>(move |event, window, cx| {
                let (next, resize) = {
                    let drag = event.drag(cx);
                    if drag.owner != drag_owner
                        || event.event.pressed_button != Some(MouseButton::Left)
                    {
                        return;
                    }
                    let current = if drag.orientation.is_side_by_side() {
                        event.event.position.x
                    } else {
                        event.event.position.y
                    };
                    let delta = (current - drag.start.get()).as_f32();
                    let Some(positions) =
                        split::drag(&drag.panes, &drag.sizes, drag.style, drag.index, delta)
                    else {
                        return;
                    };
                    (
                        SplitState::with_dividers(drag.ids.iter().cloned().zip(positions)),
                        drag.resize.clone(),
                    )
                };
                if let Some(handler) = &resize {
                    handler(&next, window, cx);
                }
                cx.stop_propagation();
            })
            .child(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, size, Context, Modifiers, Render, TestAppContext, VisualTestContext};
    use std::{cell::RefCell, rc::Rc};

    struct Harness {
        state: SplitState,
        bounds: Rc<RefCell<Vec<Bounds<Pixels>>>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let bounds = self.bounds.clone();
            div().size_full().flex().child(
                SplitView::new("split", SplitOrientation::Vertical)
                    .state(self.state.clone())
                    .pane(
                        "left",
                        SplitPane::new(80.0, 320.0),
                        div()
                            .debug_selector(|| "split-left".into())
                            .size_full()
                            .bg(gpui::rgb(0xff0000)),
                    )
                    .pane(
                        "middle",
                        SplitPane::new(80.0, 320.0),
                        div()
                            .debug_selector(|| "split-middle".into())
                            .size_full()
                            .bg(gpui::rgb(0x00ff00)),
                    )
                    .pane(
                        "right",
                        SplitPane::new(80.0, 320.0),
                        div()
                            .debug_selector(|| "split-right".into())
                            .size_full()
                            .bg(gpui::rgb(0x0000ff)),
                    )
                    .on_resize(
                        cx.listener(|harness: &mut Harness, state: &SplitState, _, cx| {
                            harness.state = state.clone();
                            cx.notify();
                        }),
                    )
                    .on_bounds(move |panes, _, _| {
                        *bounds.borrow_mut() = panes.to_vec();
                    }),
            )
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }

    fn harness(
        app: &mut TestAppContext,
        state: SplitState,
    ) -> (gpui::Entity<Harness>, &mut VisualTestContext) {
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        app.add_window_view(move |_, _| Harness {
            state,
            bounds: Rc::new(RefCell::new(Vec::new())),
        })
    }

    #[test]
    fn panes_fill_the_measured_extent_and_report_exact_bounds() {
        let mut app = TestAppContext::single();
        let (harness, cx) = harness(
            &mut app,
            SplitState::with_dividers([("left", 180.0), ("middle", 360.0)]),
        );
        cx.simulate_resize(size(px(600.), px(300.)));
        draw(cx);
        let left = cx.debug_bounds("split-left").expect("left");
        let middle = cx.debug_bounds("split-middle").expect("middle");
        let right = cx.debug_bounds("split-right").expect("right");
        assert_eq!(left.size.width, px(180.));
        assert_eq!(middle.origin.x, px(181.));
        assert_eq!(middle.size.width, px(179.));
        assert_eq!(right.origin.x, px(361.));
        assert_eq!(right.size.width, px(239.));
        assert_eq!(right.origin.x + right.size.width, px(600.));
        let reported = cx.update(|_, cx| harness.read(cx).bounds.borrow().clone());
        assert_eq!(reported.len(), 3);
        assert_eq!(reported[0], left);
        assert_eq!(reported[1], middle);
        assert_eq!(reported[2], right);
    }

    struct HorizontalHarness {
        state: SplitState,
    }

    impl Render for HorizontalHarness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                SplitView::new("split-horizontal", SplitOrientation::Horizontal)
                    .divider_style(SplitDividerStyle::Thick)
                    .state(self.state.clone())
                    .pane(
                        "top",
                        SplitPane::new(80.0, 400.0),
                        div()
                            .debug_selector(|| "split-top".into())
                            .size_full()
                            .bg(gpui::rgb(0xff0000)),
                    )
                    .pane(
                        "bottom",
                        SplitPane::new(80.0, f32::INFINITY),
                        div()
                            .debug_selector(|| "split-bottom".into())
                            .size_full()
                            .bg(gpui::rgb(0x0000ff)),
                    ),
            )
        }
    }

    #[test]
    fn horizontal_thick_divider_keeps_its_measured_height() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (_harness, cx) = app.add_window_view(|_, _| HorizontalHarness {
            state: SplitState::with_dividers([("top", 200.0)]),
        });
        cx.simulate_resize(size(px(300.), px(600.)));
        draw(cx);
        let top = cx.debug_bounds("split-top").expect("top");
        let bottom = cx.debug_bounds("split-bottom").expect("bottom");
        let divider = cx
            .debug_bounds("split-horizontal-divider-0")
            .expect("divider");
        assert_eq!(top.size.height, px(200.));
        assert_eq!(divider.origin.y, px(200.));
        assert_eq!(divider.size.height, px(9.));
        assert_eq!(bottom.origin.y, px(209.));
        assert_eq!(bottom.size.height, px(391.));
    }

    #[test]
    fn divider_drag_updates_the_controlled_state_and_respects_minimums() {
        let mut app = TestAppContext::single();
        let (harness, cx) = harness(
            &mut app,
            SplitState::with_dividers([("left", 180.0), ("middle", 360.0)]),
        );
        cx.simulate_resize(size(px(600.), px(300.)));
        draw(cx);
        let divider = cx.debug_bounds("split-divider-0").expect("divider");
        let start = divider.center();
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        // The first move crosses the drag threshold and is consumed by the
        // drag starter; the next move carries the resize delta.
        cx.simulate_mouse_move(
            point(start.x + px(20.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(start.x + px(60.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(
            cx.update(|_, cx| harness.read(cx).state.position("left")),
            Some(240.0)
        );
        cx.simulate_mouse_move(
            point(start.x - px(500.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(
            cx.update(|_, cx| harness.read(cx).state.position("left")),
            Some(80.0)
        );
        cx.simulate_mouse_up(
            point(start.x - px(500.), start.y),
            MouseButton::Left,
            Modifiers::default(),
        );
    }

    #[test]
    fn keyboard_focus_moves_the_divider() {
        let mut app = TestAppContext::single();
        let (harness, cx) = harness(
            &mut app,
            SplitState::with_dividers([("left", 180.0), ("middle", 360.0)]),
        );
        cx.simulate_resize(size(px(600.), px(300.)));
        draw(cx);
        let divider = cx.debug_bounds("split-divider-0").expect("divider");
        cx.simulate_click(divider.center(), Modifiers::default());
        draw(cx);
        cx.simulate_keystrokes("right");
        draw(cx);
        assert_eq!(
            cx.update(|_, cx| harness.read(cx).state.position("left")),
            Some(190.0)
        );
        cx.simulate_keystrokes("home");
        draw(cx);
        assert_eq!(
            cx.update(|_, cx| harness.read(cx).state.position("left")),
            Some(80.0)
        );
    }
}
