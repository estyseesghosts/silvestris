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
    AnyElement, App, Bounds, BoxShadow, Context, Entity, FocusHandle, MouseButton, Pixels, Point,
    Render, Window,
};
use silvestris_core::command::{display_shortcut, CommandId, MenuEntry, MenuModel, Platform};
use std::collections::BTreeMap;
use std::rc::Rc;

type MenuAction = Rc<dyn Fn(&mut Window, &mut App)>;

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
                MenuEntry::Heading(_) | MenuEntry::Action { .. } => None,
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
    // Opened from a control: the top edge hangs square from it.
    pop_down: bool,
    /// Whether command rows print their keyboard shortcut. Action and
    /// command pop-down menus suppress them (HIG pp. 288-375).
    show_shortcuts: bool,
    layer: Entity<TransientLayer>,
    surface: Option<SurfaceId>,
    actions: BTreeMap<String, MenuAction>,
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
            pop_down: false,
            show_shortcuts: true,
            layer,
            surface: None,
            actions: BTreeMap::new(),
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

    /// Register host behavior for a non-command row such as an overflow item
    /// that returns focus to an existing control.
    pub fn on_action(
        &mut self,
        id: impl Into<String>,
        action: impl Fn(&mut Window, &mut App) + 'static,
    ) {
        self.actions.insert(id.into(), Rc::new(action));
    }

    /// Remove host actions before installing the current overflow rows.
    pub fn clear_actions(&mut self) {
        self.actions.clear();
    }

    /// Show or hide each command row's keyboard shortcut. Action menus and
    /// command pop-downs hide them; the main menu bar keeps them.
    pub fn set_show_shortcuts(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_shortcuts != show {
            self.show_shortcuts = show;
            cx.notify();
        }
    }

    #[must_use]
    pub fn shows_shortcuts(&self) -> bool {
        self.show_shortcuts
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
    /// Open as a contextual menu with its top-left corner at `position`. All
    /// four corners are rounded (M: menu.context.body).
    pub fn open(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.pop_down = false;
        self.open_at(position, window, cx);
    }

    /// Open hanging from a control, with the frame's top-left corner at
    /// `position` (usually the control's bottom-left). The top corners are
    /// square and the bottom ones rounded, as under 10.6's toolbar Action
    /// button (M: menu.context.rows).
    pub fn open_pop_down(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pop_down = true;
        self.open_at(position, window, cx);
    }

    fn open_at(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
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
            MenuEntry::Action { id, .. } => self.actions.contains_key(id),
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
    /// Highlight one row per column along `path`, opening each submenu row
    /// it reaches, as pointer scanning would. Nothing is invoked. Disabled rows
    /// stop the walk (HIG p. 168: disabled items are not highlighted).
    pub fn highlight_path(&mut self, path: &[usize], window: &Window, cx: &mut Context<Self>) {
        for (depth, &index) in path.iter().enumerate() {
            if self.path.len() <= depth {
                return;
            }
            self.highlight(depth, index, true, window, cx);
            if self.path.get(depth) != Some(&Some(index)) {
                return;
            }
        }
    }
    fn choose(view: &Entity<Self>, depth: usize, index: usize, window: &mut Window, cx: &mut App) {
        enum Choice {
            Command(CommandId),
            Action(MenuAction),
        }
        let choice = view.update(cx, |menu, cx| {
            let entry = menu.entries(depth).get(index)?.clone();
            if !menu.enabled(&entry, window, cx) {
                return None;
            }
            match entry {
                MenuEntry::Command(id) => {
                    menu.dismiss(window, cx, true);
                    Some(Choice::Command(id))
                }
                MenuEntry::Action { id, .. } => {
                    let action = menu.actions.get(&id)?.clone();
                    menu.dismiss(window, cx, true);
                    Some(Choice::Action(action))
                }
                MenuEntry::Submenu { .. } => {
                    menu.highlight(depth, index, true, window, cx);
                    None
                }
                _ => None,
            }
        });
        match choice {
            Some(Choice::Command(id)) => {
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
            Some(Choice::Action(action)) => action(window, cx),
            None => {}
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
                                MenuEntry::Action { label, .. } => label.clone(),
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

    /// Body width: 21 px from the body edge to the label and 21 px after it
    /// (M: menu.context.rows; the ink sits 22 px in from each body edge and
    /// Lato has a one-pixel side bearing), widened for key equivalents and
    /// submenu arrows. No minimum: the measured Arrange By submenu body is
    /// 129 px wide.
    fn panel_width(&self, entries: &[MenuEntry], window: &mut Window, cx: &App) -> f32 {
        let theme = cx.theme().clone();
        let rows: Vec<(String, Option<String>, bool)> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Command(id) => Some(
                    self.registry
                        .read(cx)
                        .descriptors()
                        .get(id)
                        .map(|d| {
                            (
                                d.menu_label(),
                                d.shortcut
                                    .as_deref()
                                    .map(|k| display_shortcut(k, Platform::current())),
                                false,
                            )
                        })
                        .unwrap_or_else(|| (id.to_string(), None, false)),
                ),
                MenuEntry::Heading(label) => Some((label.clone(), None, false)),
                MenuEntry::Submenu { label, .. } => Some((label.clone(), None, true)),
                MenuEntry::Action { label, .. } => Some((label.clone(), None, false)),
                MenuEntry::Separator => None,
            })
            .collect();
        let key_column = key_column_width(&rows, &theme, window);
        rows.iter()
            .map(|(label, shortcut, submenu)| {
                let label = measure_menu_text(label, &theme, window);
                let tail = match shortcut {
                    Some(text) => {
                        let (mods, _) = split_shortcut(text);
                        KEY_GAP
                            + measure_menu_text(mods, &theme, window)
                            + modifier_gap(mods)
                            + key_column
                            + KEY_RIGHT
                    }
                    None if *submenu => ARROW_ZONE + ARROW_GAP,
                    None => TEXT_INSET,
                };
                TEXT_INSET + label + tail
            })
            .fold(0., f32::max)
            // Whole pixels, so the edge hairlines stay on device rows.
            .ceil()
    }

    #[allow(clippy::too_many_arguments)]
    fn panel(
        &self,
        entries: &[MenuEntry],
        depth: usize,
        frame_x: f32,
        square: SquareCorners,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = cx.theme().clone();
        let m = &theme.metrics;
        let viewport = window.viewport_size().width.as_f32();
        let body_width = self
            .panel_width(entries, window, cx)
            .min((viewport - 2. * VIEWPORT_MARGIN - 2.).max(0.))
            .floor();
        let frame_width = body_width + 2.;
        let show_shortcuts = self.show_shortcuts;
        let shortcuts: Vec<(String, Option<String>, bool)> = entries
            .iter()
            .filter_map(|entry| match entry {
                MenuEntry::Command(id) => self.descriptor(id, window, cx).map(|d| {
                    (
                        String::new(),
                        if show_shortcuts {
                            d.shortcut
                                .as_deref()
                                .map(|k| display_shortcut(k, Platform::current()))
                        } else {
                            None
                        },
                        false,
                    )
                }),
                _ => None,
            })
            .collect();
        let key_column = key_column_width(&shortcuts, &theme, window);
        let mut body = menu_body(&theme, square)
            .id(("menu-body", depth))
            .w(px(body_width))
            .max_h((window.viewport_size().height - px(2. * VIEWPORT_MARGIN + 2.)).max(px(0.)))
            .overflow_y_scroll()
            .track_scroll(&self.scroll[depth]);
        for (i, entry) in entries.iter().enumerate() {
            if matches!(entry, MenuEntry::Separator) {
                body = body.child(menu_separator(&theme));
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
                MenuEntry::Submenu { label, .. }
                | MenuEntry::Heading(label)
                | MenuEntry::Action { label, .. } => label.clone(),
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
            let mixed = descriptor.as_ref().is_some_and(|d| d.mixed);
            let shortcut = if self.show_shortcuts {
                descriptor
                    .as_ref()
                    .and_then(|d| d.shortcut.as_deref())
                    .map(|k| display_shortcut(k, Platform::current()))
            } else {
                None
            };
            let submenu = matches!(entry, MenuEntry::Submenu { .. });
            let dy = m.menu_label_dy;
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
                } else if mixed {
                    format!("{label}, mixed")
                } else {
                    label.clone()
                })
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .h(px(m.menu_item_height))
                .pl(px(TEXT_INSET))
                .pr(px(if shortcut.is_some() {
                    KEY_RIGHT
                } else if submenu {
                    ARROW_ZONE + ARROW_GAP
                } else {
                    TEXT_INSET
                }))
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
                .when(check || mixed, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left(px(MARK_X))
                            .top(px(dy))
                            .bottom(px(-dy))
                            .flex()
                            .items_center()
                            .debug_selector(move || format!("menu-mark-{depth}-{i}"))
                            .child(if check { "\u{2713}" } else { "\u{2013}" }),
                    )
                })
                .child(
                    div()
                        .relative()
                        .top(px(dy))
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(label),
                )
                .when_some(shortcut, |el, text| {
                    let (mods, key) = split_shortcut(&text);
                    let (mods, key) = (mods.to_string(), key.to_string());
                    el.child(
                        div()
                            .relative()
                            .flex_none()
                            .top(px(dy))
                            .ml(px(KEY_GAP))
                            .mr(px(modifier_gap(&mods)))
                            .child(mods),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_none()
                            .top(px(dy))
                            .w(px(key_column))
                            .debug_selector(move || format!("menu-key-{depth}-{i}"))
                            .child(key),
                    )
                })
                .when(submenu, |el| el.child(submenu_arrow(foreground)))
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
                    // M: menu.context.edge / menu.context.corners: the
                    // submenu's edge column sits one column inside the
                    // parent's, and its first item lines up with this row.
                    let child_frame = self.panel_width(entries, window, cx) + 2.;
                    let opens_left =
                        submenu_opens_left(frame_x, frame_width, child_frame, viewport);
                    let child_x = if opens_left {
                        frame_x + SUBMENU_OVERLAP - child_frame
                    } else {
                        frame_x + frame_width - SUBMENU_OVERLAP
                    };
                    let child = self.panel(
                        entries,
                        depth + 1,
                        child_x,
                        SquareCorners {
                            top_left: !opens_left,
                            top_right: opens_left,
                        },
                        window,
                        cx,
                    );
                    // The row spans the body; the frame edge is one hairline
                    // outside it on each side.
                    // Anchor from a zero-size box at the row's top-left, so the
                    // target is in row space regardless of the flex flow.
                    row = row.child(
                        div().absolute().top_0().left_0().child(
                            deferred(
                                anchored()
                                    .position_mode(AnchoredPositionMode::Local)
                                    .attach(
                                        Bounds::new(
                                            point(px(0.), px(0.)),
                                            size(px(body_width), px(m.menu_item_height)),
                                        ),
                                        Anchor::TopRight,
                                    )
                                    .offset(point(
                                        px(1. - SUBMENU_OVERLAP),
                                        px(-(m.menu_padding + 1.)),
                                    ))
                                    .child(child),
                            )
                            .priority(depth + 2),
                        ),
                    );
                }
            }
            body = body.child(row);
        }
        menu_frame(&theme, square)
            .id(("menu-panel", depth))
            .debug_selector(move || format!("menu-panel-{depth}"))
            .role(gpui::Role::Menu)
            .aria_label(self.model.label.clone())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(body)
            .into_any_element()
    }
}

