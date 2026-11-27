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
    icons::{
        ConsumerArtwork, ConsumerTemplateImage, PictorialImage, PictorialState, TemplateImage,
        TemplateImageSlot,
    },
    menu::PopupMenu,
    responder::ResponderChain,
    theme::ActiveTheme,
};
use gpui::{
    canvas, container_query, deferred, div, prelude::*, px, AnyElement, App, Bounds, ElementId,
    Entity, Pixels, RenderOnce, SharedString, TextRun, Window,
};
use silvestris_core::{
    command::{CommandId, MenuEntry, MenuModel},
    layout::{fit_toolbar, ToolbarSlot},
};
use std::{cell::Cell, cell::RefCell, collections::BTreeMap, collections::HashMap, rc::Rc};

type SegmentAction = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;
type OverflowAction = (String, Rc<dyn Fn(&mut Window, &mut App)>);
type LayoutAction = Rc<dyn Fn(ToolbarLayout, &mut Window, &mut App)>;
type PaletteAction = Rc<dyn Fn(ToolbarLayout, &mut Window, &mut App)>;

/// One toolbar's saved arrangement. `order` lists item ids left to right;
/// `hidden` removes optional items; `mode` remembers the display mode when the
/// user changed it.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolbarLayout {
    pub order: Vec<String>,
    pub hidden: Vec<String>,
    pub mode: Option<ToolbarDisplayMode>,
}

impl ToolbarLayout {
    /// The declared arrangement: every top-level item in declaration order,
    /// nothing hidden, no display-mode override.
    #[must_use]
    pub fn default_for(items: &[ToolbarItem]) -> Self {
        Self {
            order: items.iter().map(|item| item.id.to_string()).collect(),
            hidden: Vec::new(),
            mode: None,
        }
    }

    #[must_use]
    pub fn shows(&self, id: &str) -> bool {
        !self.hidden.iter().any(|hidden| hidden == id)
    }
}

/// Saved toolbar arrangements by key. Put one field of this type in the
/// application's preferences document.
pub type SavedToolbars = BTreeMap<String, ToolbarLayout>;

/// Where a toolbar reads and writes its arrangement.
pub trait ToolbarStore {
    fn load(&self, key: &str, cx: &App) -> Option<ToolbarLayout>;
    fn save(&self, key: &str, layout: ToolbarLayout, cx: &mut App);
    /// Restore the declared default set.
    fn clear(&self, key: &str, cx: &mut App);
}

/// Keeps arrangements in memory: for tests and apps without a preference
/// document.
#[derive(Default)]
pub struct MemoryToolbars {
    layouts: RefCell<SavedToolbars>,
}

impl ToolbarStore for MemoryToolbars {
    fn load(&self, key: &str, _: &App) -> Option<ToolbarLayout> {
        self.layouts.borrow().get(key).cloned()
    }
    fn save(&self, key: &str, layout: ToolbarLayout, _: &mut App) {
        self.layouts.borrow_mut().insert(key.to_string(), layout);
    }
    fn clear(&self, key: &str, _: &mut App) {
        self.layouts.borrow_mut().remove(key);
    }
}

/// The value a customizable toolbar item offers to GPUI's drag system. A
/// palette item uses `from_palette` to add itself instead of moving.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolbarItemDrag {
    pub id: SharedString,
    pub from_palette: bool,
}

/// User-visible toolbar presentation. The host supplies its height from the
/// active theme; item controls keep their standard control size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// A monochrome glyph accepted by textured/capsule controls and segmented
/// controls. Both origins receive the host's state ink and the fixed 19 px
/// toolbar or 13 px segmented slot.
#[derive(Clone)]
pub enum ToolbarGlyph {
    /// A Silvestris-provided template image.
    Silvestris(TemplateImage),
    /// Application-provided artwork, painted with host ink.
    Consumer(ConsumerTemplateImage),
}

impl ToolbarGlyph {
    /// Render in the fixed host slot with the control's current ink.
    pub fn element(&self, slot: TemplateImageSlot, ink: silvestris_core::Rgba8) -> AnyElement {
        match self {
            Self::Silvestris(image) => image.element(slot, ink),
            Self::Consumer(image) => image.element(slot, ink),
        }
    }

    /// Renderer callback for `Button::icon`/`CommandButton::icon`.
    pub fn button_renderer(
        &self,
        slot: TemplateImageSlot,
    ) -> impl Fn(gpui::Rgba) -> AnyElement + 'static {
        let render: Rc<dyn Fn(gpui::Rgba) -> AnyElement> = match self {
            Self::Silvestris(image) => {
                let image = *image;
                Rc::new(move |ink| image.element(slot, rgba8(ink)))
            }
            Self::Consumer(image) => {
                let image = image.clone();
                Rc::new(move |ink| image.element(slot, rgba8(ink)))
            }
        };
        move |ink| render(ink)
    }
}

