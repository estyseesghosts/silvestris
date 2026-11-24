//! Themed command popup (#129; handbook 12, HIG pp. 165-173).
//! Mount once as a deferred overlay. Every action goes through CommandRegistry.
use crate::{
    command::{CommandRegistry, InvokeCommand},
    convert::hsla,
    paint,
    responder::ResponderChain,
    transient::{self, SurfaceId, SurfaceSpec, TransientKind, TransientLayer},
    ActiveTheme,
};
use gpui::{
    anchored, canvas, deferred, div, point, prelude::*, px, size, Anchor, AnchoredPositionMode,
    AnyElement, App, Bounds, Context, Entity, FocusHandle, MouseButton, Pixels, Point, Render,
    Window,
};
use silvestris_core::command::{display_shortcut, CommandId, MenuEntry, MenuModel, Platform};

/// Project the command model to GPUI's native application menu. GPUI currently
/// provides this surface on macOS; other consumers can use the same models
/// as submenu entries in a PopupMenu.
pub fn native_menus(
    models: &[MenuModel],
    descriptors: &silvestris_core::command::CommandSet,
) -> Vec<gpui::Menu> {
    fn items(
        entries: &[MenuEntry],
        descriptors: &silvestris_core::command::CommandSet,
    ) -> Vec<gpui::MenuItem> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Command(id) => {
                    descriptors
                        .get(id)
                        .map(|descriptor| gpui::MenuItem::Action {
                            name: descriptor.menu_label().into(),
                            action: Box::new(InvokeCommand {
                                id: id.as_str().into(),
                            }),
                            os_action: None,
                            checked: descriptor.checked,
                            disabled: !descriptor.enabled,
                        })
                }
                MenuEntry::Submenu { label, entries } => Some(gpui::MenuItem::submenu(
                    gpui::Menu::new(label.clone()).items(items(entries, descriptors)),
                )),
                MenuEntry::Separator => Some(gpui::MenuItem::separator()),
                MenuEntry::Heading(_) => None,
            })
            .collect()
    }
    models
        .iter()
        .map(|model| gpui::Menu::new(model.label.clone()).items(items(&model.entries, descriptors)))
        .collect()
}

/// Standard macOS Edit menu, dispatched through the focused TextField.
pub fn native_edit_menu() -> gpui::Menu {
    use crate::controls::text_field::{Copy, Cut, Paste, SelectAll};
    use gpui::{MenuItem, OsAction};

    gpui::Menu::new("Edit").items([
        MenuItem::os_action("Cut", Cut, OsAction::Cut),
        MenuItem::os_action("Copy", Copy, OsAction::Copy),
        MenuItem::os_action("Paste", Paste, OsAction::Paste),
        MenuItem::separator(),
        MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
    ])
}

pub struct PopupMenu {
    model: MenuModel,
    registry: Entity<CommandRegistry>,
    chain: Option<Entity<ResponderChain>>,
    focus: FocusHandle,
    position: Point<Pixels>,
    layer: Entity<TransientLayer>,
    surface: Option<SurfaceId>,
    // One highlighted row per open menu column.
    path: Vec<Option<usize>>,
    scroll: Vec<gpui::ScrollHandle>,
}

