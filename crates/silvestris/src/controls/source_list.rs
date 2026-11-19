//! Virtualized source list (#128/#130; handbook 09 section C).
//! Owns presentation state only. Consumers subscribe to selection/expansion
//! events and retain ownership of domain actions and persistence.
use super::Disclosure;
use crate::{convert::hsla, paint, ActiveTheme};
use gpui::{
    canvas, div, prelude::*, px, uniform_list, Context, EventEmitter, FocusHandle, Focusable,
    MouseButton, Render, ScrollStrategy, UniformListScrollHandle, Window,
};
use silvestris_core::source_list::{Navigation, SourceNode, SourceTree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceListEvent {
    Selected(String),
    Expanded { id: String, expanded: bool },
}

pub struct SourceList {
    tree: SourceTree,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
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
        })
    }

    pub fn state(&self) -> &SourceTree {
        &self.tree
    }

    /// Model synchronization does not invoke consumer selection handlers.
    pub fn replace(
        &mut self,
        nodes: Vec<SourceNode>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.tree.replace(nodes)?;
        cx.notify();
        Ok(())
    }

    pub fn select(&mut self, id: &str, cx: &mut Context<Self>) {
        if self.tree.select(id) {
            self.reveal();
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

impl Render for SourceList {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let active = window.is_window_active() && self.focus.contains_focused(window, cx);
        let selected = self.tree.selected().map(str::to_owned);
        // Share cached rows across repaints; row elements only for viewport.
        let rows = self.tree.rows_snapshot();
        let view = cx.entity();
        let list = uniform_list("rows", rows.len(), move |range, _, _| {
            range
                .map(|i| {
                    let row = &rows[i];
                    let is_selected = selected.as_deref() == Some(row.id.as_str());
                    let selection = if active {
                        theme.lists.source_active
                    } else {
                        theme.lists.source_inactive
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
                    div()
                        .id(gpui::SharedString::from(id.clone()))
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
                                .child(div().min_w_0().truncate().child(row.label.clone())),
                        )
                        .when(!row.disabled, |el| {
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
                        .into_any_element()
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
            .bg(hsla(if window.is_window_active() {
                cx.theme().colors.sidebar
            } else {
                cx.theme().colors.sidebar_inactive
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|list, _, window, cx| window.focus(&list.focus, cx)),
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
            .when(!self.tree.rows().is_empty(), |el| el.child(list))
    }
}