fn rgba8(ink: gpui::Rgba) -> silvestris_core::Rgba8 {
    silvestris_core::Rgba8 {
        r: (ink.r * 255.0).round().clamp(0.0, 255.0) as u8,
        g: (ink.g * 255.0).round().clamp(0.0, 255.0) as u8,
        b: (ink.b * 255.0).round().clamp(0.0, 255.0) as u8,
        a: (ink.a * 255.0).round().clamp(0.0, 255.0) as u8,
    }
}

impl From<TemplateImage> for ToolbarGlyph {
    fn from(image: TemplateImage) -> Self {
        Self::Silvestris(image)
    }
}

impl From<ConsumerTemplateImage> for ToolbarGlyph {
    fn from(image: ConsumerTemplateImage) -> Self {
        Self::Consumer(image)
    }
}

/// Full-colour artwork accepted only by the standalone toolbar icon-button
/// presentation. A 32 px pictorial image is never valid in a 19 px textured
/// slot.
#[derive(Clone)]
pub enum ToolbarPictorial {
    /// Silvestris-provided pictorial artwork.
    Silvestris(PictorialImage),
    /// Application-provided pictorial artwork.
    Consumer(ConsumerArtwork),
}

impl ToolbarPictorial {
    /// Render the artwork at the fixed 32 px slot for a state.
    pub fn element(&self, state: PictorialState, window: &mut Window, cx: &mut App) -> AnyElement {
        match self {
            Self::Silvestris(image) => image.element(state),
            Self::Consumer(artwork) => artwork.stateful_toolbar_element(state, window, cx),
        }
    }

    /// The artwork's own descriptive name, when it has one.
    #[must_use]
    pub fn label(&self) -> Option<&SharedString> {
        match self {
            Self::Silvestris(_) => None,
            Self::Consumer(artwork) => Some(artwork.label()),
        }
    }
}

impl From<PictorialImage> for ToolbarPictorial {
    fn from(image: PictorialImage) -> Self {
        Self::Silvestris(image)
    }
}

impl From<ConsumerArtwork> for ToolbarPictorial {
    fn from(artwork: ConsumerArtwork) -> Self {
        Self::Consumer(artwork)
    }
}

