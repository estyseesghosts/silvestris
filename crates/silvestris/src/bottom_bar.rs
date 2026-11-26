//! Standard bottom-bar composition (#75).
//!
//! A bottom bar is a window-frame area below the window body whose controls
//! directly affect the contents or organization of that body (HIG p. 212). It is
//! not a second toolbar: it accepts regular or small rectangular-style controls,
//! status values, and the square gradient controls that adjoin a source list
//! (HIG pp. 212-214, 257-258, 273-276; `specs/toolbar-item.md` §1-2). Capsule
//! controls and application status computation stay out of the framework.
//!
//! The bar's material, separator and active/inactive appearance are painted by
//! [`crate::chrome::WindowFrame`] through `theme.chrome.bottom_*`, so a bar can
//! be mounted with [`crate::chrome::WindowFrame::bottom_bar_component`].

use crate::{
    command::CommandButton,
    command::CommandRegistry,
    controls::button::{ButtonStyle, ControlSize},
    icons::TemplateImage,
    responder::ResponderChain,
    theme::ActiveTheme,
};
use gpui::{
    div, prelude::*, px, AnyElement, App, Entity, IntoElement, RenderOnce, SharedString, Window,
};
use silvestris_core::command::CommandId;
use std::rc::Rc;

type StatusAction = Rc<dyn Fn(&mut Window, &mut App)>;

/// Standard bar families (HIG pp. 365-366).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BottomBarSize {
    /// Regular controls: 4 px above a regular control and 6 px below it.
    #[default]
    Regular,
    /// Small controls: 2 px above a small control and 2 px below it.
    Small,
}

impl BottomBarSize {
    /// Control size used by this bar family.
    #[must_use]
    pub fn control_size(self) -> ControlSize {
        match self {
            Self::Regular => ControlSize::Regular,
            Self::Small => ControlSize::Small,
        }
    }

    /// Space from the dark separating line to the tops of the controls
    /// (HIG p. 365: 4 px regular, 2 px small).
    #[must_use]
    pub fn top_inset(self) -> f32 {
        match self {
            Self::Regular => 4.0,
            Self::Small => 2.0,
        }
    }
}

/// One status value in a bottom bar.
///
/// The consumer owns the value and omits entries it does not want to show, so
/// zero-value hiding stays application policy. An activation callback is
/// optional because a status may open a sheet or other detail surface when
/// activated.
#[derive(Clone)]
pub struct BottomBarStatus {
    id: SharedString,
    text: SharedString,
    help: Option<SharedString>,
    on_activate: Option<StatusAction>,
}

impl BottomBarStatus {
    /// Create a status value with a stable id and displayed text.
    pub fn new(id: impl Into<SharedString>, text: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            help: None,
            on_activate: None,
        }
    }

    /// Help tag for the value, required when it is clickable but its text is
    /// not a complete description.
    pub fn help(mut self, help: impl Into<SharedString>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Make the status value activatable. The bar remains a status host; the
    /// callback owns whatever surface it opens.
    pub fn on_activate(mut self, activate: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_activate = Some(Rc::new(activate));
        self
    }

    /// Stable identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }
}

/// The presentation of a command-backed bottom-bar control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BottomBarControlStyle {
    /// Small rectangular-style toolbar control (HIG pp. 257-258).
    #[default]
    Rectangular,
    /// Square gradient button adjoining a source list (HIG pp. 273-276).
    Gradient,
}

#[derive(Clone)]
enum BottomBarItemKind {
    Status(BottomBarStatus),
    Command {
        command: CommandId,
        image: Option<TemplateImage>,
        style: BottomBarControlStyle,
    },
    Group(Vec<BottomBarItem>),
    FixedSpace(f32),
    FlexibleSpace,
}

/// A typed bottom-bar element. Statuses truncate before controls shrink;
/// controls keep their standard size and are never stretched vertically.
#[derive(Clone)]
pub struct BottomBarItem {
    id: SharedString,
    label: SharedString,
    kind: BottomBarItemKind,
}

impl BottomBarItem {
    /// A status value. Status items are the only flexible text in a bar. The
    /// status's own stable id is the element identity.
    pub fn status(status: BottomBarStatus) -> Self {
        let id = status.id.clone();
        let label = status.text.clone();
        Self {
            id,
            label,
            kind: BottomBarItemKind::Status(status),
        }
    }

    /// A command-backed rectangular control.
    pub fn command(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        command: impl Into<CommandId>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: BottomBarItemKind::Command {
                command: command.into(),
                image: None,
                style: BottomBarControlStyle::default(),
            },
        }
    }

    /// Set the template image shown in the control.
    pub fn image(mut self, image: TemplateImage) -> Self {
        if let BottomBarItemKind::Command { image: slot, .. } = &mut self.kind {
            *slot = Some(image);
        }
        self
    }

    /// Select rectangular (default) or square gradient presentation.
    pub fn style(mut self, style: BottomBarControlStyle) -> Self {
        if let BottomBarItemKind::Command { style: slot, .. } = &mut self.kind {
            *slot = style;
        }
        self
    }

    /// Keep related controls together.
    pub fn group(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        items: Vec<Self>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind: BottomBarItemKind::Group(items),
        }
    }

    /// A fixed spacer, in logical pixels.
    pub fn fixed_space(id: impl Into<SharedString>, width: f32) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            kind: BottomBarItemKind::FixedSpace(width.max(0.0)),
        }
    }

    /// A spacer that absorbs remaining width, separating leading and trailing
    /// groups.
    pub fn flexible_space(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: SharedString::default(),
            kind: BottomBarItemKind::FlexibleSpace,
        }
    }

    /// Stable identifier.
    #[must_use]
    pub fn id(&self) -> &SharedString {
        &self.id
    }

    /// External descriptive label.
    #[must_use]
    pub fn label(&self) -> &SharedString {
        &self.label
    }
}

