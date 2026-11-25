//! Alert (#23): icon, title, message, informative text, up to three buttons
//! and a suppression checkbox (HIG pp. 233-239).
//!
//! An alert is a view, not a window. [`Alert::begin_sheet`] wraps it in a
//! document sheet; [`Alert::run_modal`] opens an application-modal window.
//! The Save panel's replace-existing alert uses the latter because HIG p. 236
//! allows only one sheet per window (the collision alert is application-modal,
//! p. 251), and cancelling it returns to the Save panel.
//!
//! Return chooses the default button; Escape chooses the cancel button.
//!
//! The built-in replace preset is the Save panel's overwrite question:
//! "“X” already exists. Do you want to replace it?", with Cancel and a default
//! Replace.

use std::cell::Cell;
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, AnyWindowHandle, App, Context, Entity, FocusHandle, KeyDownEvent, Render,
    SharedString, Window,
};

use super::{Button, CheckState, Checkbox, ControlSize};
use crate::chrome::WindowFrame;
use crate::convert::hsla;
use crate::icons::Icon;
use crate::modal::{self, ModalResult, ModalScope, ModalSession};
use crate::window_role::{open_role, WindowRole};
use crate::{sheet, ActiveTheme};

struct AlertWindow {
    alert: Entity<Alert>,
    title: SharedString,
}

impl Render for AlertWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        WindowFrame::new(self.title.clone())
            .role(WindowRole::Dialog)
            .fixed_size()
            .child(self.alert.clone())
    }
}

/// The job a button does in an alert.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AlertRole {
    #[default]
    Normal,
    /// Return activates this button.
    Default,
    /// Escape activates this button.
    Cancel,
    Destructive,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertButton {
    pub id: SharedString,
    pub label: SharedString,
    pub role: AlertRole,
}

impl AlertButton {
    pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role: AlertRole::Normal,
        }
    }

    pub fn role(mut self, role: AlertRole) -> Self {
        self.role = role;
        self
    }
}

/// What the user chose. `choice` is the pressed button's id; `cancelled` is
/// true for Escape or a Cancel-role button.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlertResult {
    pub choice: Option<SharedString>,
    pub suppressed: bool,
    pub cancelled: bool,
}

impl AlertResult {
    fn cancelled(suppressed: bool) -> Self {
        Self {
            choice: None,
            suppressed,
            cancelled: true,
        }
    }
}

type ResultFn = Rc<dyn Fn(&AlertResult, &mut Window, &mut App)>;

pub struct Alert {
    icon: Option<Icon>,
    title: SharedString,
    message: Option<SharedString>,
    informative: Option<SharedString>,
    buttons: Vec<AlertButton>,
    suppression: Option<SharedString>,
    suppressed: Rc<Cell<bool>>,
    button_focus: Vec<FocusHandle>,
    focus_pair: Option<(FocusHandle, FocusHandle)>,
    session: Option<Entity<ModalSession>>,
    app_window: Option<AnyWindowHandle>,
    window_title: Option<SharedString>,
    on_result: Option<ResultFn>,
}

impl Alert {
    pub fn new(title: impl Into<SharedString>) -> Self {
        Self {
            icon: None,
            title: title.into(),
            message: None,
            informative: None,
            buttons: Vec::new(),
            suppression: None,
            suppressed: Rc::new(Cell::new(false)),
            button_focus: Vec::new(),
            focus_pair: None,
            session: None,
            app_window: None,
            window_title: None,
            on_result: None,
        }
    }

