//! Single-choice text segmented control (#128; HIG pp. 286-288).
//! Equal widths use the widest shaped label. Consumer owns selected ID.
//! A shared outer bezel is clipped per segment; internal edges stay square.
use crate::{
    convert::{hairline, hsla},
    paint, ActiveTheme,
};
use gpui::{
    canvas, div, point, prelude::*, px, size, App, Bounds, ContentMask, ElementId, SharedString,
    TextRun, Window,
};
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Segment {
    pub id: SharedString,
    pub label: SharedString,
    pub disabled: bool,
}
impl Segment {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
        }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

type SelectionHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    label: SharedString,
    segments: Vec<Segment>,
    selected: Option<SharedString>,
    disabled: bool,
    min_segment_width: f32,
    on_select: Option<SelectionHandler>,
}
impl SegmentedControl {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        segments: Vec<Segment>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            segments,
            selected: None,
            disabled: false,
            min_segment_width: 0.,
            on_select: None,
        }
    }
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    pub fn min_segment_width(mut self, width: f32) -> Self {
        self.min_segment_width = width.max(0.);
        self
    }
    pub fn on_select(mut self, f: impl Fn(&SharedString, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

fn destination(
    segments: &[Segment],
    selected: Option<&SharedString>,
    key: &str,
) -> Option<SharedString> {
    let enabled: Vec<_> = segments.iter().filter(|s| !s.disabled).collect();
    if enabled.is_empty() {
        return None;
    }
    let current = enabled.iter().position(|s| Some(&s.id) == selected);
    let i = match key {
        "home" => 0,
        "end" => enabled.len() - 1,
        "left" | "up" => current
            .map(|i| (i + enabled.len() - 1) % enabled.len())
            .unwrap_or(enabled.len() - 1),
        "right" | "down" => current.map(|i| (i + 1) % enabled.len()).unwrap_or(0),
        "enter" | "space" => current.unwrap_or(0),
        _ => return None,
    };
    Some(enabled[i].id.clone())
}

impl RenderOnce for SegmentedControl {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let t = cx.theme().clone();
        let focus = window
            .use_keyed_state(self.id.clone(), cx, |_, cx| {
                cx.focus_handle().tab_stop(true)
            })
            .read(cx)
            .clone();
        let width = self
            .segments
            .iter()
            .map(|s| {
                let run = TextRun {
                    len: s.label.len(),
                    font: gpui::font(t.type_.ui),
                    color: hsla(t.colors.text),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window
                    .text_system()
                    .shape_line(s.label.clone(), px(t.type_.size_regular), &[run], None)
                    .width
                    .as_f32()
                    + 2. * t.metrics.push_padding[0]
            })
            .fold(self.min_segment_width, f32::max);
        let count = self.segments.len();
        let enabled = !self.disabled && self.segments.iter().any(|s| !s.disabled);
        let selected = self.selected.clone();
        let keyboard_segments = self.segments.clone();
        let keyboard = self.on_select.clone();
        let ring_focus = focus.clone();
        let radius = t.metrics.segment_height / 2.;
        let ring = t.colors.focus_ring;
        let profile = t.metrics.focus_ring;
        let mut group = div()
            .id(self.id)
            .role(gpui::Role::Group)
            .aria_label(self.label)
            .relative()
            .flex()
            .flex_none()
            .h(px(t.metrics.segment_height))
            .when(enabled, |el| el.track_focus(&focus))
            .on_key_down(move |event, window, cx| {
                if !enabled || event.keystroke.modifiers.modified() {
                    return;
                }
                if let Some(id) =
                    destination(&keyboard_segments, selected.as_ref(), &event.keystroke.key)
                {
                    cx.stop_propagation();
                    if selected.as_ref() != Some(&id) {
                        if let Some(f) = &keyboard {
                            f(&id, window, cx);
                        }
                    }
                }
            });
        for (i, segment) in self.segments.into_iter().enumerate() {
            let is_selected = self.selected.as_ref() == Some(&segment.id);
            let active = enabled && !segment.disabled;
            let states = t.controls.segment;
            let base = if !active {
                states.disabled
            } else if is_selected {
                states.selected
            } else {
                states.normal
            };
            let pressed = states.pressed;
            let handler = self.on_select.clone();
            let click_focus = focus.clone();
            let paint_segment = move |bounds: Bounds<gpui::Pixels>, window: &mut Window, bevel| {
                let all = Bounds::new(
                    point(bounds.origin.x - px(i as f32 * width), bounds.origin.y),
                    size(px(count as f32 * width), bounds.size.height),
                );
                window.with_content_mask(Some(ContentMask { bounds }), |window| {
                    paint::bevel(window, all, radius, &bevel);
                    if i > 0 {
                        window.paint_quad(gpui::fill(
                            Bounds::new(bounds.origin, size(hairline(window), bounds.size.height)),
                            hsla(bevel.border.at(0.5)),
                        ));
                    }
                });
            };
            group = group.child(
                div()
                    .id(segment.id.clone())
                    .debug_selector(|| format!("segment-{}", segment.id))
                    .group("silvestris-segment")
                    .role(gpui::Role::Button)
                    // GPUI at this pin lacks aria_pressed; expose selection in name.
                    .aria_label(if is_selected {
                        format!("{}, selected", segment.label)
                    } else {
                        segment.label.to_string()
                    })
                    .relative()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .w(px(width))
                    .h(px(t.metrics.segment_height))
                    .font_family(t.type_.ui)
                    .text_size(px(t.type_.size_regular))
                    .text_color(hsla(base.text))
                    .child(
                        canvas(|_, _, _| (), move |b, _, w, _| paint_segment(b, w, base))
                            .absolute()
                            .inset_0(),
                    )
                    .when(active, |el| {
                        el.child(
                            div()
                                .id("pressed")
                                .absolute()
                                .inset_0()
                                .opacity(0.)
                                .group_active("silvestris-segment", |s| s.opacity(1.))
                                .child(
                                    canvas(
                                        |_, _, _| (),
                                        move |b, _, w, _| paint_segment(b, w, pressed),
                                    )
                                    .size_full(),
                                ),
                        )
                    })
                    .child(
                        div()
                            .id("label")
                            .relative()
                            .when(active, |el| {
                                el.group_active("silvestris-segment", |s| {
                                    s.text_color(hsla(pressed.text))
                                })
                            })
                            .child(segment.label),
                    )
                    .when(active, |el| {
                        el.on_click(move |_, window, cx| {
                            window.focus(&click_focus, cx);
                            cx.stop_propagation();
                            if !is_selected {
                                if let Some(f) = &handler {
                                    f(&segment.id, window, cx);
                                }
                            }
                        })
                    }),
            );
        }
        group.child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    if enabled && ring_focus.is_focused(window) && window.last_input_was_keyboard()
                    {
                        paint::focus_ring(window, bounds, radius, ring, profile);
                    }
                },
            )
            .absolute()
            .inset_0(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext};
    struct Harness {
        selected: SharedString,
        changes: usize,
        disabled: bool,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(
                SegmentedControl::new(
                    "layout",
                    "Display layout",
                    vec![
                        Segment::new("a", "Grid"),
                        Segment::new("b", "Unavailable").disabled(true),
                        Segment::new("c", "A Longer View Name"),
                    ],
                )
                .selected(self.selected.clone())
                .disabled(self.disabled)
                .on_select(cx.listener(|h, selected: &SharedString, _, cx| {
                    h.selected = selected.clone();
                    h.changes += 1;
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
    #[test]
    fn equal_widths_clicks_and_keyboard_skip_disabled_in_all_themes() {
        for theme in crate::ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(theme), cx));
            let (h, cx) = app.add_window_view(|_, _| Harness {
                selected: "a".into(),
                changes: 0,
                disabled: false,
            });
            draw(cx);
            let a = cx.debug_bounds("segment-a").unwrap();
            let b = cx.debug_bounds("segment-b").unwrap();
            let c = cx.debug_bounds("segment-c").unwrap();
            assert_eq!(a.size.width, b.size.width);
            assert_eq!(b.size.width, c.size.width);
            assert_eq!(a.right(), b.left());
            assert_eq!(b.right(), c.left());
            cx.simulate_click(b.center(), Modifiers::default());
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).changes, 0));
            cx.simulate_click(c.center(), Modifiers::default());
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).selected.as_ref(), "c"));
            cx.simulate_keystrokes("right");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).selected.as_ref(), "a"));
            cx.simulate_keystrokes("right");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).selected.as_ref(), "c"));
            cx.simulate_keystrokes("home end");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).selected.as_ref(), "c"));
            cx.simulate_keystrokes("ctrl-left");
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).selected.as_ref(), "c"));
            // Selecting an already selected segment does not dispatch again.
            let changes = cx.update(|_, cx| h.read(cx).changes);
            cx.simulate_click(c.center(), Modifiers::default());
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).changes, changes));
            cx.update(|_, cx| {
                h.update(cx, |h, cx| {
                    h.disabled = true;
                    cx.notify();
                })
            });
            draw(cx);
            cx.simulate_click(a.center(), Modifiers::default());
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).changes, changes));
        }
    }
    #[test]
    fn navigation_handles_no_selection_or_no_enabled_segments() {
        let segments = vec![
            Segment::new("a", "A").disabled(true),
            Segment::new("b", "B"),
        ];
        assert_eq!(destination(&segments, None, "right").as_deref(), Some("b"));
        assert_eq!(
            destination(&segments, Some(&"missing".into()), "left").as_deref(),
            Some("b")
        );
        assert_eq!(destination(&[], None, "right"), None);
        assert_eq!(
            destination(&[Segment::new("a", "A").disabled(true)], None, "home"),
            None
        );
        assert_eq!(destination(&segments, None, "escape"), None);
    }
}