/// A standard bottom bar. The containing view must observe the command registry
/// and responder chain so command state repaints.
#[derive(IntoElement)]
pub struct BottomBar {
    id: gpui::ElementId,
    label: SharedString,
    registry: Option<Entity<CommandRegistry>>,
    chain: Option<Entity<ResponderChain>>,
    items: Vec<BottomBarItem>,
    size: BottomBarSize,
}

impl BottomBar {
    /// Create a bar. Command items require [`Self::registry`].
    pub fn new(
        id: impl Into<gpui::ElementId>,
        label: impl Into<SharedString>,
        items: Vec<BottomBarItem>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            registry: None,
            chain: None,
            items,
            size: BottomBarSize::default(),
        }
    }

    /// Supply the registry used to resolve command state and activation.
    pub fn registry(mut self, registry: Entity<CommandRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Route command items through the window's responder chain.
    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.chain = Some(chain);
        self
    }

    /// Select the regular or small bar family.
    pub fn size(mut self, size: BottomBarSize) -> Self {
        self.size = size;
        self
    }

    /// Height of this bar family in the active theme.
    #[must_use]
    pub fn height(&self, cx: &App) -> f32 {
        let metrics = &cx.theme().metrics;
        match self.size {
            BottomBarSize::Regular => metrics.bottom_bar_height,
            BottomBarSize::Small => metrics.bottom_bar_small_height,
        }
    }
}

impl RenderOnce for BottomBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme().clone();
        let height = self.height(cx);
        let size = self.size;
        let registry = self.registry;
        let chain = self.chain;
        let debug_id = self.id.to_string();
        let mut row = div()
            .id(self.id)
            .debug_selector(move || debug_id.clone())
            .role(gpui::Role::Group)
            .aria_label(self.label)
            .flex()
            .items_start()
            .gap(px(8.0))
            .h(px(height))
            .w_full()
            .min_w_0()
            // HIG p. 365: 8 px from the bar's left edge and between controls;
            // p. 366 gives the same 8 px for small bars.
            .pl(px(8.0))
            // Keep the resize grip's hit region clear.
            .pr(px(theme.grip.hit))
            .pt(px(size.top_inset()));
        for item in &self.items {
            row = row.child(render_item(
                item,
                size,
                registry.as_ref(),
                chain.as_ref(),
                cx,
            ));
        }
        row
    }
}

fn render_item(
    item: &BottomBarItem,
    size: BottomBarSize,
    registry: Option<&Entity<CommandRegistry>>,
    chain: Option<&Entity<ResponderChain>>,
    cx: &App,
) -> AnyElement {
    let control_size = size.control_size();
    let control_height = match size {
        BottomBarSize::Regular => cx.theme().metrics.textured_height,
        BottomBarSize::Small => cx.theme().metrics.push_small_height,
    };
    match &item.kind {
        BottomBarItemKind::Status(status) => {
            let text = div()
                .h(px(control_height))
                .flex()
                .items_center()
                .min_w_0()
                .truncate()
                .text_size(px(cx.theme().type_.size_small))
                .text_color(crate::convert::hsla(cx.theme().colors.text_secondary))
                .child(status.text.clone());
            let mut element = div()
                .id(item.id.clone())
                .debug_selector({
                    let id = item.id.to_string();
                    move || id.clone()
                })
                .flex()
                .items_center()
                .flex_shrink_1()
                .min_w_0()
                .child(text);
            if let Some(help) = &status.help {
                element = crate::controls::tooltip::attach(element, help.clone());
            }
            if let Some(activate) = &status.on_activate {
                let activate = activate.clone();
                element = element
                    .role(gpui::Role::Button)
                    .aria_label(status.text.clone())
                    .cursor(gpui::CursorStyle::PointingHand)
                    .on_click(move |_, window, cx| activate(window, cx));
            }
            element.into_any_element()
        }
        BottomBarItemKind::Command {
            command,
            image,
            style,
        } => {
            let registry = registry.expect("bottom-bar command items require a registry");
            let mut button = CommandButton::new(item.id.clone(), command.clone(), registry.clone())
                .external_label(item.label.clone())
                .style(match style {
                    BottomBarControlStyle::Rectangular => ButtonStyle::Textured,
                    BottomBarControlStyle::Gradient => ButtonStyle::Gradient,
                })
                .size(control_size)
                .presentation(
                    image.is_some(),
                    image.is_none() && *style == BottomBarControlStyle::Rectangular,
                );
            if let Some(image) = image {
                button = button.template_image(*image);
            }
            if let Some(chain) = chain {
                button = button.chain(chain.clone());
            }
            div()
                .debug_selector({
                    let id = item.id.to_string();
                    move || id.clone()
                })
                .flex_none()
                .flex_shrink_0()
                .child(button)
                .into_any_element()
        }
        BottomBarItemKind::Group(items) => {
            let mut group = div()
                .debug_selector({
                    let id = item.id.to_string();
                    move || id.clone()
                })
                .flex()
                .items_start()
                .gap(px(8.0))
                .flex_none()
                .min_w_0();
            for item in items {
                group = group.child(render_item(item, size, registry, chain, cx));
            }
            group.into_any_element()
        }
        BottomBarItemKind::FixedSpace(width) => div()
            .flex_none()
            .flex_shrink_0()
            .w(px(*width))
            .into_any_element(),
        BottomBarItemKind::FlexibleSpace => {
            div().flex_1().flex_shrink_1().min_w_0().into_any_element()
        }
    }
}