    /// The Save panel's overwrite question. Replace is the default; Cancel is
    /// the Escape button.
    pub fn replace_existing(name: impl AsRef<str>) -> Self {
        Self::new(format!("\u{201C}{}\u{201D} already exists.", name.as_ref()))
            .message("Do you want to replace it?")
            .button(AlertButton::new("alert-cancel", "Cancel").role(AlertRole::Cancel))
            .button(AlertButton::new("alert-replace", "Replace").role(AlertRole::Default))
    }

    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn message(mut self, message: impl Into<SharedString>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn informative(mut self, text: impl Into<SharedString>) -> Self {
        self.informative = Some(text.into());
        self
    }

    pub fn button(mut self, button: AlertButton) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn suppressible(mut self, label: impl Into<SharedString>) -> Self {
        self.suppression = Some(label.into());
        self
    }

    pub fn on_result(mut self, f: impl Fn(&AlertResult, &mut Window, &mut App) + 'static) -> Self {
        self.on_result = Some(Rc::new(f));
        self
    }

    /// Title-bar text for an application-modal alert window.
    pub fn window_title(mut self, title: impl Into<SharedString>) -> Self {
        self.window_title = Some(title.into());
        self
    }

    /// Use the caller's focus handles for the default and cancel buttons, so
    /// an existing `ConfirmationFocus` keeps working.
    pub fn focus_pair(mut self, default: FocusHandle, cancel: FocusHandle) -> Self {
        self.focus_pair = Some((default, cancel));
        self
    }

    pub fn title(&self) -> &SharedString {
        &self.title
    }

    pub fn buttons(&self) -> &[AlertButton] {
        &self.buttons
    }

    fn ensure_focus(&mut self, cx: &App) {
        while self.button_focus.len() < self.buttons.len() {
            self.button_focus.push(cx.focus_handle());
        }
        if let Some((default, cancel)) = &self.focus_pair {
            for (index, button) in self.buttons.iter().enumerate() {
                match button.role {
                    AlertRole::Default => self.button_focus[index] = default.clone(),
                    AlertRole::Cancel => self.button_focus[index] = cancel.clone(),
                    _ => {}
                }
            }
        }
    }

    fn focus_stops(&self) -> Vec<FocusHandle> {
        self.button_focus.clone()
    }

    fn default_button(&self) -> Option<&AlertButton> {
        self.buttons
            .iter()
            .find(|button| button.role == AlertRole::Default)
            .or_else(|| self.buttons.last())
    }

    fn cancel_button(&self) -> Option<&AlertButton> {
        self.buttons
            .iter()
            .find(|button| button.role == AlertRole::Cancel)
    }

    fn choose(&mut self, id: &str, window: &mut Window, cx: &mut App) {
        let Some(button) = self.buttons.iter().find(|button| button.id == id) else {
            return;
        };
        let cancelled = button.role == AlertRole::Cancel;
        let result = AlertResult {
            choice: Some(button.id.clone()),
            suppressed: self.suppressed.get(),
            cancelled,
        };
        self.finish(result, window, cx);
    }

    fn cancel(&mut self, window: &mut Window, cx: &mut App) {
        let suppressed = self.suppressed.get();
        let result = match self.cancel_button() {
            Some(button) => AlertResult {
                choice: Some(button.id.clone()),
                suppressed,
                cancelled: true,
            },
            None => AlertResult::cancelled(suppressed),
        };
        self.finish(result, window, cx);
    }

    fn accept_default(&mut self, window: &mut Window, cx: &mut App) {
        match self.default_button().map(|button| button.id.clone()) {
            Some(id) => self.choose(&id, window, cx),
            None => self.finish(AlertResult::cancelled(self.suppressed.get()), window, cx),
        }
    }

    fn finish(&mut self, result: AlertResult, window: &mut Window, cx: &mut App) {
        if let Some(session) = self.session.take() {
            let outcome = if result.cancelled {
                ModalResult::Rejected
            } else {
                ModalResult::Accepted
            };
            if !sheet::dismiss(session.clone(), outcome, window, cx) {
                modal::finish(session, outcome, window, cx);
            }
        }
        if let Some(f) = self.on_result.take() {
            f(&result, window, cx);
        }
        if self.app_window.is_some() {
            window.remove_window();
        }
    }

    /// Present this alert as an application-modal window (HIG p. 251: the
    /// replace-existing collision alert is application-modal, not a sheet over
    /// the Save sheet). The returned entity stays alive as the window content.
    pub fn run_modal(self, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let mut alert = self;
        alert.ensure_focus(cx);
        let stops = alert.focus_stops();
        let session =
            cx.new(|_| ModalSession::new(ModalScope::Application).with_focus_stops(stops));
        alert.session = Some(session.clone());
        let title = alert.window_title.clone().unwrap_or_else(|| "Alert".into());
        let alert = cx.new(|_| alert);
        let alert_for_window = alert.clone();
        let panel_title = title.clone();
        let bounds =
            crate::window_role::centered_on(window.bounds(), gpui::size(px(420.), px(190.)));
        match open_role(WindowRole::Dialog, title, bounds, cx, move |window, cx| {
            let handle = window.window_handle();
            alert_for_window.update(cx, |alert, cx| {
                alert.app_window = Some(handle);
                cx.notify();
            });
            modal::present(session, window, cx);
            window.on_window_should_close(cx, |window, cx| {
                modal::cancel_owned(window.window_handle(), cx);
                true
            });
            cx.new(|_| AlertWindow {
                alert: alert_for_window,
                title: panel_title,
            })
        }) {
            Ok(_) => {}
            Err(_) => {
                let handle = alert.read(cx).session.clone();
                if let Some(session) = handle {
                    modal::finish(session, ModalResult::Cancelled, window, cx);
                }
            }
        }
        alert
    }

    /// Present this alert as an application-modal sheet over `owner`. Used for
    /// confirmations that belong to a document (never over another sheet).
    pub fn begin_sheet(self, owner: AnyWindowHandle, cx: &mut App) -> Entity<Self> {
        let mut alert = self;
        alert.ensure_focus(cx);
        let stops = alert.focus_stops();
        let session =
            cx.new(|_| ModalSession::new(ModalScope::Application).with_focus_stops(stops));
        alert.session = Some(session.clone());
        let title = alert.title.clone();
        let alert = cx.new(|_| alert);
        let content_alert = alert.clone();
        let sheet = sheet::Sheet::new(
            title,
            420.0,
            session,
            move |_, _| content_alert.clone().into_any_element(),
            cx,
        );
        sheet::present(owner, sheet, cx);
        alert
    }
}

impl Render for Alert {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_focus(cx);
        let theme = cx.theme().clone();
        let this = cx.entity();
        let suppressed = self.suppressed.clone();
        let mut body = div()
            .id("silvestris-alert")
            .debug_selector(|| "silvestris-alert".into())
            .role(gpui::Role::AlertDialog)
            .aria_label(self.title.clone())
            .flex()
            .gap(px(12.))
            .p(px(18.))
            .capture_key_down(cx.listener(|alert, event: &KeyDownEvent, window, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => {
                        cx.stop_propagation();
                        alert.cancel(window, cx);
                    }
                    "enter" => {
                        cx.stop_propagation();
                        alert.accept_default(window, cx);
                    }
                    _ => {}
                }
            }));
        if let Some(icon) = self.icon {
            body = body.child(
                div()
                    .flex_none()
                    .mt(px(2.))
                    .child(icon.file_element(32.0, crate::icons::FileIconState::normal())),
            );
        }
        let mut column = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.title.clone()),
            );
        if let Some(message) = &self.message {
            column = column.child(div().child(message.clone()));
        }
        if let Some(informative) = &self.informative {
            column = column.child(
                div()
                    .text_size(px(theme.type_.size_small))
                    .text_color(hsla(theme.colors.text_secondary))
                    .child(informative.clone()),
            );
        }
        if let Some(label) = &self.suppression {
            let suppressed = suppressed.clone();
            column = column.child(
                Checkbox::new("alert-suppress", label.clone())
                    .size(ControlSize::Small)
                    .checked(suppressed.get())
                    .on_change(move |state, window, _| {
                        suppressed.set(*state == CheckState::On);
                        window.refresh();
                    }),
            );
        }
        let mut buttons = div().flex().justify_end().gap(px(8.)).pt(px(4.));
        for (index, button) in self.buttons.iter().enumerate().rev() {
            let id = button.id.clone();
            let focus = self.button_focus.get(index).cloned();
            let mut element = Button::new(button.id.clone(), button.label.clone())
                .size(ControlSize::Regular)
                .default_button(button.role == AlertRole::Default)
                .cancel_button(button.role == AlertRole::Cancel)
                .destructive(button.role == AlertRole::Destructive)
                .on_click({
                    let this = this.clone();
                    move |_, window, cx| {
                        this.update(cx, |alert, cx| {
                            alert.choose(&id, window, cx);
                            cx.notify();
                        });
                    }
                });
            if let Some(focus) = focus {
                element = element.focus_handle(focus);
            }
            buttons = buttons.child(element);
        }
        column = column.child(buttons);
        body = body.child(column);
        div().w_full().child(body).into_any_element()
    }
}
