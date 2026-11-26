//! Standard toolbar composition and responsive overflow (#26).
//!
//! Toolbar controls keep their measured natural widths. The pure planner in
//! `silvestris_core::layout` uses those measurements and item priorities to
//! preserve order while moving lower-priority items into the shared menu host.

use crate::{
    command::{CommandButton, CommandRegistry},
    controls::{
        button::{Button, ButtonStyle, ControlSize},
        segmented::{Segment, SegmentMode, SegmentStyle, SegmentedControl},
        text_field::TextField,
    },
    icons::{TemplateImage, TemplateImageSlot},
    menu::PopupMenu,
    responder::ResponderChain,
    theme::ActiveTheme,
};
use gpui::{
    canvas, container_query, deferred, div, prelude::*, px, AnyElement, App, Bounds, ElementId,
    Entity, Pixels, RenderOnce, SharedString, Window,
};
use silvestris_core::{
    command::{CommandId, MenuEntry, MenuModel},
    layout::{fit_toolbar, ToolbarSlot},
};
use std::{cell::RefCell, collections::HashMap, rc::Rc};

type SegmentAction = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type OverflowAction = (String, Rc<dyn Fn(&mut Window, &mut App)>);

/// User-visible toolbar presentation. The host supplies its height from the
/// active theme; item controls keep their standard control size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolbarDisplayMode {
    /// Template images without visible labels.
    IconOnly,
    /// Template images with their external labels.
    #[default]
    IconAndLabel,
    /// External labels without template images.
    TextOnly,
}

/// Window-frame bezel used for command, toggle, segmented, and pop-down items.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ToolbarItemStyle {
    /// Rectangular textured control, the default for consistent toolbar use.
    #[default]
    Rectangular,
    /// Capsule-style control. Prefer a consistent style across one toolbar.
    Capsule,
}

#[derive(Clone)]
enum ToolbarItemKind {
    Command {
        command: CommandId,
        image: Option<TemplateImage>,
        toggle_help: Option<(SharedString, SharedString)>,
    },
    Segmented {
        segments: Vec<ToolbarSegment>,
        selected: Option<SharedString>,
        on_select: SegmentAction,
    },
    PopDown {
        menu: MenuModel,
        image: Option<TemplateImage>,
    },
    Search(Entity<TextField>),
    Group(Vec<ToolbarItem>),
    FixedSpace(f32),
    FlexibleSpace,
}

/// One segment in a toolbar's equal-width segmented control.
#[derive(Clone)]
pub struct ToolbarSegment {
    pub id: SharedString,
    pub label: SharedString,
    pub image: Option<TemplateImage>,
    pub disabled: bool,
    pub command: Option<CommandId>,
}

impl ToolbarSegment {
    /// Create a stable segment with its external label.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            image: None,
            disabled: false,
            command: None,
        }
    }

    /// Set the template image; if any segment lacks an image, all segments use labels.
    pub fn image(mut self, image: TemplateImage) -> Self {
        self.image = Some(image);
        self
    }

    /// Disable this segment independently of its bound command state.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Bind this segment to a command descriptor for checked/enabled state and
    /// activation through the responder chain.
    pub fn command(mut self, command: impl Into<CommandId>) -> Self {
        self.command = Some(command.into());
        self
    }
}

/// A typed toolbar item. `label` is the external accessibility/customization
/// name; it is separate from the command title and never inferred from an icon.
#[derive(Clone)]
pub struct ToolbarItem {
    id: SharedString,
    label: SharedString,
    priority: u8,
    style: ToolbarItemStyle,
    kind: ToolbarItemKind,
}

