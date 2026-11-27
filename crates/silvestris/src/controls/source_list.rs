//! Virtualized source list (#128/#130; handbook 09 section C).
//! Owns presentation state only. Consumers subscribe to selection/expansion
//! events and retain ownership of domain actions and persistence.
use super::scroll_view::ScrollView;
use super::Disclosure;
use crate::{convert::hsla, paint, ActiveTheme};
use gpui::{
    canvas, div, prelude::*, px, uniform_list, Context, EventEmitter, FocusHandle, Focusable,
    MouseButton, Render, ScrollStrategy, UniformListScrollHandle, Window,
};
use silvestris_core::source_list::{Navigation, SourceNode, SourceRow, SourceTree};
use std::rc::Rc;

type IconRenderer = Rc<dyn Fn(&str, bool, bool, &mut Window, &mut gpui::App) -> gpui::AnyElement>;
type AccessoryRenderer = Rc<dyn Fn(&SourceRow, &mut Window, &mut gpui::App) -> gpui::AnyElement>;
type AccessoryModel = Rc<dyn Fn(&SourceRow) -> Vec<RowAccessory>>;
type AccessoryAction = Rc<dyn Fn(&mut Window, &mut gpui::App)>;
type RowDecorator =
    Rc<dyn Fn(&SourceRow, gpui::AnyElement, &mut Window, &mut gpui::App) -> gpui::AnyElement>;

/// Primary blue-grey sidebar or secondary document-integrated list
/// (HIG p. 208; M: sidebar.background and SL/mail-inbox.png).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SourceListStyle {
    #[default]
    Primary,
    Secondary,
}

/// Semantic status for [`RowAccessory::StatusDot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowStatus {
    Running,
    Paused,
    Error,
    Idle,
}

/// A trailing row accessory. Consumers choose one; they do not paint their own
/// badge, dot or action button (issue #4).
#[derive(Clone)]
pub enum RowAccessory {
    /// Count badge, e.g. Mail's unread count.
    Badge(usize),
    /// Eject glyph for a removable device.
    Eject,
    /// Small asynchronous spinner.
    Spinner,
    /// Semantic status dot; the theme maps the role to a colour.
    StatusDot(RowStatus),
    /// A subtle icon action. It stops row selection and drag events.
    Button {
        id: gpui::SharedString,
        label: gpui::SharedString,
        glyph: gpui::SharedString,
        action: AccessoryAction,
    },
    /// Reveal the inner accessory only while the pointer is over the row.
    Hover(Box<RowAccessory>),
}

impl RowAccessory {
    pub fn badge(count: usize) -> Self {
        Self::Badge(count)
    }

    pub fn status(status: RowStatus) -> Self {
        Self::StatusDot(status)
    }

