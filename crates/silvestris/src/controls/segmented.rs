//! Single-choice text segmented control (#128; HIG pp. 286-288).
//! Equal widths use the widest shaped label; `fit_labels` instead sizes
//! each segment to its label plus a margin (tab views, HIG p. 341). Consumer owns selected ID.
//! A shared outer bezel is clipped per segment; internal edges stay square.
use crate::{
    convert::{hairline, hsla},
    paint, ActiveTheme,
};
use gpui::{
    canvas, div, point, prelude::*, px, size, AnyElement, App, Bounds, ContentMask, ElementId,
    SharedString, TextRun, Window,
};
use silvestris_core::Rgba8;
use std::rc::Rc;

use super::ControlSize;

/// Selection semantics (HIG pp. 286-288).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentMode {
    /// Radio-like: one latched segment.
    #[default]
    SelectOne,
    /// Checkbox-like: the consumer owns a set and each click toggles one.
    SelectAny,
    /// Toolbar action: never latches; every click dispatches.
    Momentary,
}

/// Capsule (window body) or textured (toolbar) material.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SegmentStyle {
    #[default]
    Rounded,
    Textured,
}

#[derive(Clone)]
pub struct Segment {
    pub id: SharedString,
    pub label: SharedString,
    pub disabled: bool,
    pub icon: Option<SharedString>,
    /// Typed monochrome glyph rendered with the segment's current ink.
    pub glyph: Option<crate::toolbar::ToolbarGlyph>,
    pub show_label: bool,
}
impl Segment {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            icon: None,
            glyph: None,
            show_label: true,
        }
    }
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
    /// An icon role resolved by [`SegmentedControl::icon_renderer`]. With
    /// `show_label(false)` the segment is icon-only and square.
    pub fn icon(mut self, role: impl Into<SharedString>) -> Self {
        self.icon = Some(role.into());
        self
    }
    /// A typed monochrome glyph, global or application-provided. Rendered with
    /// the segment's current ink in the 13 px segmented slot.
    pub fn glyph(mut self, glyph: impl Into<crate::toolbar::ToolbarGlyph>) -> Self {
        self.glyph = Some(glyph.into());
        self
    }
    pub fn show_label(mut self, show: bool) -> Self {
        self.show_label = show;
        self
    }

    fn has_glyph(&self) -> bool {
        self.glyph.is_some() || self.icon.is_some()
    }
}

type SelectionHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type IconRenderer = Rc<dyn Fn(&str, Rgba8) -> AnyElement>;

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    label: SharedString,
    segments: Vec<Segment>,
    selected: Option<SharedString>,
    selected_many: Vec<SharedString>,
    mode: SegmentMode,
    style: SegmentStyle,
    size: ControlSize,
    disabled: bool,
    min_segment_width: f32,
    label_margin: Option<f32>,
    icon_renderer: Option<IconRenderer>,
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
            selected_many: Vec::new(),
            mode: SegmentMode::SelectOne,
            style: SegmentStyle::Rounded,
            size: ControlSize::Regular,
            disabled: false,
            min_segment_width: 0.,
            label_margin: None,
            icon_renderer: None,
            on_select: None,
        }
    }
    pub fn selected(mut self, id: impl Into<SharedString>) -> Self {
        self.selected = Some(id.into());
        self
    }
    /// The selected set for [`SegmentMode::SelectAny`].
    pub fn selected_many(mut self, ids: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.selected_many = ids.into_iter().map(Into::into).collect();
        self
    }
    pub fn mode(mut self, mode: SegmentMode) -> Self {
        self.mode = mode;
        self
    }
    pub fn style(mut self, style: SegmentStyle) -> Self {
        self.style = style;
        self
    }
    pub fn size(mut self, size: ControlSize) -> Self {
        self.size = size;
        self
    }
    pub fn icon_renderer(mut self, f: impl Fn(&str, Rgba8) -> AnyElement + 'static) -> Self {
        self.icon_renderer = Some(Rc::new(f));
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
    /// Size each segment to its own label plus `margin` on each side, as
    /// tab-view tabs do (HIG p. 341: 12 / 10 / 8 px).
    pub fn fit_labels(mut self, margin: f32) -> Self {
        self.label_margin = Some(margin.max(0.));
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
        let height = match self.size {
            ControlSize::Regular => t.metrics.segment_height,
            ControlSize::Small => t.metrics.push_small_height,
            ControlSize::Mini => t.metrics.push_mini_height,
        };
        let text_size = self.size.label_size(&t);
        let states = match self.style {
            SegmentStyle::Rounded => t.controls.segment,
            SegmentStyle::Textured => t.controls.textured,
        };
        let radius = match self.style {
            SegmentStyle::Rounded => t.metrics.segment_corner.radius(height),
            SegmentStyle::Textured => t.metrics.textured_radius.min(height / 2.0),
        };
        let labels: Vec<f32> = self
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
                    .shape_line(s.label.clone(), px(text_size), &[run], None)
                    .width
                    .as_f32()
            })
            .collect();
        let widths: Vec<f32> = match self.label_margin {
            Some(margin) => self
                .segments
                .iter()
                .zip(labels.iter())
                .map(|(segment, w)| {
                    if segment.has_glyph() && !segment.show_label {
                        height.max(self.min_segment_width)
                    } else {
                        let icon = if segment.has_glyph() { 16.0 } else { 0.0 };
                        (w + icon + 2. * margin).ceil().max(self.min_segment_width)
                    }
                })
                .collect(),
            None => {
                let w = self
                    .segments
                    .iter()
                    .zip(labels.iter())
                    .map(|(segment, w)| {
                        if segment.has_glyph() && !segment.show_label {
                            height
                        } else {
                            let icon = if segment.has_glyph() { 16.0 } else { 0.0 };
                            w + icon + 2. * t.metrics.push_padding[0]
                        }
                    })
                    .fold(self.min_segment_width, f32::max);
                self.segments
                    .iter()
                    .map(|segment| {
                        if segment.has_glyph() && !segment.show_label {
                            height.max(self.min_segment_width)
                        } else {
                            w
                        }
                    })
                    .collect()
            }
        };
        let total: f32 = widths.iter().sum();
        let enabled = !self.disabled && self.segments.iter().any(|s| !s.disabled);
        let mode = self.mode;
        let selected = self.selected.clone();
        let selected_many = self.selected_many.clone();
        let keyboard_segments = self.segments.clone();
        let keyboard_selected = selected.clone().or_else(|| selected_many.first().cloned());
        let keyboard = self.on_select.clone();
        let icon_renderer = self.icon_renderer.clone();
        let ring_focus = focus.clone();
        let ring = t.colors.focus_ring;
        let profile = t.metrics.focus_ring;
        let mut group = div()
            .debug_selector(|| self.id.to_string())
            .id(self.id)
            .role(gpui::Role::Group)
            .aria_label(self.label)
            .relative()
            .flex()
            .flex_none()
            .h(px(height))
            .when(enabled, |el| el.track_focus(&focus))
            .on_key_down(move |event, window, cx| {
                if !enabled || event.keystroke.modifiers.modified() {
                    return;
                }
                if let Some(id) = destination(
                    &keyboard_segments,
                    keyboard_selected.as_ref(),
                    &event.keystroke.key,
                ) {
                    cx.stop_propagation();
                    let latched =
                        mode == SegmentMode::SelectOne && keyboard_selected.as_ref() == Some(&id);
                    if !latched {
                        if let Some(f) = &keyboard {
                            f(&id, window, cx);
                        }
                    }
                }
            });
        for (i, segment) in self.segments.into_iter().enumerate() {
            let is_selected = match mode {
                SegmentMode::SelectOne => self.selected.as_ref() == Some(&segment.id),
                SegmentMode::SelectAny => self.selected_many.contains(&segment.id),
                SegmentMode::Momentary => false,
            };
            let active = enabled && !segment.disabled;
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
            let width = widths[i];
            let before: f32 = widths[..i].iter().sum();
            let paint_segment = move |bounds: Bounds<gpui::Pixels>, window: &mut Window, bevel| {
                let all = Bounds::new(
                    point(bounds.origin.x - px(before), bounds.origin.y),
                    size(px(total), bounds.size.height),
                );
                // One row taller than the segment so the emboss line under
                // the bezel survives the clip (M: segmented.normal: 5e5e5e
                // edge, then the b9b9b9 emboss row).
                let mut mask = bounds;
                mask.size.height += hairline(window);
                window.with_content_mask(Some(ContentMask { bounds: mask }), |window| {
                    paint::bevel(window, all, radius, &bevel);
                    if i > 0 {
                        window.paint_quad(gpui::fill(
                            Bounds::new(bounds.origin, size(hairline(window), bounds.size.height)),
                            hsla(bevel.border.at(0.5)),
                        ));
                    }
                });
            };
            let glyph = segment
                .glyph
                .as_ref()
                .map(|glyph| glyph.element(crate::icons::TemplateImageSlot::Segmented, base.text))
                .or_else(|| {
                    segment.icon.as_ref().and_then(|role| {
                        icon_renderer.as_ref().map(|render| render(role, base.text))
                    })
                });
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
                    .h(px(height))
                    .font_family(t.type_.ui)
                    .text_size(px(text_size))
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
                    .when_some(glyph, |el, glyph| {
                        el.child(
                            div()
                                .id("icon")
                                .relative()
                                .flex()
                                .flex_none()
                                .items_center()
                                .justify_center()
                                .size(px(16.0))
                                // A text glyph's line box is taller than the
                                // 16 px slot; pin it so the glyph centres.
                                .line_height(px(16.0))
                                .child(glyph),
                        )
                    })
                    .when(segment.show_label, |el| {
                        el.child(
                            div()
                                .id("label")
                                .relative()
                                .when(segment.has_glyph(), |el| el.ml(px(4.0)))
                                .when(active, |el| {
                                    el.group_active("silvestris-segment", |s| {
                                        s.text_color(hsla(pressed.text))
                                    })
                                })
                                .child(segment.label.clone()),
                        )
                    })
                    .when(active, |el| {
                        el.on_click(move |_, window, cx| {
                            window.focus(&click_focus, cx);
                            cx.stop_propagation();
                            let dispatch = match mode {
                                SegmentMode::SelectOne => !is_selected,
                                SegmentMode::SelectAny | SegmentMode::Momentary => true,
                            };
                            if dispatch {
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

    #[test]
    fn select_any_and_momentary_dispatch_every_click_and_never_latch() {
        struct ModeHarness {
            mode: SegmentMode,
            calls: usize,
        }
        impl Render for ModeHarness {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div().size_full().child(
                    SegmentedControl::new(
                        "modes",
                        "Modes",
                        vec![Segment::new("a", "A"), Segment::new("b", "B")],
                    )
                    .mode(self.mode)
                    .selected_many(["a"])
                    .on_select(cx.listener(|h, _, _, cx| {
                        h.calls += 1;
                        cx.notify();
                    })),
                )
            }
        }
        for mode in [SegmentMode::SelectAny, SegmentMode::Momentary] {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
            let (h, cx) = app.add_window_view(|_, _| ModeHarness { mode, calls: 0 });
            draw(cx);
            let a = cx.debug_bounds("segment-a").unwrap();
            let b = cx.debug_bounds("segment-b").unwrap();
            cx.simulate_click(a.center(), Modifiers::default());
            draw(cx);
            cx.simulate_click(b.center(), Modifiers::default());
            draw(cx);
            cx.update(|_, cx| assert_eq!(h.read(cx).calls, 2, "{mode:?}"));
        }
    }
}