impl ToolbarItem {
    /// A command button whose current state and activation use `CommandRegistry`.
    pub fn command(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: impl Into<CommandId>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: 128,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::Command {
                command: command.into(),
                image: None,
                toggle_help: None,
            },
        }
    }

    /// A command toggle. Checked state remains owned by the command descriptor.
    pub fn toggle(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: impl Into<CommandId>,
    ) -> Self {
        Self::command(id, label, command)
    }

    /// Set the command-backed toggle's help text for its on and off states.
    /// The active text follows the checked state published by the registry.
    pub fn toggle_help(
        mut self,
        when_on: impl Into<SharedString>,
        when_off: impl Into<SharedString>,
    ) -> Self {
        if let ToolbarItemKind::Command { toggle_help, .. } = &mut self.kind {
            *toggle_help = Some((when_on.into(), when_off.into()));
        }
        self
    }

    /// A single-choice toolbar segmented control.
    pub fn segmented(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        segments: Vec<ToolbarSegment>,
        selected: Option<impl Into<SharedString>>,
        on_select: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: 128,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::Segmented {
                segments,
                selected: selected.map(Into::into),
                on_select: Rc::new(on_select),
            },
        }
    }

    /// A pop-down menu of commands, hosted by the shared transient menu system.
    pub fn pop_down(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        menu: MenuModel,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: 128,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::PopDown { menu, image: None },
        }
    }

    /// A real search field. If it overflows, its menu row returns focus here.
    pub fn search(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        field: Entity<TextField>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: 255,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::Search(field),
        }
    }

    /// A group keeps related controls together in the toolbar and overflow menu.
    pub fn group(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        items: Vec<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: 128,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::Group(items),
        }
    }

    /// A fixed-width spacer. The consumer supplies the desired logical width.
    pub fn fixed_space(id: impl Into<SharedString>, width: f32) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            priority: 255,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::FixedSpace(width.max(0.0)),
        }
    }

    /// A flexible spacer that absorbs remaining toolbar width.
    pub fn flexible_space(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            priority: 255,
            style: ToolbarItemStyle::default(),
            kind: ToolbarItemKind::FlexibleSpace,
        }
    }

    /// Lower-priority items move to overflow first. Equal-priority items
    /// overflow from right to left, preserving the order of those that remain.
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Select rectangular/textured (default) or capsule presentation.
    pub fn style(mut self, style: ToolbarItemStyle) -> Self {
        self.style = style;
        self
    }

    /// Use a monochrome template image in the fixed toolbar slot.
    pub fn image(mut self, image: TemplateImage) -> Self {
        match &mut self.kind {
            ToolbarItemKind::Command { image: slot, .. }
            | ToolbarItemKind::PopDown { image: slot, .. } => *slot = Some(image),
            _ => {}
        }
        self
    }

    /// Stable identifier used for element identity and customization storage.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// External descriptive label used for accessibility and display modes.
    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

#[derive(Default)]
struct ToolbarMeasurements {
    signature: Vec<String>,
    widths: Vec<Option<f32>>,
    overflow_menu: Option<Entity<PopupMenu>>,
    pop_down_menus: HashMap<String, Entity<PopupMenu>>,
}

/// A standard toolbar. Its containing view must observe the command registry
/// and responder chain so presentation changes are repainted.
#[derive(IntoElement)]
pub struct Toolbar {
    id: ElementId,
    label: SharedString,
    registry: Entity<CommandRegistry>,
    chain: Option<Entity<ResponderChain>>,
    items: Vec<ToolbarItem>,
    mode: ToolbarDisplayMode,
}

