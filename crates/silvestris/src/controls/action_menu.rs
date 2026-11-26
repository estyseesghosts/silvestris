//! Action menu and command pop-down (HIG pp. 288-375; #46).
//!
//! Both are command menus, not value pop-ups: their entries are
//! [`CommandRegistry`] commands and choosing one invokes it through the
//! registry/responder chain. A value pop-up remains [`PopUpButton`], which
//! lists mutually exclusive nouns.
//!
//! - `ActionMenu` is the toolbar Action affordance: the standard
//!   [`TemplateImage::Action`] artwork plus a small down arrow. Its menu
//!   prints no keyboard shortcuts (HIG Action-menu rule).
//! - `CommandPopDown` is the labelled form for a command group; an optional
//!   template image precedes the label.
//! - Both open through [`PopupMenu::open_pop_down`] on the shared transient
//!   layer, so opening one dismisses another and focus returns to the button.
//!
//! The arrow reuses the pop-up cap's triangle geometry and ink; its inset is
//! an estimate because no separate Action-menu crop exists. See
//! `docs/exceptions.md`.
use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    canvas, deferred, div, point, prelude::*, px, App, Bounds, ElementId, Entity, IntoElement,
    Pixels, RenderOnce, SharedString, Window,
};
use silvestris_core::command::MenuModel;

use crate::command::CommandRegistry;
use crate::controls::button::{Button, ButtonStyle};
use crate::controls::ControlSize;
use crate::icons::{TemplateImage, TemplateImageSlot};
use crate::menu::PopupMenu;
use crate::responder::ResponderChain;
use crate::transient::TransientLayer;
use crate::ActiveTheme;

#[derive(Clone)]
struct Config {
    id: ElementId,
    label: SharedString,
    menu: MenuModel,
    registry: Entity<CommandRegistry>,
    chain: Option<Entity<ResponderChain>>,
    layer: Option<Entity<TransientLayer>>,
    size: ControlSize,
    style: ButtonStyle,
    disabled: bool,
    show_label: bool,
    image: Option<TemplateImage>,
}

fn ensure_menu(
    id: &ElementId,
    config: &Config,
    window: &mut Window,
    cx: &mut App,
) -> Entity<PopupMenu> {
    let slot = window.use_keyed_state(id.clone(), cx, |_, _| None::<Entity<PopupMenu>>);
    let existing = slot.read(cx).clone();
    let menu = existing.unwrap_or_else(|| {
        let menu =
            cx.new(|cx| PopupMenu::new(config.menu.clone(), config.registry.clone(), window, cx));
        menu.update(cx, |menu, cx| menu.set_show_shortcuts(false, cx));
        if let Some(chain) = &config.chain {
            menu.update(cx, |menu, cx| menu.set_chain(chain.clone(), cx));
        }
        if let Some(layer) = &config.layer {
            menu.update(cx, |menu, cx| menu.set_layer(layer.clone(), cx));
        }
        slot.update(cx, |slot, _| *slot = Some(menu.clone()));
        menu
    });
    menu.update(cx, |menu, cx| {
        menu.set_show_shortcuts(false, cx);
        menu.set_model(config.menu.clone(), window, cx);
    });
    menu
}

fn render(config: Config, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let menu = ensure_menu(&config.id, &config, window, cx);
    let theme = cx.theme().clone();
    let bounds = Rc::new(Cell::new(None::<Bounds<Pixels>>));
    let click_bounds = bounds.clone();
    let click_menu = menu.clone();
    let mut button = Button::new(config.id.clone(), config.label.clone())
        .style(config.style)
        .size(config.size)
        .disabled(config.disabled)
        .tooltip(config.label.clone())
        .on_click(move |_, window, cx| {
            if let Some(bounds) = click_bounds.get() {
                click_menu.update(cx, |menu, cx| {
                    menu.open_pop_down(bounds.bottom_left(), window, cx);
                });
            }
        });
    if let Some(image) = config.image {
        button = button.icon(image.button_icon_renderer(TemplateImageSlot::Toolbar));
    }
    button = button.icon_and_label(config.show_label && config.image.is_some());
    let arrow = if config.disabled {
        theme
            .controls
            .popup
            .arrow
            .mix(theme.colors.text_disabled, 0.7)
    } else {
        theme.controls.popup.arrow
    };
    div()
        .relative()
        .flex_none()
        .child(button)
        .child(
            canvas(|_, _, _| (), move |rect, _, _, _| bounds.set(Some(rect)))
                .absolute()
                .inset_0(),
        )
        .child(
            canvas(
                |_, _, _| (),
                move |rect, _, window, _| {
                    // Pop-up cap triangle geometry, inset 5 px from the
                    // button's trailing/bottom edge (see docs/exceptions.md).
                    let center = point(rect.right() - px(5.), rect.bottom() - px(5.));
                    crate::paint::polygon(
                        window,
                        &[
                            point(center.x, center.y + px(2.)),
                            point(center.x - px(3.5), center.y - px(2.)),
                            point(center.x + px(3.5), center.y - px(2.)),
                        ],
                        arrow,
                    );
                },
            )
            .absolute()
            .inset_0(),
        )
        .child(deferred(menu).priority(1))
}

/// The toolbar Action affordance: Action artwork plus a down arrow, opening a
/// menu of commands. Ten-point-six Action menus do not print shortcuts.
#[derive(IntoElement)]
pub struct ActionMenu {
    config: Config,
}

impl ActionMenu {
    /// `label` is the external name ("Action", "More"). The visible button is
    /// the Action artwork.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        menu: MenuModel,
        registry: Entity<CommandRegistry>,
    ) -> Self {
        Self {
            config: Config {
                id: id.into(),
                label: label.into(),
                menu,
                registry,
                chain: None,
                layer: None,
                size: ControlSize::Regular,
                style: ButtonStyle::Textured,
                disabled: false,
                show_label: false,
                image: Some(TemplateImage::Action),
            },
        }
    }

    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.config.chain = Some(chain);
        self
    }

    pub fn layer(mut self, layer: Entity<TransientLayer>) -> Self {
        self.config.layer = Some(layer);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.config.size = size;
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.config.style = style;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.config.disabled = disabled;
        self
    }
}

impl RenderOnce for ActionMenu {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        render(self.config, window, cx)
    }
}

/// A labelled command pop-down: a button and down arrow whose menu contains
/// commands, never mutually exclusive values.
#[derive(IntoElement)]
pub struct CommandPopDown {
    config: Config,
}

impl CommandPopDown {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        menu: MenuModel,
        registry: Entity<CommandRegistry>,
    ) -> Self {
        Self {
            config: Config {
                id: id.into(),
                label: label.into(),
                menu,
                registry,
                chain: None,
                layer: None,
                size: ControlSize::Regular,
                style: ButtonStyle::Textured,
                disabled: false,
                show_label: true,
                image: None,
            },
        }
    }

    /// Put a monochrome template image before the label.
    pub fn image(mut self, image: TemplateImage) -> Self {
        self.config.image = Some(image);
        self
    }

    pub fn chain(mut self, chain: Entity<ResponderChain>) -> Self {
        self.config.chain = Some(chain);
        self
    }

    pub fn layer(mut self, layer: Entity<TransientLayer>) -> Self {
        self.config.layer = Some(layer);
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.config.size = size;
        self
    }

    pub fn style(mut self, style: ButtonStyle) -> Self {
        self.config.style = style;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.config.disabled = disabled;
        self
    }
}

impl RenderOnce for CommandPopDown {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        render(self.config, window, cx)
    }
}