/// Corners of a menu frame that stay square: the top edge of a pop-down menu
/// hanging from its control, and the top corner of a submenu on the parent's
/// side (M: menu.context.corners, menu.context.rows).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct SquareCorners {
    pub top_left: bool,
    pub top_right: bool,
}

// M: menu.context.rows: label ink 22 px inside each body edge.
const TEXT_INSET: f32 = 21.;
// M: menu.context.arrow / menu.bar.arrow_and_keys: arrow 19 px in from the
// body edge, 10 rows tall, 4 rows below the item top, tip 7.5 px right.
const ARROW_ZONE: f32 = 19.;
const ARROW_WIDTH: f32 = 7.5;
const ARROW_HEIGHT: f32 = 10.;
// estimate: label-to-arrow and label-to-key gaps (no capture has a label
// long enough to meet either).
const ARROW_GAP: f32 = 8.;
const KEY_GAP: f32 = 20.;
// M: menu.bar.key_column: modifiers end 4 px before the key column, which
// starts 22 px from the body edge; the widest key (delete) ends 7 px in.
const MODIFIER_GAP: f32 = 4.;
const KEY_RIGHT: f32 = 7.;
const KEY_MIN: f32 = 15.;
// estimate: no 10.6 capture shows a checked menu item.
const MARK_X: f32 = 6.;
// M: menu.context.edge: the submenu edge column overlaps the parent's frame
// by two columns.
const SUBMENU_OVERLAP: f32 = 2.;
// designed: keep menus 4 px inside the window.
const VIEWPORT_MARGIN: f32 = 4.;