impl Toolbar {
    /// Create a toolbar from standard, typed items.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        registry: Entity<CommandRegistry>,
        items: Vec<ToolbarItem>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            registry,
            chain: None,
            items,
            mode: ToolbarDisplayMode::default(),
        }
    }

    /// Route command items through the window's responder chain.
    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.chain = Some(chain);
        self
    }

    /// Select the presentation shared by toolbar items.
    pub fn display_mode(mut self, mode: ToolbarDisplayMode) -> Self {
        self.mode = mode;
        self
    }

    /// Minimum width that keeps the overflow control itself reachable.
    #[must_use]
    pub fn minimum_width(&self, cx: &App) -> f32 {
        cx.theme().metrics.textured_height
    }

    /// Height selected by this toolbar's display mode and active appearance.
    #[must_use]
    pub fn height(&self, cx: &App) -> f32 {
        let metrics = &cx.theme().metrics;
        if self.mode == ToolbarDisplayMode::IconOnly {
            metrics.toolbar_height
        } else {
            metrics.toolbar_labelled_height
        }
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let registry = self.registry.clone();
        let chain = self.chain.clone();
        let items = self.items;
        let signature: Vec<String> = items.iter().map(|item| item.id.to_string()).collect();
        let measurements = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            cx.observe(&registry, |_, _, cx| cx.notify()).detach();
            if let Some(chain) = &chain {
                cx.observe(chain, |_, _, cx| cx.notify()).detach();
            }
            ToolbarMeasurements {
                signature: signature.clone(),
                widths: vec![None; signature.len()],
                overflow_menu: None,
                pop_down_menus: HashMap::new(),
            }
        });
        measurements.update(cx, |measurements, cx| {
            if measurements.signature != signature {
                measurements.signature = signature.clone();
                measurements.widths = vec![None; signature.len()];
                cx.notify();
            }
        });
        let overflow_menu = measurements
            .read(cx)
            .overflow_menu
            .clone()
            .unwrap_or_else(|| {
                let menu = cx.new(|cx| {
                    PopupMenu::new(
                        MenuModel {
                            label: "Toolbar Items".into(),
                            entries: Vec::new(),
                        },
                        registry.clone(),
                        window,
                        cx,
                    )
                });
                if let Some(chain) = &chain {
                    menu.update(cx, |menu, cx| menu.set_chain(chain.clone(), cx));
                }
                measurements.update(cx, |measurements, _| {
                    measurements.overflow_menu = Some(menu.clone());
                });
                menu
            });
        let mut pop_down_menus = measurements.read(cx).pop_down_menus.clone();
        ensure_pop_down_menus(
            &items,
            &registry,
            chain.as_ref(),
            &mut pop_down_menus,
            window,
            cx,
        );
        measurements.update(cx, |measurements, _| {
            measurements.pop_down_menus = pop_down_menus.clone();
        });
        let measured_widths = measurements.read(cx).widths.clone();
        let mode = self.mode;
        let label = self.label;
        let id = self.id;
        container_query(move |size, window, cx| {
            let theme = cx.theme().clone();
            let toolbar_height = if mode == ToolbarDisplayMode::IconOnly {
                theme.metrics.toolbar_height
            } else {
                theme.metrics.toolbar_labelled_height
            };
            let overflow_width = theme.metrics.textured_height;
            let gap = 8.0;
            let slots: Vec<ToolbarSlot> = items
                .iter()
                .enumerate()
                .map(|(index, item)| ToolbarSlot {
                    width: match item.kind {
                        ToolbarItemKind::FlexibleSpace => 0.0,
                        _ => measured_widths
                            .get(index)
                            .and_then(|width| *width)
                            .unwrap_or(0.0),
                    },
                    priority: item.priority,
                })
                .collect();
            let visible = fit_toolbar(&slots, size.width.as_f32(), gap, overflow_width);
            let mut row = div()
                .id(id.clone())
                .role(gpui::Role::Toolbar)
                .aria_label(label.clone())
                .flex()
                .items_center()
                .gap(px(gap))
                .h(px(toolbar_height))
                .w_full();

            let overflow_entries = overflow_menu_entries(&items, &visible);
            let overflow = !overflow_entries.is_empty();
            let mut overflow_actions = Vec::new();
            for (index, item) in items.iter().enumerate() {
                if !visible[index] {
                    collect_overflow_actions(item, cx, &mut overflow_actions);
                }
            }
            overflow_menu.update(cx, |menu, cx| {
                menu.set_model(
                    MenuModel {
                        label: "Toolbar Items".into(),
                        entries: overflow_entries,
                    },
                    window,
                    cx,
                );
                menu.clear_actions();
            });
            for (action_id, action) in overflow_actions {
                overflow_menu.update(cx, |menu, _| {
                    menu.on_action(action_id, move |w, cx| action(w, cx))
                });
            }

            for (index, item) in items.iter().enumerate() {
                if !visible[index] {
                    continue;
                }
                let element = render_item(
                    item,
                    mode,
                    &registry,
                    chain.as_ref(),
                    &pop_down_menus,
                    window,
                    cx,
                );
                let measured = measurements.clone();
                let mut child = match &item.kind {
                    ToolbarItemKind::FlexibleSpace => {
                        div().relative().flex_1().min_w_0().child(element)
                    }
                    _ => div().relative().flex_none().child(element),
                };
                child = child.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, _, cx| {
                            let width = f32::from(bounds.size.width);
                            measured.update(cx, |state, cx| {
                                let current = state.widths.get_mut(index);
                                if let Some(current) = current {
                                    let changed = match current {
                                        Some(old) => (*old - width).abs() > 0.5,
                                        None => true,
                                    };
                                    if changed {
                                        *current = Some(width);
                                        cx.notify();
                                    }
                                }
                            });
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
                row = row.child(child);
            }

            if overflow {
                let menu = overflow_menu.clone();
                let bounds_state = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
                let click_bounds = bounds_state.clone();
                let click_menu = menu.clone();
                let button = Button::new("toolbar-overflow", "Toolbar Items")
                    .style(ButtonStyle::Textured)
                    .size(ControlSize::Regular)
                    .icon(TemplateImage::Overflow.button_icon_renderer(TemplateImageSlot::Toolbar))
                    .on_click(move |_, window, cx| {
                        if let Some(bounds) = *click_bounds.borrow() {
                            click_menu.update(cx, |menu, cx| {
                                menu.open_pop_down(bounds.bottom_left(), window, cx);
                            });
                        }
                    });
                row = row.child(
                    div()
                        .relative()
                        .flex_none()
                        .child(button)
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |rect, _, _, _| *bounds_state.borrow_mut() = Some(rect),
                            )
                            .absolute()
                            .inset_0(),
                        )
                        .child(deferred(menu).priority(1)),
                );
            }
            row
        })
    }
}