#[derive(Clone)]
enum ToolbarItemKind {
    Command {
        command: CommandId,
        glyph: Option<ToolbarGlyph>,
        toggle_help: Option<(SharedString, SharedString)>,
    },
    /// Standalone pictorial command: the label sits below the artwork.
    PictorialCommand {
        command: CommandId,
        artwork: ToolbarPictorial,
        toggle_help: Option<(SharedString, SharedString)>,
    },
    Segmented {
        segments: Vec<ToolbarSegment>,
        selected: Option<SharedString>,
        on_select: SegmentAction,
    },
    PopDown {
        menu: MenuModel,
        glyph: Option<ToolbarGlyph>,
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
    pub glyph: Option<ToolbarGlyph>,
    pub disabled: bool,
    pub command: Option<CommandId>,
}

impl ToolbarSegment {
    /// Create a stable segment with its external label.
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            glyph: None,
            disabled: false,
            command: None,
        }
    }

    /// Set a Silvestris template image; if any segment lacks a glyph, all
    /// segments use labels.
    pub fn image(mut self, image: TemplateImage) -> Self {
        self.glyph = Some(ToolbarGlyph::Silvestris(image));
        self
    }

    /// Set a monochrome glyph, global or application-provided. If any segment
    /// lacks a glyph, all segments use labels.
    pub fn glyph(mut self, glyph: impl Into<ToolbarGlyph>) -> Self {
        self.glyph = Some(glyph.into());
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
    /// Whether user customization may hide this item. Required toolbar items
    /// remain fixed and are not offered by the palette.
    optional: bool,
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
            optional: false,
            kind: ToolbarItemKind::Command {
                command: command.into(),
                glyph: None,
                toggle_help: None,
            },
        }
    }

    /// A standalone pictorial command: 32 px artwork with the external label
    /// below it in Icon & Label mode. Uses the same command registry and
    /// responder chain as a rectangular command.
    pub fn pictorial_command(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: impl Into<CommandId>,
        artwork: impl Into<ToolbarPictorial>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            priority: 128,
            style: ToolbarItemStyle::default(),
            optional: false,
            kind: ToolbarItemKind::PictorialCommand {
                command: command.into(),
                artwork: artwork.into(),
                toggle_help: None,
            },
        }
    }

    /// A standalone pictorial toggle. Checked state remains owned by the
    /// command descriptor.
    pub fn pictorial_toggle(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: impl Into<CommandId>,
        artwork: impl Into<ToolbarPictorial>,
    ) -> Self {
        Self::pictorial_command(id, label, command, artwork)
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
        match &mut self.kind {
            ToolbarItemKind::Command { toggle_help, .. }
            | ToolbarItemKind::PictorialCommand { toggle_help, .. } => {
                *toggle_help = Some((when_on.into(), when_off.into()));
            }
            _ => {}
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
            optional: false,
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
            optional: false,
            kind: ToolbarItemKind::PopDown { menu, glyph: None },
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
            optional: false,
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
            optional: false,
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
            optional: false,
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
            optional: false,
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

    /// Mark this item as optional: user customization may hide it, and the
    /// customization palette offers it. Required items stay fixed.
    pub fn optional(mut self, optional: bool) -> Self {
        self.optional = optional;
        self
    }

    #[must_use]
    pub fn is_optional(&self) -> bool {
        self.optional
    }

    /// Use a Silvestris monochrome template image in the fixed toolbar slot.
    /// Convenience wrapper around [`Self::glyph`].
    pub fn image(mut self, image: TemplateImage) -> Self {
        self = self.glyph(ToolbarGlyph::Silvestris(image));
        self
    }

    /// Use a monochrome glyph, global or application-provided. Valid for
    /// rectangular/capsule commands, pop-downs and segments; not for pictorial
    /// items.
    pub fn glyph(mut self, glyph: impl Into<ToolbarGlyph>) -> Self {
        let glyph = Some(glyph.into());
        match &mut self.kind {
            ToolbarItemKind::Command { glyph: slot, .. }
            | ToolbarItemKind::PopDown { glyph: slot, .. } => *slot = glyph,
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
    bounds: Vec<Option<Bounds<Pixels>>>,
    overflow_menu: Option<Entity<PopupMenu>>,
    pop_down_menus: HashMap<String, Entity<PopupMenu>>,
    /// The insertion caret while an item is dragged over the toolbar.
    insertion: Option<usize>,
    /// The arrangement applied by the last customization drop.
    layout: Option<ToolbarLayout>,
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
    layout: Option<ToolbarLayout>,
    store: Option<(Rc<dyn ToolbarStore>, String)>,
    on_layout_change: Option<LayoutAction>,
    customizable: bool,
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
            layout: None,
            store: None,
            on_layout_change: None,
            customizable: true,
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

    /// Use an explicit arrangement. Without one, a stored arrangement is
    /// loaded when `autosave` is set; otherwise the declared order applies.
    pub fn layout(mut self, layout: ToolbarLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    /// Persist customization under `key`. The consumer still observes the
    /// registry; the toolbar writes a `ToolbarLayout` on each customization.
    pub fn autosave(mut self, store: Rc<dyn ToolbarStore>, key: impl Into<String>) -> Self {
        self.store = Some((store, key.into()));
        self
    }

    /// Notify the consumer of the new arrangement after a customization drop.
    pub fn on_layout_change(
        mut self,
        f: impl Fn(ToolbarLayout, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_layout_change = Some(Rc::new(f));
        self
    }

    /// `false` fixes the composition. The Preferences toolbar and other fixed
    /// hosts set this; the palette is not offered.
    pub fn customizable(mut self, customizable: bool) -> Self {
        self.customizable = customizable;
        self
    }

    #[must_use]
    pub fn is_customizable(&self) -> bool {
        self.customizable
    }

    /// Minimum width that keeps the overflow control itself reachable.
    #[must_use]
    pub fn minimum_width(&self, cx: &App) -> f32 {
        cx.theme().metrics.textured_height
    }

    /// Height selected by this toolbar's display mode and active appearance.
    /// A stored or explicit layout's mode override wins, so the frame height
    /// matches what `render` draws.
    #[must_use]
    pub fn height(&self, cx: &App) -> f32 {
        let metrics = &cx.theme().metrics;
        if self.effective_mode(cx) == ToolbarDisplayMode::IconOnly {
            metrics.toolbar_height
        } else {
            metrics.toolbar_labelled_height
        }
    }

    fn effective_mode(&self, cx: &App) -> ToolbarDisplayMode {
        let stored = self
            .store
            .as_ref()
            .and_then(|(store, key)| store.load(key, cx));
        self.layout
            .clone()
            .or(stored)
            .and_then(|layout| layout.mode)
            .unwrap_or(self.mode)
    }
}

impl RenderOnce for Toolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let registry = self.registry.clone();
        let chain = self.chain.clone();
        let declared = self.items;
        let store = self.store.clone();
        let on_layout_change = self.on_layout_change.clone();
        let customizable = self.customizable;
        let declared_signature: Vec<String> =
            declared.iter().map(|item| item.id.to_string()).collect();
        let measurements = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            cx.observe(&registry, |_, _, cx| cx.notify()).detach();
            if let Some(chain) = &chain {
                cx.observe(chain, |_, _, cx| cx.notify()).detach();
            }
            ToolbarMeasurements {
                signature: declared_signature.clone(),
                widths: vec![None; declared_signature.len()],
                bounds: vec![None; declared_signature.len()],
                overflow_menu: None,
                pop_down_menus: HashMap::new(),
                insertion: None,
                layout: None,
            }
        });
        // An explicit layout wins; then the stored one; then a layout applied
        // by the last customization drop when neither is present.
        let layout = self
            .layout
            .clone()
            .or_else(|| store.as_ref().and_then(|(store, key)| store.load(key, cx)));
        let layout = layout.or_else(|| measurements.read(cx).layout.clone());
        let mut mode = self.mode;
        let items = if let Some(layout) = &layout {
            mode = layout.mode.unwrap_or(mode);
            apply_layout(&declared, layout)
        } else {
            declared.clone()
        };
        let signature: Vec<String> = items.iter().map(|item| item.id.to_string()).collect();
        measurements.update(cx, |measurements, cx| {
            if measurements.signature != signature {
                measurements.signature = signature.clone();
                measurements.widths = vec![None; signature.len()];
                measurements.bounds = vec![None; signature.len()];
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
            let effective_ids: Vec<SharedString> =
                items.iter().map(|item| item.id.clone()).collect();
            let insertion = measurements.read(cx).insertion;
            let current_hidden = layout
                .as_ref()
                .map(|layout| layout.hidden.clone())
                .unwrap_or_default();
            let move_measurements = measurements.clone();
            let drop_measurements = measurements.clone();
            let drop_store = store.clone();
            let drop_change = on_layout_change.clone();
            let drop_ids = effective_ids.clone();
            let drop_mode = mode;
            let mut row = div()
                .id(id.clone())
                .role(gpui::Role::Toolbar)
                .aria_label(label.clone())
                .flex()
                .items_center()
                .gap(px(gap))
                .h(px(toolbar_height))
                .w_full()
                .on_drag_move::<ToolbarItemDrag>(move |event, _, cx| {
                    if !customizable || event.event.pressed_button != Some(gpui::MouseButton::Left)
                    {
                        return;
                    }
                    let x = event.event.position.x;
                    let insertion = {
                        let state = move_measurements.read(cx);
                        let mut index = 0;
                        let mut found = false;
                        for bounds in state.bounds.iter().flatten() {
                            if x < bounds.center().x {
                                found = true;
                                break;
                            }
                            index += 1;
                        }
                        if !found {
                            index = state.bounds.iter().flatten().count();
                        }
                        index
                    };
                    move_measurements.update(cx, |state, cx| {
                        if state.insertion != Some(insertion) {
                            state.insertion = Some(insertion);
                            cx.notify();
                        }
                    });
                })
                .on_drop::<ToolbarItemDrag>(move |drag, window, cx| {
                    let insertion = drop_measurements.read(cx).insertion;
                    let mut new_layout = ToolbarLayout {
                        order: drop_ids.iter().map(ToString::to_string).collect(),
                        hidden: current_hidden.clone(),
                        mode: Some(drop_mode),
                    };
                    apply_drag(&mut new_layout, drag, insertion);
                    if let Some((store, key)) = &drop_store {
                        store.save(key, new_layout.clone(), cx);
                    }
                    if let Some(change) = &drop_change {
                        change(new_layout.clone(), window, cx);
                    }
                    drop_measurements.update(cx, |state, cx| {
                        state.layout = Some(new_layout);
                        state.insertion = None;
                        cx.notify();
                    });
                });

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
                if insertion == Some(index) {
                    row = row.child(
                        div()
                            .flex_none()
                            .w(px(2.))
                            .h(px((toolbar_height - 8.).max(2.)))
                            .bg(crate::convert::hsla(theme.colors.divider)),
                    );
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
                let item_identity = SharedString::from(format!("toolbar-item-{}", item.id));
                let mut child = match &item.kind {
                    ToolbarItemKind::FlexibleSpace => div()
                        .id(item_identity)
                        .relative()
                        .flex_1()
                        .min_w_0()
                        .child(element),
                    _ => div()
                        .id(item_identity)
                        .relative()
                        .flex_none()
                        .child(element),
                };
                if customizable
                    && !matches!(
                        item.kind,
                        ToolbarItemKind::FixedSpace(_) | ToolbarItemKind::FlexibleSpace
                    )
                {
                    child = child.on_drag(
                        ToolbarItemDrag {
                            id: item.id.clone(),
                            from_palette: false,
                        },
                        |_, _, _, cx| cx.new(|_| gpui::EmptyView),
                    );
                }
                child = child.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, _, cx| {
                            let width = f32::from(bounds.size.width);
                            measured.update(cx, |state, cx| {
                                if let Some(current) = state.widths.get_mut(index) {
                                    let changed = match current {
                                        Some(old) => (*old - width).abs() > 0.5,
                                        None => true,
                                    };
                                    if changed {
                                        *current = Some(width);
                                        cx.notify();
                                    }
                                }
                                if let Some(current) = state.bounds.get_mut(index) {
                                    *current = Some(bounds);
                                }
                            });
                        },
                    )
                    .absolute()
                    .inset_0(),
                );
                row = row.child(child);
            }

            if insertion == Some(items.len()) {
                row = row.child(
                    div()
                        .flex_none()
                        .w(px(2.))
                        .h(px((toolbar_height - 8.).max(2.)))
                        .bg(crate::convert::hsla(theme.colors.divider)),
                );
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

/// Standalone toolbar icon button: the 32 px pictorial artwork with the
/// external label centred below in Icon & Label mode. There is no visible
/// bezel at rest; the control occludes the titlebar drag area, takes Tab focus
/// and dispatches the same command as the menu and shortcut paths.
#[derive(IntoElement)]
pub struct ToolbarIconButton {
    id: ElementId,
    label: SharedString,
    command: CommandId,
    registry: Entity<CommandRegistry>,
    chain: Option<Entity<ResponderChain>>,
    artwork: ToolbarPictorial,
    show_icon: bool,
    show_label: bool,
    tooltip: Option<SharedString>,
}

impl ToolbarIconButton {
    /// Create a pictorial command button for an existing command descriptor.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        command: impl Into<CommandId>,
        registry: Entity<CommandRegistry>,
        artwork: impl Into<ToolbarPictorial>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            command: command.into(),
            registry,
            chain: None,
            artwork: artwork.into(),
            show_icon: true,
            show_label: true,
            tooltip: None,
        }
    }

    /// Choose the artwork/label combination for the toolbar display mode.
    pub fn presentation(mut self, show_icon: bool, show_label: bool) -> Self {
        self.show_icon = show_icon;
        self.show_label = show_label;
        self
    }

    /// Resolve and dispatch through the window's responder chain.
    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.chain = Some(chain);
        self
    }

    /// Override the command descriptor's help text for this presentation.
    pub fn tooltip(mut self, text: impl Into<SharedString>) -> Self {
        self.tooltip = Some(text.into());
        self
    }
}

/// HIG p. 270: at least 10 px of hit target around the icon or label.
const ICON_BUTTON_MARGIN: f32 = 10.0;
/// HIG p. 270: standalone toolbar icon buttons use a 32×32 px image.
const ICON_BUTTON_SLOT: f32 = 32.0;

impl RenderOnce for ToolbarIconButton {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let descriptor = match &self.chain {
            Some(chain) => chain
                .read(cx)
                .present(&self.registry, &self.command, window, cx),
            None => self
                .registry
                .read(cx)
                .descriptors()
                .get(&self.command)
                .cloned(),
        };
        let enabled = descriptor.as_ref().is_some_and(|d| d.enabled);
        let checked = descriptor.as_ref().is_some_and(|d| d.checked);
        let tooltip = self.tooltip.or_else(|| {
            descriptor
                .as_ref()
                .and_then(|descriptor| descriptor.tooltip.clone().map(Into::into))
        });

        let feedback = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
            (cx.focus_handle().tab_stop(true), Rc::new(Cell::new(false)))
        });
        let (focus, pressed) = {
            let state = feedback.read(cx);
            (state.0.clone(), state.1.clone())
        };
        if !enabled {
            pressed.set(false);
        }

        let label_size = theme.type_.size_regular;
        let label_run = TextRun {
            len: self.label.len(),
            font: gpui::font(theme.type_.ui),
            color: crate::convert::hsla(theme.colors.text),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let label_width = window
            .text_system()
            .shape_line(self.label.clone(), px(label_size), &[label_run], None)
            .width
            .as_f32();
        let content_width = match (self.show_icon, self.show_label) {
            (true, true) => ICON_BUTTON_SLOT.max(label_width),
            (true, false) => ICON_BUTTON_SLOT,
            (false, true) => label_width,
            (false, false) => ICON_BUTTON_SLOT,
        };
        let width = (content_width + 2.0 * ICON_BUTTON_MARGIN).ceil();
        let height = if self.show_label {
            theme.metrics.toolbar_labelled_height
        } else {
            theme.metrics.toolbar_height
        };
        // The 49 px labelled envelope leaves this row under the 32 px image;
        // the artwork is top-aligned, never stretched.
        let label_row = (height - ICON_BUTTON_SLOT).max(0.0);

        let state = PictorialState {
            enabled,
            active_window: window.is_window_active(),
            pressed: pressed.get(),
            selected: checked,
        };
        let artwork = self.artwork.element(state, window, cx);
        let selector = self.id.to_string();
        let icon_selector = format!("{selector}-icon");
        let label_selector = format!("{selector}-label");
        let aria_label = if checked {
            format!("{}, selected", self.label)
        } else {
            self.label.to_string()
        };

        let registry = self.registry.clone();
        let command = self.command.clone();
        let chain = self.chain.clone();
        let click_focus = focus.clone();
        let click_pressed = pressed.clone();
        let key_pressed = pressed.clone();
        let up_pressed = pressed.clone();
        let release_pressed = pressed.clone();
        let ring_focus = focus.clone();
        let ring = theme.colors.focus_ring;
        let profile = theme.metrics.focus_ring;
        let radius = theme.metrics.textured_radius;

        div()
            .debug_selector(|| selector)
            .id(self.id)
            .role(gpui::Role::Button)
            .aria_label(aria_label)
            // Controls must stop an enclosing native titlebar drag hitbox.
            .occlude()
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(height))
            .w(px(width))
            .when_some(tooltip, crate::controls::tooltip::attach)
            .font_family(theme.type_.ui)
            .text_size(px(label_size))
            .text_color(crate::convert::hsla(theme.colors.text))
            .when(enabled, |el| el.track_focus(&focus))
            .on_mouse_down(gpui::MouseButton::Left, move |_, window, cx| {
                if enabled {
                    window.focus(&click_focus, cx);
                    click_pressed.set(true);
                    window.refresh();
                }
                cx.stop_propagation();
            })
            .on_mouse_up(gpui::MouseButton::Left, move |_, window, _| {
                if up_pressed.get() {
                    up_pressed.set(false);
                    window.refresh();
                }
            })
            .when_some(enabled.then(|| focus.clone()), |el, key_focus| {
                el.on_key_down(move |event, window, _| {
                    if !key_focus.is_focused(window) {
                        return;
                    }
                    if !window.default_prevented()
                        && !event.keystroke.modifiers.modified()
                        && matches!(event.keystroke.key.as_str(), "enter" | "space")
                    {
                        key_pressed.set(true);
                        window.refresh();
                    }
                })
            })
            .on_key_up(move |_, window, _| {
                if release_pressed.get() {
                    release_pressed.set(false);
                    window.refresh();
                }
            })
            .when(enabled, |el| {
                el.on_click(move |_, window, cx| {
                    pressed.set(false);
                    window.refresh();
                    cx.stop_propagation();
                    match &chain {
                        Some(chain) => {
                            let chain = chain.read(cx).clone();
                            chain.dispatch(&registry, &command, window, cx);
                        }
                        None => {
                            CommandRegistry::invoke(&registry, &command, window, cx);
                        }
                    }
                })
            })
            .when(self.show_icon, |el| {
                el.child(
                    div()
                        .id(SharedString::from(icon_selector.clone()))
                        .debug_selector(move || icon_selector)
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .w(px(ICON_BUTTON_SLOT))
                        .h(px(ICON_BUTTON_SLOT))
                        .child(artwork),
                )
            })
            .when(self.show_label, |el| {
                el.child(
                    div()
                        .id(SharedString::from(label_selector.clone()))
                        .debug_selector(move || label_selector)
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .w_full()
                        .h(px(label_row))
                        .line_height(px(label_row))
                        .text_align(gpui::TextAlign::Center)
                        .whitespace_nowrap()
                        .child(self.label),
                )
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        if ring_focus.is_focused(window) && window.last_input_was_keyboard() {
                            crate::paint::focus_ring(window, bounds, radius, ring, profile);
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

/// Lets the user add, remove and restore optional toolbar items. Host it in
/// the application's customization surface; it renders the external labels,
/// not a second copy of the toolbar. Pictorial items keep the palette's
/// labelled-chip preview rather than a 32 px image.
#[derive(IntoElement)]
pub struct ToolbarPalette {
    id: ElementId,
    label: SharedString,
    items: Vec<ToolbarItem>,
    layout: ToolbarLayout,
    on_change: Option<PaletteAction>,
    store: Option<(Rc<dyn ToolbarStore>, String)>,
}

impl ToolbarPalette {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        items: Vec<ToolbarItem>,
        layout: ToolbarLayout,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            items,
            layout,
            on_change: None,
            store: None,
        }
    }

    /// Report the new arrangement. The consumer feeds it back to `Toolbar`
    /// and to this palette; the palette itself is controlled.
    pub fn on_change(mut self, f: impl Fn(ToolbarLayout, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(Rc::new(f));
        self
    }

    /// Save changed arrangements under `key` as well as reporting them.
    pub fn autosave(mut self, store: Rc<dyn ToolbarStore>, key: impl Into<String>) -> Self {
        self.store = Some((store, key.into()));
        self
    }
}

impl RenderOnce for ToolbarPalette {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let hot = window.use_keyed_state(
            SharedString::from(format!("{}-remove-hot", self.id)),
            cx,
            |_, _| Rc::new(std::cell::Cell::new(false)),
        );
        let mut palette = div()
            .id(self.id.clone())
            .debug_selector(|| "toolbar-palette".into())
            .role(gpui::Role::Group)
            .aria_label(self.label.clone())
            .w_full()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.))
            .p(px(8.))
            .rounded(px(6.))
            .border_1()
            .border_color(crate::convert::hsla(theme.colors.group_border));
        for item in self.items.iter().filter(|item| item.optional) {
            let shown = self.layout.shows(&item.id);
            let store = self.store.clone();
            let change = self.on_change.clone();
            let layout_after = self.layout.clone();
            let id = item.id.clone();
            let button = Button::new(
                SharedString::from(format!("palette-{}", item.id)),
                item.label.clone(),
            )
            .style(ButtonStyle::Textured)
            .size(ControlSize::Small)
            .selected(shown)
            .tooltip(item.label.clone())
            .on_click(move |_, window, cx| {
                let mut layout_after = layout_after.clone();
                if shown {
                    hide_item(&mut layout_after, id.as_ref());
                } else {
                    layout_after.hidden.retain(|hidden| hidden != id.as_ref());
                    if !layout_after
                        .order
                        .iter()
                        .any(|ordered| ordered == id.as_ref())
                    {
                        layout_after.order.push(id.to_string());
                    }
                }
                if let Some((store, key)) = &store {
                    store.save(key, layout_after.clone(), cx);
                }
                if let Some(change) = &change {
                    change(layout_after.clone(), window, cx);
                }
            });
            let drag = ToolbarItemDrag {
                id: item.id.clone(),
                from_palette: true,
            };
            palette = palette.child(
                div()
                    .id(SharedString::from(format!("palette-chip-{}", item.id)))
                    .on_drag(drag, |_, _, _, cx| cx.new(|_| gpui::EmptyView))
                    .child(button),
            );
        }
        let remove_hot = hot.read(cx).get();
        let remove_layout = self.layout.clone();
        let remove_store = self.store.clone();
        let remove_change = self.on_change.clone();
        let remove_optional: Vec<SharedString> = self
            .items
            .iter()
            .filter(|item| item.optional)
            .map(|item| item.id.clone())
            .collect();
        let hot_for_zone = hot.clone();
        let hot_for_drop = hot.clone();
        palette = palette.child(
            div()
                .id(SharedString::from(format!("{}-remove", self.id)))
                .debug_selector(|| "toolbar-palette-remove".into())
                .flex()
                .items_center()
                .px(px(8.))
                .h(px(theme.metrics.textured_height))
                .rounded(px(4.))
                .border_1()
                .border_color(crate::convert::hsla(theme.colors.group_border))
                .bg(crate::convert::hsla(if remove_hot {
                    theme.lists.table_active.fill.from
                } else {
                    theme.colors.content
                }))
                .text_size(px(theme.type_.size_small))
                .text_color(crate::convert::hsla(theme.colors.text_secondary))
                .child("Drag an optional item here to remove")
                .on_drag_move::<ToolbarItemDrag>(move |_, _, cx| {
                    hot_for_zone.update(cx, |hot, cx| {
                        if !hot.get() {
                            hot.set(true);
                            cx.notify();
                        }
                    });
                })
                .on_drop::<ToolbarItemDrag>(move |drag, window, cx| {
                    hot_for_drop.update(cx, |hot, _| hot.set(false));
                    window.refresh();
                    if drag.from_palette || !remove_optional.iter().any(|id| id == &drag.id) {
                        return;
                    }
                    let mut layout = remove_layout.clone();
                    hide_item(&mut layout, drag.id.as_ref());
                    if let Some((store, key)) = &remove_store {
                        store.save(key, layout.clone(), cx);
                    }
                    if let Some(change) = &remove_change {
                        change(layout, window, cx);
                    }
                }),
        );
        let default_items = self.items.clone();
        let default_store = self.store.clone();
        let default_change = self.on_change.clone();
        palette = palette.child(
            Button::new(
                SharedString::from(format!("{}-default", self.id)),
                "Default Set",
            )
            .style(ButtonStyle::Textured)
            .size(ControlSize::Small)
            .on_click(move |_, window, cx| {
                let layout = ToolbarLayout::default_for(&default_items);
                if let Some((store, key)) = &default_store {
                    store.clear(key, cx);
                }
                if let Some(change) = &default_change {
                    change(layout, window, cx);
                }
            }),
        );
        palette
    }
}