fn measure_menu_text(text: &str, theme: &silvestris_core::Theme, window: &mut Window) -> f32 {
    if text.is_empty() {
        return 0.;
    }
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
        .shape_line(
            text.to_string().into(),
            px(theme.type_.size_menu),
            &[run],
            None,
        )
        .width
        .as_f32()
}

fn key_column_width(
    rows: &[(String, Option<String>, bool)],
    theme: &silvestris_core::Theme,
    window: &mut Window,
) -> f32 {
    rows.iter()
        .filter_map(|(_, shortcut, _)| shortcut.as_deref())
        .map(|text| measure_menu_text(split_shortcut(text).1, theme, window))
        .fold(KEY_MIN, f32::max)
}

/// Symbol modifiers keep the measured gap before the key; spelled-out
/// modifiers ("Ctrl+") already end in a separator.
fn modifier_gap(mods: &str) -> f32 {
    if mods.ends_with(['⌃', '⌥', '⇧', '⌘']) {
        MODIFIER_GAP
    } else {
        0.
    }
}

/// Split a displayed shortcut into its modifiers and the key of its last
/// stroke. The key is left-aligned in its own column and the modifiers are
/// right-aligned against it (M: menu.bar.key_column; HIG p. 167).
pub fn split_shortcut(text: &str) -> (&str, &str) {
    let modifier = |c: char| matches!(c, '⌃' | '⌥' | '⇧' | '⌘');
    let mut split = 0;
    for (i, c) in text.char_indices() {
        let end = i + c.len_utf8();
        if modifier(c) || c == ' ' || (c == '+' && end < text.len()) {
            split = end;
        }
    }
    text.split_at(split)
}