fn render_item(
    item: &ToolbarItem,
    mode: ToolbarDisplayMode,
    registry: &Entity<CommandRegistry>,
    chain: Option<&Entity<ResponderChain>>,
    pop_down_menus: &HashMap<String, Entity<PopupMenu>>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let show_icon = mode != ToolbarDisplayMode::TextOnly;
    let show_label = mode != ToolbarDisplayMode::IconOnly;
    match &item.kind {
        ToolbarItemKind::Command {
            command,
            image,
            toggle_help,
        } => {
            let image = image.filter(|_| show_icon);
            let descriptor = registry.read(cx).descriptors().get(command).cloned();
            let mut button = CommandButton::new(item.id.clone(), command.clone(), registry.clone())
                .external_label(item.label.clone())
                .style(match item.style {
                    ToolbarItemStyle::Rectangular => ButtonStyle::Textured,
                    ToolbarItemStyle::Capsule => ButtonStyle::Round,
                })
                .presentation(image.is_some(), show_label || image.is_none());
            if let Some((on, off)) = toggle_help {
                let help = if descriptor
                    .as_ref()
                    .is_some_and(|descriptor| descriptor.checked)
                {
                    on.clone()
                } else {
                    off.clone()
                };
                button = button.tooltip(help);
            } else if let Some(help) = descriptor.and_then(|descriptor| descriptor.tooltip) {
                button = button.tooltip(help);
            }
            if let Some(chain) = chain {
                button = button.chain(chain.clone());
            }
            if let Some(image) = image {
                button = button.template_image(image);
            }
            button.into_any_element()
        }
        ToolbarItemKind::Segmented {
            segments,
            selected,
            on_select,
        } => {
            let show_segment_icons =
                show_icon && segments.iter().all(|segment| segment.image.is_some());
            let show_segment_labels = show_label || !show_segment_icons;
            let selected = if segments.iter().any(|segment| segment.command.is_some()) {
                segments.iter().find_map(|segment| {
                    let command = segment.command.as_ref()?;
                    registry
                        .read(cx)
                        .descriptors()
                        .get(command)
                        .is_some_and(|descriptor| descriptor.checked)
                        .then(|| segment.id.clone())
                })
            } else {
                selected.clone()
            };
            let item_segments = segments;
            let segments = item_segments
                .iter()
                .map(|segment| {
                    let command_disabled = segment.command.as_ref().is_some_and(|command| {
                        !registry
                            .read(cx)
                            .descriptors()
                            .get(command)
                            .is_some_and(|descriptor| descriptor.enabled)
                    });
                    let mut value = Segment::new(segment.id.clone(), segment.label.clone())
                        .disabled(segment.disabled || command_disabled)
                        .show_label(show_segment_labels);
                    if let Some(image) = segment.image.filter(|_| show_segment_icons) {
                        value = value.icon(image.role());
                    }
                    value
                })
                .collect();
            let mut control = SegmentedControl::new(item.id.clone(), item.label.clone(), segments)
                .mode(SegmentMode::SelectOne)
                .style(match item.style {
                    ToolbarItemStyle::Rectangular => SegmentStyle::Textured,
                    ToolbarItemStyle::Capsule => SegmentStyle::Rounded,
                })
                .size(ControlSize::Regular)
                .on_select({
                    let on_select = on_select.clone();
                    let segments = item_segments.clone();
                    let registry = registry.clone();
                    let chain = chain.cloned();
                    move |id, window, cx| {
                        if let Some(command) = segments
                            .iter()
                            .find(|segment| segment.id == *id)
                            .and_then(|segment| segment.command.as_ref())
                        {
                            if let Some(chain) = &chain {
                                let chain = chain.read(cx).clone();
                                chain.dispatch(&registry, command, window, cx);
                            } else {
                                CommandRegistry::invoke(&registry, command, window, cx);
                            }
                        } else {
                            on_select(id, window, cx);
                        }
                    }
                })
                .icon_renderer(|role, ink| {
                    TemplateImage::from_role(role)
                        .map(|image| image.element(TemplateImageSlot::Segmented, ink))
                        .unwrap_or_else(|| div().into_any_element())
                });
            if let Some(selected) = selected {
                control = control.selected(selected.clone());
            }
            control.into_any_element()
        }
        ToolbarItemKind::PopDown { menu, image } => render_pop_down(
            item,
            menu.clone(),
            image.filter(|_| show_icon),
            item.style,
            pop_down_menus
                .get(item.id.as_ref())
                .expect("pop-down menu state initialized before rendering")
                .clone(),
            window,
            cx,
        ),
        ToolbarItemKind::Search(field) => {
            field.update(cx, |field, cx| {
                field.set_label(item.label.clone(), cx);
                field.set_search(true, cx);
            });
            field.clone().into_any_element()
        }
        ToolbarItemKind::Group(items) => {
            let mut group = div().flex().flex_none().items_center().gap(px(8.0));
            for item in items {
                group = group.child(render_item(
                    item,
                    mode,
                    registry,
                    chain,
                    pop_down_menus,
                    window,
                    cx,
                ));
            }
            group.into_any_element()
        }
        ToolbarItemKind::FixedSpace(width) => div().flex_none().w(px(*width)).into_any_element(),
        ToolbarItemKind::FlexibleSpace => div().flex_1().min_w_0().into_any_element(),
    }
}