impl PopupMenu {
    pub fn new(
        model: MenuModel,
        registry: Entity<CommandRegistry>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&registry, |_, _, cx| cx.notify()).detach();
        let layer = cx.new(|cx| TransientLayer::new(window, cx));
        cx.observe(&layer, |menu, layer, cx| {
            if menu.surface.is_some_and(|id| !layer.read(cx).is_open(id)) {
                menu.surface = None;
            }
            cx.notify();
        })
        .detach();
        Self {
            model,
            registry,
            chain: None,
            focus: cx.focus_handle(),
            position: point(px(0.), px(0.)),
            layer,
            surface: None,
            path: vec![None],
            scroll: vec![gpui::ScrollHandle::new()],
        }
    }

    /// Put this menu on a window's shared transient layer. Without this it uses
    /// a private layer, so two menus in one window would not exclude each other.
    pub fn set_layer(&mut self, layer: Entity<TransientLayer>, cx: &mut Context<Self>) {
        cx.observe(&layer, |menu, layer, cx| {
            if menu.surface.is_some_and(|id| !layer.read(cx).is_open(id)) {
                menu.surface = None;
            }
            cx.notify();
        })
        .detach();
        if let Some(chain) = self.chain.clone() {
            layer.update(cx, |layer, _| layer.set_chain(chain));
        }
        self.layer = layer;
    }
    /// Resolve and validate through the window's responder chain (#57). Without
    /// one, the registry's own enabled state is used.
    pub fn set_chain(&mut self, chain: Entity<ResponderChain>, cx: &mut Context<Self>) {
        cx.observe(&chain, |_, _, cx| cx.notify()).detach();
        self.layer
            .update(cx, |layer, _| layer.set_chain(chain.clone()));
        self.chain = Some(chain);
    }
    fn descriptor(
        &self,
        id: &CommandId,
        window: &Window,
        cx: &App,
    ) -> Option<silvestris_core::command::CommandDescriptor> {
        match &self.chain {
            Some(chain) => chain.read(cx).present(&self.registry, id, window, cx),
            None => self.registry.read(cx).descriptors().get(id).cloned(),
        }
    }
    /// Whether the menu was opened and not dismissed. Kept for existing
    /// consumers; the authoritative check against a shared layer is
    /// [`is_open_in`](Self::is_open_in).
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.surface.is_some()
    }

    /// Whether the layer still has the surface open. Use this when a menu
    /// shares a window layer that can close it from the outside.
    #[must_use]
    pub fn is_open_in(&self, cx: &App) -> bool {
        self.surface
            .is_some_and(|id| self.layer.read(cx).is_open(id))
    }
    /// Replace a consumer's command tree. Dismiss a changed open menu rather
    /// than transferring its highlighted index to a different command.
    pub fn set_model(&mut self, model: MenuModel, window: &mut Window, cx: &mut Context<Self>) {
        if self.model != model {
            if self.is_open_in(cx) {
                self.dismiss(window, cx, true);
            }
            self.model = model;
            self.path = vec![None];
            self.scroll = vec![gpui::ScrollHandle::new()];
            cx.notify();
        }
    }
    pub fn open(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_open_in(cx) {
            // Reposition the existing surface: closing and reopening would
            // record the menu's own focus as the return target.
            self.position = position;
            self.path = vec![None];
            self.scroll = vec![gpui::ScrollHandle::new()];
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        self.position = position;
        self.path = vec![None];
        self.scroll = vec![gpui::ScrollHandle::new()];
        let anchor = Bounds::new(position, size(px(0.), px(0.)));
        let title = self.model.label.clone();
        let handle = self.layer.update(cx, |layer, cx| {
            layer.push(
                SurfaceSpec::new(TransientKind::Menu)
                    .anchor(anchor)
                    .owner(crate::responder::ResponderScope::Window)
                    .title(title),
                window,
                cx,
            )
        });
        self.surface = Some(handle.id());
        window.focus(&self.focus, cx);
        cx.notify();
    }
    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>, restore: bool) {
        let Some(id) = self.surface.take() else {
            return;
        };
        self.layer
            .update(cx, |layer, cx| layer.close(id, restore, window, cx));
        cx.notify();
    }

    /// Close the innermost surface. A submenu closes before its parent menu.
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.path.len() > 1 {
            self.path.pop();
            cx.notify();
            return true;
        }
        let Some(id) = self.surface else {
            return false;
        };
        self.layer.update(cx, |layer, cx| {
            layer.dismiss_surface(id, transient::DismissTrigger::Escape, window, cx, true)
        });
        if self
            .surface
            .is_some_and(|surface| !self.layer.read(cx).is_open(surface))
        {
            self.surface = None;
        }
        cx.notify();
        true
    }
    fn entries(&self, depth: usize) -> &[MenuEntry] {
        let mut entries = self.model.entries.as_slice();
        for level in 0..depth {
            let Some(Some(i)) = self.path.get(level) else {
                return &[];
            };
            let Some(MenuEntry::Submenu {
                entries: children, ..
            }) = entries.get(*i)
            else {
                return &[];
            };
            entries = children;
        }
        entries
    }
    fn enabled(&self, entry: &MenuEntry, window: &Window, cx: &App) -> bool {
        match entry {
            MenuEntry::Command(id) => self.descriptor(id, window, cx).is_some_and(|d| d.enabled),
            MenuEntry::Submenu { entries, .. } => !entries.is_empty(),
            _ => false,
        }
    }
    fn highlight(
        &mut self,
        depth: usize,
        index: usize,
        submenu: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(entry) = self.entries(depth).get(index) else {
            return;
        };
        let enabled = self.enabled(entry, window, cx);
        let child = enabled && submenu && matches!(entry, MenuEntry::Submenu { .. });
        let desired = if enabled { Some(index) } else { None };
        if self.path.get(depth) == Some(&desired) && (!child || self.path.len() > depth + 1) {
            return;
        }
        self.path.truncate(depth + 1);
        self.path[depth] = desired;
        if let Some(scroll) = self.scroll.get(depth) {
            scroll.scroll_to_item(index);
        }
        if child {
            self.path.push(None);
        }
        cx.notify();
    }
    fn choose(view: &Entity<Self>, depth: usize, index: usize, window: &mut Window, cx: &mut App) {
        let command = view.update(cx, |menu, cx| {
            let entry = menu.entries(depth).get(index)?.clone();
            if !menu.enabled(&entry, window, cx) {
                return None;
            }
            match entry {
                MenuEntry::Command(id) => {
                    menu.dismiss(window, cx, true);
                    Some(id)
                }
                MenuEntry::Submenu { .. } => {
                    menu.highlight(depth, index, true, window, cx);
                    None
                }
                _ => None,
            }
        });
        if let Some(id) = command {
            let (registry, chain) = {
                let menu = view.read(cx);
                (menu.registry.clone(), menu.chain.clone())
            };
            match chain {
                Some(chain) => {
                    let chain = chain.read(cx).clone();
                    chain.dispatch(&registry, &id, window, cx);
                }
                None => {
                    CommandRegistry::invoke(&registry, &id, window, cx);
                }
            }
        }
    }
    fn key(view: &Entity<Self>, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App) {
        let choice = view.update(cx, |menu, cx| {
            if !menu.is_open_in(cx) {
                return None;
            }
            let depth = menu.path.len() - 1;
            let key = event.keystroke.key.as_str();
            if key == "escape" {
                menu.escape(window, cx);
                return None;
            }
            if key == "tab" {
                menu.dismiss(window, cx, true);
                return None;
            }
            if event.keystroke.modifiers.modified() {
                return None;
            }
            if key == "left" {
                if depth > 0 {
                    menu.path.pop();
                    cx.notify();
                }
                return None;
            }
            if matches!(key, "enter" | "space" | "right") {
                if let Some(i) = menu.path[depth] {
                    if key != "right"
                        || matches!(menu.entries(depth).get(i), Some(MenuEntry::Submenu { .. }))
                    {
                        return Some((depth, i));
                    }
                }
                return None;
            }
            let entries = menu.entries(depth);
            let enabled: Vec<_> = entries
                .iter()
                .enumerate()
                .filter_map(|(i, e)| menu.enabled(e, window, cx).then_some(i))
                .collect();
            let current = menu.path[depth].and_then(|i| enabled.iter().position(|x| *x == i));
            let index = match key {
                "home" => enabled.first().copied(),
                "end" => enabled.last().copied(),
                "down" => enabled
                    .get(current.map(|i| (i + 1) % enabled.len().max(1)).unwrap_or(0))
                    .copied(),
                "up" => enabled
                    .get(
                        current
                            .map(|i| (i + enabled.len() - 1) % enabled.len())
                            .unwrap_or(enabled.len().saturating_sub(1)),
                    )
                    .copied(),
                _ if key.chars().count() == 1 => {
                    let start = current.map(|i| i + 1).unwrap_or(0);
                    (0..enabled.len())
                        .map(|offset| enabled[(start + offset) % enabled.len()])
                        .find(|i| {
                            let label = match &entries[*i] {
                                MenuEntry::Command(id) => menu
                                    .registry
                                    .read(cx)
                                    .descriptors()
                                    .get(id)
                                    .map(|d| d.label.clone())
                                    .unwrap_or_default(),
                                MenuEntry::Submenu { label, .. } => label.clone(),
                                _ => String::new(),
                            };
                            label.to_lowercase().starts_with(&key.to_lowercase())
                        })
                }
                _ => None,
            };
            if let Some(i) = index {
                menu.highlight(depth, i, false, window, cx);
            }
            None
        });
        cx.stop_propagation();
        if let Some((depth, i)) = choice {
            Self::choose(view, depth, i, window, cx);
        }
    }

    fn panel(
        &self,
        entries: &[MenuEntry],
        depth: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let measure = |text: String| {
            let run = gpui::TextRun {
                len: text.len(),
                font: gpui::font(theme.type_.ui),
                color: hsla(theme.colors.menu_text),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_line(text.into(), px(theme.type_.size_menu), &[run], None)
                .width
                .as_f32()
        };
        let width = entries
            .iter()
            .map(|entry| {
                let (label, shortcut) = match entry {
                    MenuEntry::Command(id) => self
                        .registry
                        .read(cx)
                        .descriptors()
                        .get(id)
                        .map(|d| {
                            (
                                d.menu_label(),
                                d.shortcut
                                    .as_deref()
                                    .map(|k| display_shortcut(k, Platform::current())),
                            )
                        })
                        .unwrap_or_else(|| (id.to_string(), None)),
                    MenuEntry::Heading(label) | MenuEntry::Submenu { label, .. } => {
                        (label.clone(), None)
                    }
                    MenuEntry::Separator => return 0.,
                };
                measure(label) + shortcut.map(|s| measure(s) + 20.).unwrap_or(0.) + 44.
            })
            .fold(160., f32::max)
            .min((window.viewport_size().width.as_f32() - 8.).max(0.));
        let mut panel = div()
            .id(("menu-panel", depth))
            .debug_selector(|| format!("menu-panel-{depth}"))
            .role(gpui::Role::Menu)
            .aria_label(self.model.label.clone())
            .relative()
            .flex()
            .flex_col()
            .w(px(width))
            .py(px(m.menu_padding))
            .max_h((window.viewport_size().height - px(8.)).max(px(0.)))
            .overflow_y_scroll()
            .track_scroll(&self.scroll[depth])
            .bg(hsla(theme.colors.menu))
            .rounded(px(m.menu_radius))
            .shadow_lg()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());
        for (i, entry) in entries.iter().enumerate() {
            if matches!(entry, MenuEntry::Separator) {
                let separator = theme.colors.menu_separator;
                panel = panel.child(
                    div()
                        .flex_none()
                        .h(px(m.menu_separator_height))
                        .relative()
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let h = crate::convert::hairline(window);
                                    let y = bounds.origin.y + (bounds.size.height - h) / 2.0;
                                    paint::hline(
                                        window,
                                        bounds.origin.x + px(1.),
                                        y,
                                        bounds.size.width - px(2.),
                                        separator,
                                    );
                                },
                            )
                            .absolute()
                            .inset_0(),
                        ),
                );
                continue;
            }
            let descriptor = match entry {
                MenuEntry::Command(id) => self.descriptor(id, window, cx),
                _ => None,
            };
            let label = match entry {
                MenuEntry::Command(id) => descriptor
                    .as_ref()
                    .map(|d| d.menu_label())
                    .unwrap_or_else(|| id.to_string()),
                MenuEntry::Submenu { label, .. } | MenuEntry::Heading(label) => label.clone(),
                _ => unreachable!(),
            };
            let enabled = self.enabled(entry, window, cx);
            let highlighted = enabled && self.path.get(depth) == Some(&Some(i));
            let selection = theme.lists.menu;
            let foreground = if !enabled {
                theme.colors.menu_text_disabled
            } else if highlighted {
                selection.text
            } else {
                theme.colors.menu_text
            };
            let check = descriptor.as_ref().is_some_and(|d| d.checked);
            let shortcut = descriptor
                .as_ref()
                .and_then(|d| d.shortcut.as_deref())
                .map(|k| display_shortcut(k, Platform::current()));
            let hover = cx.entity();
            let choose = cx.entity();
            let mut row = div()
                .id(("item", i))
                .debug_selector(|| format!("menu-item-{depth}-{i}"))
                .role(if matches!(entry, MenuEntry::Heading(_)) {
                    gpui::Role::Heading
                } else {
                    gpui::Role::MenuItem
                })
                .aria_selected(highlighted)
                .aria_label(if check {
                    format!("{label}, checked")
                } else {
                    label.clone()
                })
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .h(px(m.menu_item_height))
                .font_family(theme.type_.ui)
                .text_size(px(theme.type_.size_menu))
                .text_color(hsla(foreground))
                .when(highlighted, |el| {
                    el.child(
                        canvas(
                            |_, _, _| (),
                            move |b, _, w, _| paint::selection(w, b, &selection),
                        )
                        .absolute()
                        .inset_0(),
                    )
                })
                .child(div().relative().w(px(20.)).flex_none().child(if check {
                    "✓"
                } else {
                    ""
                }))
                .child(div().relative().flex_1().min_w_0().truncate().child(label))
                .when_some(shortcut, |el, text| {
                    el.child(div().relative().ml(px(20.)).child(text))
                })
                .child(div().relative().w(px(16.)).ml(px(8.)).child(
                    if matches!(entry, MenuEntry::Submenu { .. }) {
                        "▸"
                    } else {
                        ""
                    },
                ))
                .on_hover(move |over, window, cx| {
                    if *over {
                        hover.update(cx, |menu, cx| menu.highlight(depth, i, true, window, cx));
                    }
                })
                .when(enabled, |el| {
                    el.on_mouse_up(MouseButton::Left, move |_, window, cx| {
                        cx.stop_propagation();
                        Self::choose(&choose, depth, i, window, cx);
                    })
                });
            if let MenuEntry::Submenu { entries, .. } = entry {
                if highlighted && self.path.len() > depth + 1 {
                    let child = self.panel(entries, depth + 1, window, cx);
                    row = row.child(
                        deferred(
                            anchored()
                                .position_mode(AnchoredPositionMode::Local)
                                .attach(
                                    Bounds::new(
                                        point(px(0.), px(0.)),
                                        size(px(width), px(m.menu_item_height)),
                                    ),
                                    Anchor::TopRight,
                                )
                                .child(child),
                        )
                        .priority(depth + 2),
                    );
                }
            }
            panel = panel.child(row);
        }
        panel.into_any_element()
    }
}
impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(surface) = self.surface.filter(|id| self.layer.read(cx).is_open(*id)) else {
            return div().into_any_element();
        };
        self.scroll
            .resize_with(self.path.len(), gpui::ScrollHandle::new);
        let panel = self.panel(&self.model.entries, 0, window, cx);
        let key = cx.entity();
        let action_view = cx.entity();
        let registry = self.registry.clone();
        let chain = self.chain.clone();
        let layer = self.layer.clone();
        transient::shield(layer, surface, window, point(px(0.), px(0.)))
            .track_focus(&self.focus)
            .key_context("SilvestrisMenu")
            .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_mouse_move(|_, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| Self::key(&key, event, window, cx))
            .on_action(move |action: &InvokeCommand, window, cx| {
                cx.stop_propagation();
                action_view.update(cx, |menu, cx| menu.dismiss(window, cx, true));
                let id = CommandId::new(&action.id);
                match chain.clone() {
                    Some(chain) => {
                        let chain = chain.read(cx).clone();
                        chain.dispatch(&registry, &id, window, cx);
                    }
                    None => {
                        CommandRegistry::invoke(&registry, &id, window, cx);
                    }
                }
            })
            .child(
                anchored()
                    .position(self.position)
                    .snap_to_window_with_margin(px(4.))
                    .child(panel),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{FocusHandle, Modifiers, TestAppContext, VisualTestContext};
    use silvestris_core::command::CommandDescriptor;
    use std::{cell::Cell, rc::Rc};
    struct Harness {
        menu: Entity<PopupMenu>,
        focus: FocusHandle,
        background_hits: Rc<Cell<usize>>,
    }
    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let hits = self.background_hits.clone();
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_mouse_down(MouseButton::Left, move |_, _, _| hits.set(hits.get() + 1))
                .child(deferred(self.menu.clone()).priority(1))
        }
    }
    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|w, cx| {
            w.draw(cx).clear(cx);
        });
    }
    fn model() -> MenuModel {
        MenuModel {
            label: "View".into(),
            entries: vec![
                MenuEntry::Heading("Views".into()),
                MenuEntry::Separator,
                MenuEntry::Command("view.one".into()),
                MenuEntry::Command("view.disabled".into()),
                MenuEntry::Submenu {
                    label: "More".into(),
                    entries: vec![MenuEntry::Command("view.two".into())],
                },
            ],
        }
    }
    #[test]
    fn mouse_activation_live_disabled_state_and_outside_click_shield_in_all_themes() {
        for theme in crate::ThemeId::ALL {
            let mut app = TestAppContext::single();
            app.update(|cx| crate::theme::install(crate::ThemePreference::explicit(theme), cx));
            let hits = Rc::new(Cell::new(0));
            let count = hits.clone();
            let (h, cx) = app.add_window_view(|window, cx| {
                let registry = cx.new(|_| CommandRegistry::default());
                registry.update(cx, |r, cx| {
                    r.register(
                        CommandDescriptor::toggle("view.one", "First View", true),
                        move |_, _, _| count.set(count.get() + 1),
                        cx,
                    )
                    .unwrap();
                    r.register(
                        CommandDescriptor::action("view.disabled", "Disabled").enabled(false),
                        |_, _, _| panic!("disabled command"),
                        cx,
                    )
                    .unwrap();
                });
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                let menu = cx.new(|cx| PopupMenu::new(model(), registry, window, cx));
                menu.update(cx, |menu, cx| {
                    menu.open(point(px(30.), px(30.)), window, cx)
                });
                Harness {
                    menu,
                    focus,
                    background_hits: Rc::new(Cell::new(0)),
                }
            });
            draw(cx);
            let row = cx.debug_bounds("menu-item-0-2").unwrap();
            cx.simulate_click(row.center(), Modifiers::default());
            draw(cx);
            assert_eq!(hits.get(), 1);
            cx.update(|window, cx| {
                assert!(!h.read(cx).menu.read(cx).is_open_in(cx));
                assert!(h.read(cx).focus.is_focused(window));
                let menu = h.read(cx).menu.clone();
                menu.update(cx, |m, cx| {
                    m.registry.update(cx, |r, cx| {
                        r.publish(
                            CommandDescriptor::action("view.one", "First View").enabled(false),
                            cx,
                        )
                        .unwrap()
                    });
                    m.open(point(px(30.), px(30.)), window, cx);
                });
            });
            draw(cx);
            let row = cx.debug_bounds("menu-item-0-2").unwrap();
            cx.simulate_click(row.center(), Modifiers::default());
            draw(cx);
            assert_eq!(hits.get(), 1);
            cx.update(|_, cx| assert!(h.read(cx).menu.read(cx).is_open_in(cx)));
            cx.simulate_click(point(px(500.), px(300.)), Modifiers::default());
            draw(cx);
            cx.update(|_, cx| {
                assert!(!h.read(cx).menu.read(cx).is_open_in(cx));
                assert_eq!(h.read(cx).background_hits.get(), 0);
            });
        }
    }
    #[test]
    fn submenu_keyboard_navigation_dispatches_and_restores_focus() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let hits = Rc::new(Cell::new(0));
        let count = hits.clone();
        let (h, cx) = app.add_window_view(|window, cx| {
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |r, cx| {
                r.register(
                    CommandDescriptor::action("view.one", "First"),
                    |_, _, _| {},
                    cx,
                )
                .unwrap();
                r.register(
                    CommandDescriptor::action("view.two", "Second"),
                    move |_, _, _| count.set(count.get() + 1),
                    cx,
                )
                .unwrap();
            });
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let menu = cx.new(|cx| PopupMenu::new(model(), registry, window, cx));
            menu.update(cx, |m, cx| m.open(point(px(10.), px(10.)), window, cx));
            Harness {
                menu,
                focus,
                background_hits: Rc::new(Cell::new(0)),
            }
        });
        draw(cx);
        cx.simulate_keystrokes("home down right");
        draw(cx);
        assert!(cx.debug_bounds("menu-panel-1").is_some());
        cx.simulate_keystrokes("home enter");
        draw(cx);
        assert_eq!(hits.get(), 1);
        cx.update(|window, cx| {
            assert!(!h.read(cx).menu.read(cx).is_open_in(cx));
            assert!(h.read(cx).focus.is_focused(window));
            let menu = h.read(cx).menu.clone();
            menu.update(cx, |m, cx| m.open(point(px(10.), px(10.)), window, cx));
        });
        draw(cx);
        cx.simulate_keystrokes("home down right");
        draw(cx);
        assert!(cx.debug_bounds("menu-panel-1").is_some());
        cx.simulate_keystrokes("escape");
        draw(cx);
        cx.update(|_, cx| {
            assert!(
                h.read(cx).menu.read(cx).is_open_in(cx),
                "the first Escape closes only the submenu"
            );
        });
        assert!(cx.debug_bounds("menu-panel-1").is_none());
        cx.simulate_keystrokes("escape");
        draw(cx);
        cx.update(|window, cx| {
            assert!(!h.read(cx).menu.read(cx).is_open_in(cx));
            assert!(h.read(cx).focus.is_focused(window));
        });
    }
    #[test]
    fn long_menu_fits_viewport_scrolls_to_end_and_closes_on_deactivation() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let (h, cx) = app.add_window_view(|window, cx| {
            let registry = cx.new(|_| CommandRegistry::default());
            registry.update(cx, |r, cx| {
                for i in 0..100 {
                    r.register(
                        CommandDescriptor::action(&format!("cmd-{i}"), &format!("Item {i}")),
                        |_, _, _| {},
                        cx,
                    )
                    .unwrap();
                }
            });
            let focus = cx.focus_handle();
            window.focus(&focus, cx);
            let model = MenuModel {
                label: "Long".into(),
                entries: (0..100)
                    .map(|i| MenuEntry::Command(CommandId::new(format!("cmd-{i}"))))
                    .collect(),
            };
            let menu = cx.new(|cx| PopupMenu::new(model, registry, window, cx));
            menu.update(cx, |m, cx| m.open(point(px(390.), px(190.)), window, cx));
            Harness {
                menu,
                focus,
                background_hits: Rc::new(Cell::new(0)),
            }
        });
        cx.simulate_resize(size(px(400.), px(200.)));
        draw(cx);
        let panel = cx.debug_bounds("menu-panel-0").unwrap();
        assert!(panel.left() >= px(0.) && panel.right() <= px(400.));
        assert!(panel.top() >= px(0.) && panel.bottom() <= px(200.));
        cx.simulate_keystrokes("end");
        draw(cx);
        let last = cx.debug_bounds("menu-item-0-99").unwrap();
        assert!(
            last.top() >= panel.top() && last.bottom() <= panel.bottom(),
            "{last:?} outside {panel:?}"
        );
        cx.update(|window, _| window.activate_window());
        draw(cx);
        cx.deactivate_window();
        draw(cx);
        cx.update(|_, cx| assert!(!h.read(cx).menu.read(cx).is_open_in(cx)));
    }
}