fn hide_item(layout: &mut ToolbarLayout, id: &str) {
    if !layout.hidden.iter().any(|hidden| hidden == id) {
        layout.hidden.push(id.to_string());
    }
}

fn apply_drag(layout: &mut ToolbarLayout, drag: &ToolbarItemDrag, insertion: Option<usize>) {
    if !drag.from_palette {
        layout.order.retain(|id| id != drag.id.as_ref());
    }
    let position = insertion
        .unwrap_or(layout.order.len())
        .min(layout.order.len());
    if !layout.order.iter().any(|id| id == drag.id.as_ref()) {
        layout.order.insert(position, drag.id.to_string());
    }
    layout.hidden.retain(|id| id != drag.id.as_ref());
}

fn apply_layout(declared: &[ToolbarItem], layout: &ToolbarLayout) -> Vec<ToolbarItem> {
    let mut used = vec![false; declared.len()];
    let mut ordered = Vec::with_capacity(declared.len());
    for id in &layout.order {
        let found = declared
            .iter()
            .enumerate()
            .find(|(index, item)| !used[*index] && item.id.as_ref() == id.as_str())
            .map(|(index, _)| index);
        if let Some(index) = found {
            used[index] = true;
            ordered.push(declared[index].clone());
        }
    }
    for (index, item) in declared.iter().enumerate() {
        if !used[index] {
            ordered.push(item.clone());
        }
    }
    ordered.retain(|item| !item.optional || layout.shows(&item.id));
    ordered
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
            glyph,
            toggle_help,
        } => {
            let glyph = glyph.as_ref().filter(|_| show_icon).cloned();
            let descriptor = registry.read(cx).descriptors().get(command).cloned();
            let mut button = CommandButton::new(item.id.clone(), command.clone(), registry.clone())
                .external_label(item.label.clone())
                .style(match item.style {
                    ToolbarItemStyle::Rectangular => ButtonStyle::Textured,
                    ToolbarItemStyle::Capsule => ButtonStyle::Round,
                })
                .presentation(glyph.is_some(), show_label || glyph.is_none());
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
            if let Some(glyph) = glyph {
                button = button.icon(glyph.button_renderer(TemplateImageSlot::Toolbar));
            }
            button.into_any_element()
        }
        ToolbarItemKind::PictorialCommand {
            command,
            artwork,
            toggle_help,
        } => {
            let mut button = ToolbarIconButton::new(
                item.id.clone(),
                item.label.clone(),
                command.clone(),
                registry.clone(),
                artwork.clone(),
            )
            .presentation(show_icon, show_label);
            let help = toggle_help.as_ref().map_or_else(
                || {
                    registry
                        .read(cx)
                        .descriptors()
                        .get(command)
                        .and_then(|descriptor| descriptor.tooltip.clone())
                },
                |(on, off)| {
                    Some(
                        if registry
                            .read(cx)
                            .descriptors()
                            .get(command)
                            .is_some_and(|descriptor| descriptor.checked)
                        {
                            on.to_string()
                        } else {
                            off.to_string()
                        },
                    )
                },
            );
            if let Some(help) = help {
                button = button.tooltip(help);
            }
            if let Some(chain) = chain {
                button = button.chain(chain.clone());
            }
            button.into_any_element()
        }
        ToolbarItemKind::Segmented {
            segments,
            selected,
            on_select,
        } => {
            let show_segment_icons =
                show_icon && segments.iter().all(|segment| segment.glyph.is_some());
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
                    if let Some(glyph) = segment
                        .glyph
                        .as_ref()
                        .filter(|_| show_segment_icons)
                        .cloned()
                    {
                        value = value.glyph(glyph);
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
        ToolbarItemKind::PopDown { menu, glyph } => render_pop_down(
            item,
            menu.clone(),
            glyph.as_ref().filter(|_| show_icon).cloned(),
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
    glyph: Option<ToolbarGlyph>,
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
    if let Some(glyph) = glyph {
        button = button.icon(glyph.button_renderer(TemplateImageSlot::Toolbar));
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
            ToolbarItemKind::Command { command, .. }
            | ToolbarItemKind::PictorialCommand { command, .. } => {
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
        ToolbarItemKind::Command { command, .. }
        | ToolbarItemKind::PictorialCommand { command, .. } => {
            vec![MenuEntry::Command(command.clone())]
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn items() -> Vec<ToolbarItem> {
        vec![
            ToolbarItem::command("keep", "Keep", "keep"),
            ToolbarItem::command("opt-a", "A", "opt-a").optional(true),
            ToolbarItem::command("opt-b", "B", "opt-b").optional(true),
        ]
    }

    #[test]
    fn a_layout_reorders_and_hides_only_optional_items() {
        let mut layout = ToolbarLayout::default_for(&items());
        layout.order = vec!["opt-b".into(), "keep".into(), "opt-a".into()];
        layout.hidden = vec!["opt-a".into(), "keep".into()];
        let applied = apply_layout(&items(), &layout);
        assert_eq!(
            applied
                .iter()
                .map(|item| item.id.to_string())
                .collect::<Vec<_>>(),
            ["opt-b", "keep"],
            "order applies; a hidden required item stays"
        );
    }

    #[test]
    fn a_palette_drag_adds_an_item_and_a_toolbar_drag_moves_it() {
        let mut layout = ToolbarLayout {
            order: vec!["keep".into(), "opt-b".into()],
            hidden: vec!["opt-a".into()],
            mode: None,
        };
        apply_drag(
            &mut layout,
            &ToolbarItemDrag {
                id: "opt-a".into(),
                from_palette: true,
            },
            Some(1),
        );
        assert_eq!(layout.order, ["keep", "opt-a", "opt-b"]);
        assert!(layout.hidden.is_empty());

        apply_drag(
            &mut layout,
            &ToolbarItemDrag {
                id: "opt-b".into(),
                from_palette: false,
            },
            Some(0),
        );
        assert_eq!(layout.order, ["opt-b", "keep", "opt-a"]);
    }

    #[test]
    fn the_default_set_restores_declaration_order() {
        let layout = ToolbarLayout::default_for(&items());
        assert_eq!(layout.order, ["keep", "opt-a", "opt-b"]);
        assert!(layout.hidden.is_empty());
        assert_eq!(layout.mode, None);
    }

    #[test]
    fn hiding_an_item_is_idempotent_for_drag_out_removal() {
        let mut layout = ToolbarLayout::default_for(&items());
        hide_item(&mut layout, "opt-a");
        hide_item(&mut layout, "opt-a");
        assert_eq!(layout.hidden, ["opt-a"]);
    }
}