fn render_pop_down(
    item: &ToolbarItem,
    model: MenuModel,
    image: Option<TemplateImage>,
    style: ToolbarItemStyle,
    menu: Entity<PopupMenu>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    menu.update(cx, |menu, cx| menu.set_model(model, window, cx));
    let bounds = Rc::new(RefCell::new(None::<Bounds<Pixels>>));
    let click_bounds = bounds.clone();
    let click_menu = menu.clone();
    let mut button = Button::new(item.id.clone(), item.label.clone())
        .style(match style {
            ToolbarItemStyle::Rectangular => ButtonStyle::Textured,
            ToolbarItemStyle::Capsule => ButtonStyle::Round,
        })
        .size(ControlSize::Regular)
        .on_click(move |_, window, cx| {
            if let Some(bounds) = *click_bounds.borrow() {
                click_menu.update(cx, |menu, cx| {
                    menu.open_pop_down(bounds.bottom_left(), window, cx);
                });
            }
        });
    if let Some(image) = image {
        button = button.icon(image.button_icon_renderer(TemplateImageSlot::Toolbar));
    }
    div()
        .relative()
        .flex_none()
        .child(button)
        .child(
            canvas(
                |_, _, _| (),
                move |rect, _, _, _| *bounds.borrow_mut() = Some(rect),
            )
            .absolute()
            .inset_0(),
        )
        .child(deferred(menu).priority(1))
        .into_any_element()
}