/// Whether a submenu flips to the parent's left. Mirrors GPUI's attached
/// placement: flip only when the right side overflows and the left side
/// overflows less.
fn submenu_opens_left(frame_x: f32, frame_width: f32, child: f32, viewport: f32) -> bool {
    let right = frame_x + frame_width - SUBMENU_OVERLAP;
    let over_right = (right + child - viewport).max(0.);
    if over_right == 0. {
        return false;
    }
    let left = frame_x + SUBMENU_OVERLAP - child;
    (-left).max(0.) < over_right
}

/// The outer frame: drop shadow and the edge hairline, painted outside the
/// body so the edge darkens the shadow, not the body (M: menu.context.edge).
pub(crate) fn menu_frame(theme: &silvestris_core::Theme, square: SquareCorners) -> gpui::Div {
    let m = &theme.metrics;
    let r = px(m.menu_radius);
    let radii = gpui::Corners {
        top_left: if square.top_left { px(0.) } else { r },
        top_right: if square.top_right { px(0.) } else { r },
        bottom_right: r,
        bottom_left: r,
    };
    let shadow = BoxShadow::new(
        px(m.menu_shadow_offset[0]),
        px(m.menu_shadow_offset[1]),
        hsla(theme.colors.menu_shadow),
    )
    .blur_radius(px(m.menu_shadow_blur))
    .spread_radius(px(m.menu_shadow_spread));
    let reach = px(m.menu_shadow_blur * 3.
        + m.menu_shadow_offset[0]
            .abs()
            .max(m.menu_shadow_offset[1].abs())
        + m.menu_shadow_spread.max(0.)
        + 1.);
    let border = theme.colors.menu_border;
    div()
        .relative()
        .flex()
        .flex_col()
        // Room for the edge hairline, painted by the canvas below.
        .p(px(1.))
        .child(
            canvas(
                |_, _, _| (),
                move |b, _, window, _| {
                    // 10.6 casts no shadow under the translucent body: it
                    // reads ffffff over white (M: menu.context.rows). Paint
                    // the shadow only outside the frame and in the corner
                    // squares a rounded corner leaves open. The edge ring
                    // keeps the shadow under it (M: menu.context.edge).
                    let body = paint::inset(b, px(1.));
                    let mut masks = vec![
                        Bounds::from_corners(
                            point(body.left() - reach, body.top() - reach),
                            point(body.right() + reach, body.top()),
                        ),
                        Bounds::from_corners(
                            point(body.left() - reach, body.bottom()),
                            point(body.right() + reach, body.bottom() + reach),
                        ),
                        Bounds::from_corners(
                            point(body.left() - reach, body.top()),
                            point(body.left(), body.bottom()),
                        ),
                        Bounds::from_corners(
                            point(body.right(), body.top()),
                            point(body.right() + reach, body.bottom()),
                        ),
                    ];
                    for (radius, x, y) in [
                        (radii.top_left, body.left(), body.top()),
                        (radii.top_right, body.right() - r, body.top()),
                        (radii.bottom_right, body.right() - r, body.bottom() - r),
                        (radii.bottom_left, body.left(), body.bottom() - r),
                    ] {
                        if radius > px(0.) {
                            masks.push(Bounds::new(point(x, y), size(r, r)));
                        }
                    }
                    for mask in masks {
                        window.with_content_mask(Some(gpui::ContentMask { bounds: mask }), |w| {
                            w.paint_drop_shadows(b, radii, std::slice::from_ref(&shadow));
                        });
                    }
                    window.paint_quad(gpui::quad(
                        b,
                        radii,
                        gpui::transparent_black(),
                        px(1.),
                        hsla(border),
                        gpui::BorderStyle::Solid,
                    ));
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// The translucent body inside [`menu_frame`], with the vertical padding.
pub(crate) fn menu_body(theme: &silvestris_core::Theme, square: SquareCorners) -> gpui::Div {
    let m = &theme.metrics;
    let r = px((m.menu_radius - 1.).max(0.));
    div()
        .relative()
        .flex()
        .flex_col()
        .py(px(m.menu_padding))
        .bg(hsla(theme.colors.menu))
        .rounded(r)
        .when(square.top_left, |el| el.rounded_tl(px(0.)))
        .when(square.top_right, |el| el.rounded_tr(px(0.)))
}

/// A separator block: one hairline on the 6th row, inset 1 px at each side
/// (M: menu.context.rows, menu.context.separator).
pub(crate) fn menu_separator(theme: &silvestris_core::Theme) -> impl IntoElement {
    let separator = theme.colors.menu_separator;
    let highlight = theme.colors.menu_separator_highlight;
    div()
        .flex_none()
        .h(px(theme.metrics.menu_separator_height))
        .relative()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let h = crate::convert::hairline(window);
                    let y = bounds.origin.y + px(5.);
                    let x = bounds.origin.x + px(1.);
                    let w = bounds.size.width - px(2.);
                    paint::hline(window, x, y, w, separator);
                    paint::hline(window, x, y + h, w, highlight);
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// The right-pointing submenu indicator, drawn as a triangle rather than a
/// font glyph (M: menu.context.arrow).
fn submenu_arrow(color: silvestris_core::color::Rgba8) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let x = bounds.right() - px(ARROW_ZONE);
            // M: 4 rows down in both 18 and 19 px rows; centred otherwise.
            let y = bounds.origin.y + ((bounds.size.height - px(ARROW_HEIGHT)) / 2.).floor();
            paint::polygon(
                window,
                &[
                    point(x, y),
                    point(x + px(ARROW_WIDTH), y + px(ARROW_HEIGHT / 2.)),
                    point(x, y + px(ARROW_HEIGHT)),
                ],
                color,
            );
        },
    )
    .absolute()
    .inset_0()
}
impl Render for PopupMenu {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(surface) = self.surface.filter(|id| self.layer.read(cx).is_open(*id)) else {
            return div().into_any_element();
        };
        self.scroll
            .resize_with(self.path.len(), gpui::ScrollHandle::new);
        let viewport = window.viewport_size().width.as_f32();
        let frame = self.panel_width(&self.model.entries, window, cx) + 2.;
        let root_x = self
            .position
            .x
            .as_f32()
            .min(viewport - VIEWPORT_MARGIN - frame)
            .max(VIEWPORT_MARGIN);
        let square = SquareCorners {
            top_left: self.pop_down,
            top_right: self.pop_down,
        };
        let entries = self.model.entries.clone();
        let panel = self.panel(&entries, 0, root_x, square, window, cx);
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