    pub fn button(
        id: impl Into<gpui::SharedString>,
        label: impl Into<gpui::SharedString>,
        glyph: impl Into<gpui::SharedString>,
        action: impl Fn(&mut Window, &mut gpui::App) + 'static,
    ) -> Self {
        Self::Button {
            id: id.into(),
            label: label.into(),
            glyph: glyph.into(),
            action: Rc::new(action),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceListEvent {
    Selected(String),
    Expanded { id: String, expanded: bool },
}

pub struct SourceList {
    tree: SourceTree,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
    style: SourceListStyle,
    icons: Option<IconRenderer>,
    accessory: Option<AccessoryRenderer>,
    accessories: Option<AccessoryModel>,
    decorate: Option<RowDecorator>,
}

impl EventEmitter<SourceListEvent> for SourceList {}
impl Focusable for SourceList {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SourceList {
    pub fn new(nodes: Vec<SourceNode>, cx: &mut Context<Self>) -> Result<Self, String> {
        let mut tree = SourceTree::default();
        tree.replace(nodes)?;
        Ok(Self {
            tree,
            focus: cx.focus_handle().tab_stop(true),
            scroll: UniformListScrollHandle::new(),
            style: SourceListStyle::Primary,
            icons: None,
            accessory: None,
            accessories: None,
            decorate: None,
        })
    }

    /// Primary blue-grey sidebar or secondary document-integrated list.
    pub fn style(&mut self, style: SourceListStyle, cx: &mut Context<Self>) {
        if self.style != style {
            self.style = style;
            cx.notify();
        }
    }

    /// Model-driven accessories. The closure runs per visible row; the list
    /// renders each returned accessory with the standard hit area, focus and
    /// event isolation.
    pub fn set_row_accessories(
        &mut self,
        render: impl Fn(&SourceRow) -> Vec<RowAccessory> + 'static,
        cx: &mut Context<Self>,
    ) {
        self.accessories = Some(Rc::new(render));
        cx.notify();
    }

    pub fn state(&self) -> &SourceTree {
        &self.tree
    }

    /// Trailing app actions. Their pointer handlers must consume activation
    /// when selecting the row would be an unintended side effect.
    pub fn set_row_accessory(
        &mut self,
        render: impl Fn(&SourceRow, &mut Window, &mut gpui::App) -> gpui::AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.accessory = Some(Rc::new(render));
        cx.notify();
    }

    /// Decorate the row with application drag/context handlers. Preserve its
    /// height so viewport virtualization remains valid. Disclosure consumes
    /// mouse-down/click separately from these enclosing row handlers.
    pub fn set_row_decorator(
        &mut self,
        render: impl Fn(&SourceRow, gpui::AnyElement, &mut Window, &mut gpui::App) -> gpui::AnyElement
            + 'static,
        cx: &mut Context<Self>,
    ) {
        self.decorate = Some(Rc::new(render));
        cx.notify();
    }

    /// Resolves semantic icon roles to original multicolour art. Called only
    /// for visible rows; the renderer owns asset caching. Booleans are selected
    /// and disabled. Unresolved roles retain an empty alignment slot.
    pub fn set_icon_renderer(
        &mut self,
        render: impl Fn(&str, bool, bool, &mut Window, &mut gpui::App) -> gpui::AnyElement + 'static,
        cx: &mut Context<Self>,
    ) {
        self.icons = Some(Rc::new(render));
        cx.notify();
    }

    /// Model synchronization does not invoke consumer selection handlers.
    pub fn replace(
        &mut self,
        nodes: Vec<SourceNode>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let before = self.tree.rows_snapshot();
        self.tree.replace(nodes)?;
        if !std::sync::Arc::ptr_eq(&before, &self.tree.rows_snapshot()) {
            cx.notify();
        }
        Ok(())
    }

    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.tree.select(id) {
            self.reveal();
            cx.notify();
        }
    }

    pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
        if self.tree.clear_selection() {
            cx.notify();
        }
    }

    pub fn set_expanded(&mut self, id: &str, expanded: bool, cx: &mut Context<Self>) {
        if self.tree.expand(id, expanded) {
            cx.notify();
        }
    }

    fn reveal(&self) {
        if let Some(i) = self
            .tree
            .rows()
            .iter()
            .position(|r| Some(r.id.as_str()) == self.tree.selected())
        {
            self.scroll.scroll_to_item(i, ScrollStrategy::Nearest);
        }
    }

    fn navigate(&mut self, n: Navigation, cx: &mut Context<Self>) {
        let before = self.tree.selected().map(str::to_owned);
        let expanded_before = self
            .tree
            .rows()
            .iter()
            .find(|r| Some(r.id.as_str()) == self.tree.selected())
            .map(|r| (r.id.clone(), r.expanded));
        self.tree.navigate(n);
        if let Some((id, expanded)) = expanded_before {
            let now = self
                .tree
                .rows()
                .iter()
                .find(|r| r.id == id)
                .is_some_and(|r| r.expanded);
            if expanded != now {
                cx.emit(SourceListEvent::Expanded { id, expanded: now });
            }
        }
        if self.tree.selected() != before.as_deref() {
            if let Some(id) = self.tree.selected() {
                cx.emit(SourceListEvent::Selected(id.to_owned()));
            }
        }
        self.reveal();
        cx.notify();
    }
}

fn accessory_element(
    accessory: &RowAccessory,
    group: &gpui::SharedString,
    selected: bool,
    theme: &silvestris_core::Theme,
    window: &mut Window,
    cx: &mut gpui::App,
) -> gpui::AnyElement {
    let ink = if selected {
        theme.lists.source_active.text
    } else {
        theme.colors.text_secondary
    };
    let reveal = |el: gpui::AnyElement| -> gpui::AnyElement {
        div()
            .flex_none()
            .opacity(0.55)
            .group_hover(group.clone(), |s| s.opacity(1.0))
            .child(el)
            .into_any_element()
    };
    match accessory {
        RowAccessory::Badge(count) => div()
            .flex_none()
            .min_w(px(16.))
            .h(px(14.))
            .px(px(4.))
            .rounded_full()
            .bg(hsla(if selected {
                theme.lists.source_active.text
            } else {
                theme.colors.divider
            }))
            .text_color(hsla(if selected {
                theme.lists.source_active.fill.from
            } else {
                theme.colors.text
            }))
            .text_size(px(theme.type_.size_mini))
            .flex()
            .items_center()
            .justify_center()
            .child(count.to_string())
            .into_any_element(),
        RowAccessory::Eject => div()
            .flex_none()
            .w(px(16.))
            .h(px(16.))
            .flex()
            .items_center()
            .justify_center()
            .text_size(px(11.))
            .text_color(hsla(ink))
            .child("\u{23CF}")
            .into_any_element(),
        RowAccessory::Spinner => super::ProgressIndicator::spinner(
            gpui::ElementId::Name(gpui::SharedString::from(format!("spinner-{}", group))),
            "Working",
        )
        .size(super::ControlSize::Mini)
        .into_any_element(),
        RowAccessory::StatusDot(status) => {
            let colour = match status {
                RowStatus::Running => theme.colors.success,
                RowStatus::Paused => theme.colors.warning,
                RowStatus::Error => theme.colors.error,
                RowStatus::Idle => theme.colors.text_disabled,
            };
            div()
                .flex_none()
                .size(px(7.))
                .rounded_full()
                .bg(hsla(colour))
                .into_any_element()
        }
        RowAccessory::Hover(inner) => {
            let child = accessory_element(inner, group, selected, theme, window, cx);
            div()
                .flex_none()
                .opacity(0.0)
                .group_hover(group.clone(), |s| s.opacity(1.0))
                .child(child)
                .into_any_element()
        }
        RowAccessory::Button {
            id,
            label,
            glyph,
            action,
        } => {
            let focus = window
                .use_keyed_state(
                    gpui::ElementId::Name(gpui::SharedString::from(format!(
                        "acc-focus-{group}-{id}"
                    ))),
                    cx,
                    |_, cx| cx.focus_handle().tab_stop(true),
                )
                .read(cx)
                .clone();
            let click_focus = focus.clone();
            let key_focus = focus.clone();
            let key_action = action.clone();
            let click_action = action.clone();
            reveal(
                div()
                    .id(gpui::SharedString::from(format!("acc-{group}-{id}")))
                    .debug_selector({
                        let name = format!("acc-{group}-{id}");
                        move || name.clone()
                    })
                    .role(gpui::Role::Button)
                    .aria_label(label.clone())
                    .occlude()
                    .flex_none()
                    .size(px(16.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .track_focus(&focus)
                    .text_size(px(11.))
                    .text_color(hsla(ink))
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.focus(&click_focus, cx);
                        cx.stop_propagation();
                    })
                    .on_click(move |_, window, cx| {
                        cx.stop_propagation();
                        click_action(window, cx);
                    })
                    .on_key_down(move |event: &gpui::KeyDownEvent, window, cx| {
                        if !key_focus.is_focused(window) || event.keystroke.modifiers.modified() {
                            return;
                        }
                        if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                            cx.stop_propagation();
                            key_action(window, cx);
                        }
                    })
                    .child(glyph.clone())
                    .into_any_element(),
            )
        }
    }
}

impl Render for SourceList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let style = self.style;
        let active = window.is_window_active() && self.focus.contains_focused(window, cx);
        let selected = self.tree.selected().map(str::to_owned);
        // Share cached rows across repaints; row elements only for viewport.
        let rows = self.tree.rows_snapshot();
        let view = cx.entity();
        let icons = self.icons.clone();
        let accessory = self.accessory.clone();
        let accessories = self.accessories.clone();
        let decorate = self.decorate.clone();
        let bg_theme = theme.clone();
        let list = uniform_list("rows", rows.len(), move |range, window, cx| {
            range
                .map(|i| {
                    let row = &rows[i];
                    if row.section {
                        let toggle_view = view.clone();
                        let toggle_id = row.id.clone();
                        return div()
                            .id(gpui::SharedString::from(row.id.clone()))
                            .debug_selector(|| format!("source-section-{}", row.id))
                            .role(gpui::Role::Heading)
                            .aria_label(row.label.clone())
                            .aria_expanded(row.expanded)
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .w_full()
                            .h(px(theme.metrics.source_row_height))
                            .pl(px(4.))
                            .font_family(theme.type_.ui)
                            .font_weight(crate::theme::heading_font_weight(&theme))
                            .text_size(px(theme.type_.size_heading))
                            .text_color(hsla(match style {
                                SourceListStyle::Primary => theme.colors.sidebar_heading,
                                SourceListStyle::Secondary => theme.colors.source_secondary_heading,
                            }))
                            .child(if row.branch {
                                Disclosure::new(
                                    gpui::SharedString::from(format!("disclose-{}", row.id)),
                                    row.label.clone(),
                                )
                                .expanded(row.expanded)
                                .selected(false)
                                .on_toggle(move |expanded, _, cx| {
                                    toggle_view.update(cx, |list, cx| {
                                        if list.tree.expand(&toggle_id, *expanded) {
                                            cx.emit(SourceListEvent::Expanded {
                                                id: toggle_id.clone(),
                                                expanded: *expanded,
                                            });
                                            cx.notify();
                                        }
                                    });
                                })
                                .into_any_element()
                            } else {
                                div()
                                    .flex_none()
                                    .size(px(theme.metrics.disclosure_size))
                                    .into_any_element()
                            })
                            .child(div().min_w_0().truncate().child(row.label.to_uppercase()))
                            .into_any_element();
                    }
                    let is_selected = selected.as_deref() == Some(row.id.as_str());
                    let window_active = window.is_window_active();
                    let selection = match (style, window_active, active) {
                        (SourceListStyle::Primary, false, _) => theme.lists.source_inactive,
                        (SourceListStyle::Primary, true, true) => theme.lists.source_active,
                        (SourceListStyle::Primary, true, false) => theme.lists.source_unfocused,
                        (SourceListStyle::Secondary, false, _) => {
                            theme.lists.source_secondary_inactive
                        }
                        (SourceListStyle::Secondary, true, true) => {
                            theme.lists.source_secondary_active
                        }
                        (SourceListStyle::Secondary, true, false) => {
                            theme.lists.source_secondary_inactive
                        }
                    };
                    let text = if row.disabled {
                        theme.colors.text_disabled
                    } else if is_selected {
                        selection.text
                    } else {
                        theme.colors.text
                    };
                    let id = row.id.clone();
                    let click_view = view.clone();
                    let toggle_view = view.clone();
                    let toggle_id = id.clone();
                    let element = div()
                        .id(gpui::SharedString::from(id.clone()))
                        .debug_selector(|| format!("source-row-{}", row.id))
                        .role(gpui::Role::TreeItem)
                        .aria_label(row.label.clone())
                        .aria_selected(is_selected)
                        .when(row.branch, |el| el.aria_expanded(row.expanded))
                        .relative()
                        .flex()
                        .items_center()
                        .w_full()
                        .h(px(theme.metrics.source_row_height))
                        .font_family(theme.type_.ui)
                        .text_size(px(theme.type_.size_view))
                        .text_color(hsla(text))
                        .when(is_selected, |el| {
                            el.child(
                                canvas(
                                    |_, _, _| (),
                                    move |bounds, _, window, _| {
                                        paint::selection(window, bounds, &selection)
                                    },
                                )
                                .absolute()
                                .inset_0(),
                            )
                        })
                        .child(
                            div()
                                .relative()
                                .flex()
                                .items_center()
                                .min_w_0()
                                .w_full()
                                .pl(px(row.depth as f32 * theme.metrics.source_indent + 4.))
                                .pr(px(6.))
                                .gap(px(4.))
                                .child(if row.branch {
                                    Disclosure::new(
                                        gpui::SharedString::from(format!("disclose-{}", row.id)),
                                        row.label.clone(),
                                    )
                                    .expanded(row.expanded)
                                    .disabled(row.disabled)
                                    .selected(is_selected)
                                    .on_toggle(move |expanded, _, cx| {
                                        toggle_view.update(cx, |list, cx| {
                                            if list.tree.expand(&toggle_id, *expanded) {
                                                cx.emit(SourceListEvent::Expanded {
                                                    id: toggle_id.clone(),
                                                    expanded: *expanded,
                                                });
                                                cx.notify();
                                            }
                                        });
                                    })
                                    .into_any_element()
                                } else {
                                    div()
                                        .flex_none()
                                        .size(px(theme.metrics.disclosure_size))
                                        .into_any_element()
                                })
                                .when(icons.is_some(), |el| {
                                    el.child(div().flex_none().size(px(16.)).when_some(
                                        row.icon.as_ref(),
                                        |el, role| {
                                            el.child(icons.as_ref().unwrap()(
                                                role,
                                                is_selected,
                                                row.disabled,
                                                window,
                                                cx,
                                            ))
                                        },
                                    ))
                                })
                                .child(div().flex_1().min_w_0().truncate().child(row.label.clone()))
                                .when_some(row.badge.as_ref(), |el, badge| {
                                    el.child(
                                        div()
                                            .flex_none()
                                            .text_size(px(theme.type_.size_small))
                                            .child(badge.clone()),
                                    )
                                })
                                .when_some(accessory.as_ref(), |el, render| {
                                    el.child(div().flex_none().child(render(row, window, cx)))
                                })
                                .when_some(accessories.as_ref(), |el, render| {
                                    let group =
                                        gpui::SharedString::from(format!("accessories-{}", row.id));
                                    el.child(
                                        div()
                                            .flex_none()
                                            .flex()
                                            .items_center()
                                            .gap(px(4.))
                                            .group(group.clone())
                                            .children(render(row).into_iter().map(|item| {
                                                accessory_element(
                                                    &item,
                                                    &group,
                                                    is_selected,
                                                    &theme,
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    )
                                }),
                        )
                        .when(row.selectable(), |el| {
                            el.on_click(move |_, window, cx| {
                                cx.stop_propagation();
                                click_view.update(cx, |list, cx| {
                                    window.focus(&list.focus, cx);
                                    if list.tree.select(&id) {
                                        cx.emit(SourceListEvent::Selected(id.clone()));
                                        cx.notify();
                                    }
                                });
                            })
                        })
                        .into_any_element();
                    match &decorate {
                        Some(render) => render(row, element, window, cx),
                        None => element,
                    }
                })
                .collect::<Vec<_>>()
        })
        .track_scroll(&self.scroll)
        .size_full();
        div()
            .id("source-list")
            .role(gpui::Role::Tree)
            .aria_label("Navigation")
            .key_context("SilvestrisSourceList")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .bg(hsla(match (style, window.is_window_active()) {
                (SourceListStyle::Primary, true) => bg_theme.colors.sidebar,
                (SourceListStyle::Primary, false) => bg_theme.colors.sidebar_inactive,
                (SourceListStyle::Secondary, true) => bg_theme.colors.source_secondary,
                (SourceListStyle::Secondary, false) => bg_theme.colors.sidebar_inactive,
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|list, _, window, cx| {
                    window.focus(&list.focus, cx);
                    cx.stop_propagation();
                }),
            )
            .on_key_down(cx.listener(|list, event: &gpui::KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let n = match event.keystroke.key.as_str() {
                    "up" => Navigation::Up,
                    "down" => Navigation::Down,
                    "home" => Navigation::Home,
                    "end" => Navigation::End,
                    "left" => Navigation::Left,
                    "right" => Navigation::Right,
                    _ => return,
                };
                cx.stop_propagation();
                list.navigate(n, cx);
            }))
            .when(self.tree.rows().is_empty(), |el| {
                el.child(div().p(px(12.)).child("No items"))
            })
            .when(!self.tree.rows().is_empty(), |el| {
                // The panel's sidebar shows a scroller when its groups overflow
                // (4.15.47 AM capture).
                el.child(ScrollView::new("source-list-scroll", &self.scroll).child(list))
            })
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    use crate::{ThemeId, ThemePreference};
    use gpui::{point, Modifiers, TestAppContext, VisualTestContext};

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
    }

    // GPUI simulate_keystrokes dispatches key-down only; button activation
    // in div.rs requires the matching key-up with unchanged focus generation.
    fn press(cx: &mut VisualTestContext, key: &str) {
        let stroke =
            cx.update(|_, cx| gpui::Keystroke::parse_with_os(key, cx.platform_info().os).unwrap());
        cx.simulate_event(gpui::KeyDownEvent {
            keystroke: stroke.clone(),
            is_held: false,
            prefer_character_input: false,
        });
        cx.simulate_event(gpui::KeyUpEvent { keystroke: stroke });
    }

    fn selected(list: &gpui::Entity<SourceList>, cx: &mut VisualTestContext) -> Option<String> {
        cx.update(|_, cx| list.read(cx).tree.selected().map(str::to_owned))
    }

    #[test]
    fn rendered_disclosure_does_not_select_row_in_any_theme() {
        for theme in ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(ThemePreference::explicit(theme), cx));
            let (list, cx) = app.add_window_view(|_, cx| {
                let mut root = SourceNode::new("root", "Collection");
                root.children = vec![SourceNode::new("child", "Document")];
                SourceList::new(vec![root, SourceNode::new("other", "Other")], cx).unwrap()
            });
            draw(cx);
            let other = cx.debug_bounds("source-row-other").expect("rendered row");
            cx.simulate_click(
                point(other.origin.x + px(80.), other.center().y),
                Modifiers::default(),
            );
            draw(cx);
            assert_eq!(selected(&list, cx).as_deref(), Some("other"));
            let root = cx.debug_bounds("source-row-root").unwrap();
            // Row inset + centre of the 13 px disclosure, independent of font.
            cx.simulate_click(
                point(root.origin.x + px(10.), root.center().y),
                Modifiers::default(),
            );
            draw(cx);
            assert_eq!(selected(&list, cx).as_deref(), Some("other"), "{theme:?}");
            cx.update(|_, cx| assert!(list.read(cx).tree.rows()[0].expanded));
            assert!(cx.debug_bounds("source-row-child").is_some());
            press(cx, "space");
            draw(cx);
            assert_eq!(selected(&list, cx).as_deref(), Some("other"));
            cx.update(|_, cx| assert!(!list.read(cx).tree.rows()[0].expanded));
            press(cx, "enter");
            draw(cx);
            cx.update(|_, cx| assert!(list.read(cx).tree.rows()[0].expanded));
        }
    }

    #[test]
    fn keyboard_navigation_survives_live_theme_switch() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(ThemePreference::explicit(ThemeId::AQUA_LIGHT), cx));
        let (list, cx) = app.add_window_view(|window, cx| {
            let mut root = SourceNode::new("root", "Collection");
            root.children = vec![SourceNode::new("child", "Document")];
            let list = SourceList::new(vec![root], cx).unwrap();
            window.focus(&list.focus, cx);
            list
        });
        draw(cx);
        cx.simulate_keystrokes("home right right");
        draw(cx);
        assert_eq!(selected(&list, cx).as_deref(), Some("child"));
        for theme in ThemeId::ALL {
            cx.update(|_, cx| crate::set_theme_preference(ThemePreference::explicit(theme), cx));
            draw(cx);
            assert_eq!(selected(&list, cx).as_deref(), Some("child"));
            cx.update(|window, cx| assert!(list.read(cx).focus.is_focused(window)));
        }
        cx.simulate_keystrokes("left left");
        draw(cx);
        assert_eq!(selected(&list, cx).as_deref(), Some("root"));
        cx.update(|_, cx| assert!(!list.read(cx).tree.rows()[0].expanded));
    }

    #[test]
    fn large_list_renders_viewport_and_scrolls_to_keyboard_selection() {
        use std::cell::Cell;
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(ThemePreference::default(), cx));
        let rendered = Rc::new(Cell::new(0));
        let (list, cx) = app.add_window_view(|window, cx| {
            let nodes = (0..10000)
                .map(|i| {
                    let mut n = SourceNode::new(format!("item-{i}"), format!("Document {i}"));
                    n.icon = Some("document".into());
                    n
                })
                .collect();
            let mut list = SourceList::new(nodes, cx).unwrap();
            let count = rendered.clone();
            list.set_icon_renderer(
                move |_, _, _, _, _| {
                    count.set(count.get() + 1);
                    div().into_any_element()
                },
                cx,
            );
            window.focus(&list.focus, cx);
            list
        });
        draw(cx);
        assert!(
            rendered.get() > 0 && rendered.get() < 1000,
            "only viewport icons should render: {}",
            rendered.get()
        );
        rendered.set(0);
        cx.simulate_keystrokes("end");
        draw(cx);
        assert_eq!(selected(&list, cx).as_deref(), Some("item-9999"));
        assert!(cx.debug_bounds("source-row-item-9999").is_some());
        assert!(rendered.get() < 1000, "scroll should not render every row");
        cx.simulate_keystrokes("home");
        draw(cx);
        assert_eq!(selected(&list, cx).as_deref(), Some("item-0"));
        assert!(cx.debug_bounds("source-row-item-0").is_some());
    }
    #[test]
    fn row_accessories_and_context_decorators_preserve_disclosure_and_selection() {
        use std::cell::{Cell, RefCell};
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(ThemePreference::default(), cx));
        let hits = Rc::new(Cell::new(0));
        let context_row = Rc::new(RefCell::new(String::new()));
        let drag_down = Rc::new(Cell::new(0));
        let (list, cx) = app.add_window_view(|_, cx| {
            let mut root = SourceNode::new("root", "Offline Card");
            root.selectable = false;
            root.children.push(SourceNode::new("child", "Copy"));
            let mut list =
                SourceList::new(vec![root, SourceNode::new("other", "Other")], cx).unwrap();
            list.select("other", cx);
            let count = hits.clone();
            list.set_row_accessory(
                move |row, _, _| {
                    let count = count.clone();
                    div()
                        .id("action")
                        .debug_selector(|| format!("action-{}", row.id))
                        .size(px(12.))
                        .on_click(move |_, _, cx| {
                            cx.stop_propagation();
                            count.set(count.get() + 1);
                        })
                        .into_any_element()
                },
                cx,
            );
            let context_row = context_row.clone();
            let drag_down = drag_down.clone();
            list.set_row_decorator(
                move |row, element, _, _| {
                    let id = row.id.clone();
                    let target = context_row.clone();
                    let count = drag_down.clone();
                    div()
                        .id(gpui::SharedString::from(format!("wrapped-{}", row.id)))
                        .child(element)
                        .on_mouse_down(MouseButton::Right, move |_, _, cx| {
                            *target.borrow_mut() = id.clone();
                            cx.stop_propagation();
                        })
                        .on_mouse_down(MouseButton::Left, move |_, _, _| count.set(count.get() + 1))
                        .into_any_element()
                },
                cx,
            );
            list
        });
        draw(cx);
        let action = cx.debug_bounds("action-root").unwrap();
        cx.simulate_click(action.center(), Modifiers::default());
        draw(cx);
        assert_eq!(hits.get(), 1);
        assert_eq!(selected(&list, cx).as_deref(), Some("other"));
        let row = cx.debug_bounds("source-row-root").unwrap();
        cx.simulate_mouse_down(
            point(row.left() + px(60.), row.center().y),
            MouseButton::Right,
            Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(row.left() + px(60.), row.center().y),
            MouseButton::Right,
            Modifiers::default(),
        );
        assert_eq!(context_row.borrow().as_str(), "root");
        drag_down.set(0);
        draw(cx);
        cx.simulate_click(
            point(row.left() + px(10.), row.center().y),
            Modifiers::default(),
        );
        draw(cx);
        assert_eq!(
            drag_down.get(),
            0,
            "disclosure must not start an enclosing drag"
        );
        assert!(cx.debug_bounds("source-row-child").is_some());
        assert_eq!(selected(&list, cx).as_deref(), Some("other"));
    }

    #[test]
    fn model_accessories_stay_isolated_and_secondary_style_keeps_selection() {
        use std::cell::Cell;
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let hits = Rc::new(Cell::new(0));
        let (list, cx) = app.add_window_view(|_, cx| {
            let mut list = SourceList::new(
                vec![
                    SourceNode::new("inbox", "Inbox"),
                    SourceNode::new("sent", "Sent"),
                ],
                cx,
            )
            .unwrap();
            list.style(SourceListStyle::Secondary, cx);
            list.select("inbox", cx);
            let count = hits.clone();
            list.set_row_accessories(
                move |row| {
                    let count = count.clone();
                    vec![
                        RowAccessory::badge(3),
                        RowAccessory::status(RowStatus::Running),
                        RowAccessory::button("act", "Act on row", "\u{2699}", move |_, _| {
                            count.set(count.get() + 1)
                        }),
                    ]
                    .into_iter()
                    .filter(|_| row.id == "inbox")
                    .collect()
                },
                cx,
            );
            list
        });
        draw(cx);
        let action = cx.debug_bounds("acc-accessories-inbox-act").unwrap();
        cx.simulate_click(action.center(), Modifiers::default());
        draw(cx);
        assert_eq!(hits.get(), 1);
        assert_eq!(selected(&list, cx).as_deref(), Some("inbox"));
        cx.update(|_, cx| assert_eq!(list.read(cx).style, SourceListStyle::Secondary));
    }
}