fn ensure_pop_down_menus(
    items: &[ToolbarItem],
    registry: &Entity<CommandRegistry>,
    chain: Option<&Entity<ResponderChain>>,
    menus: &mut HashMap<String, Entity<PopupMenu>>,
    window: &mut Window,
    cx: &mut App,
) {
    for item in items {
        match &item.kind {
            ToolbarItemKind::PopDown { menu: model, .. } => {
                if !menus.contains_key(item.id.as_ref()) {
                    let menu =
                        cx.new(|cx| PopupMenu::new(model.clone(), registry.clone(), window, cx));
                    if let Some(chain) = chain {
                        menu.update(cx, |menu, cx| menu.set_chain(chain.clone(), cx));
                    }
                    menus.insert(item.id.to_string(), menu);
                }
            }
            ToolbarItemKind::Group(children) => {
                ensure_pop_down_menus(children, registry, chain, menus, window, cx);
            }
            _ => {}
        }
    }
}

fn overflow_menu_entries(items: &[ToolbarItem], visible: &[bool]) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if visible[index] {
            continue;
        }
        match &item.kind {
            ToolbarItemKind::Command { command, .. } => {
                entries.push(MenuEntry::Command(command.clone()));
            }
            ToolbarItemKind::Segmented { segments, .. } => {
                for segment in segments {
                    if let Some(command) = &segment.command {
                        entries.push(MenuEntry::Command(command.clone()));
                    } else {
                        entries.push(MenuEntry::Action {
                            id: format!("toolbar-segment-{}-{}", item.id, segment.id),
                            label: segment.label.to_string(),
                        });
                    }
                }
            }
            ToolbarItemKind::PopDown { menu, .. } => {
                entries.push(MenuEntry::Submenu {
                    label: item.label.to_string(),
                    entries: menu.entries.clone(),
                });
            }
            ToolbarItemKind::Search(_) => entries.push(MenuEntry::Action {
                id: format!("toolbar-search-{}", item.id),
                label: item.label.to_string(),
            }),
            ToolbarItemKind::Group(items) => entries.push(MenuEntry::Submenu {
                label: item.label.to_string(),
                entries: items.iter().flat_map(overflow_entries_for_item).collect(),
            }),
            ToolbarItemKind::FixedSpace(_) | ToolbarItemKind::FlexibleSpace => {}
        }
    }
    entries
}

fn overflow_entries_for_item(item: &ToolbarItem) -> Vec<MenuEntry> {
    match &item.kind {
        ToolbarItemKind::Command { command, .. } => vec![MenuEntry::Command(command.clone())],
        ToolbarItemKind::Segmented { segments, .. } => segments
            .iter()
            .map(|segment| match &segment.command {
                Some(command) => MenuEntry::Command(command.clone()),
                None => MenuEntry::Action {
                    id: format!("toolbar-segment-{}-{}", item.id, segment.id),
                    label: segment.label.to_string(),
                },
            })
            .collect(),
        ToolbarItemKind::PopDown { menu, .. } => vec![MenuEntry::Submenu {
            label: item.label.to_string(),
            entries: menu.entries.clone(),
        }],
        ToolbarItemKind::Search(_) => vec![MenuEntry::Action {
            id: format!("toolbar-search-{}", item.id),
            label: item.label.to_string(),
        }],
        ToolbarItemKind::Group(items) => items.iter().flat_map(overflow_entries_for_item).collect(),
        ToolbarItemKind::FixedSpace(_) | ToolbarItemKind::FlexibleSpace => Vec::new(),
    }
}

fn collect_overflow_actions(item: &ToolbarItem, cx: &App, actions: &mut Vec<OverflowAction>) {
    match &item.kind {
        ToolbarItemKind::Search(field) => {
            use gpui::Focusable;
            let focus = field.read(cx).focus_handle(cx);
            actions.push((
                format!("toolbar-search-{}", item.id),
                Rc::new(move |window: &mut Window, cx: &mut App| {
                    window.focus(&focus, cx);
                }),
            ));
        }
        ToolbarItemKind::Segmented {
            segments,
            on_select,
            ..
        } => {
            for segment in segments
                .iter()
                .filter(|segment| segment.command.is_none() && !segment.disabled)
            {
                let on_select = on_select.clone();
                let segment_id = segment.id.clone();
                actions.push((
                    format!("toolbar-segment-{}-{}", item.id, segment.id),
                    Rc::new(move |window: &mut Window, cx: &mut App| {
                        on_select(&segment_id, window, cx);
                    }),
                ));
            }
        }
        ToolbarItemKind::Group(items) => {
            for item in items {
                collect_overflow_actions(item, cx, actions);
            }
        }
        _ => {}
    }
}
