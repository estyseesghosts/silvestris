//! Document-modal confirmation sheet (HIG pp. 233–239).
use gpui::{div, prelude::*, px, App, FocusHandle, MouseButton, SharedString, Window};
use std::rc::Rc;

use crate::{command::InvokeCommand, convert::hsla, ActiveTheme};

#[derive(Clone)]
pub struct ConfirmationFocus {
    cancel: FocusHandle,
    accept: FocusHandle,
}

impl ConfirmationFocus {
    pub fn new(cx: &App) -> Self {
        Self {
            cancel: cx.focus_handle(),
            accept: cx.focus_handle(),
        }
    }

    pub fn focus_cancel(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.cancel, cx);
    }

    /// The sheet's focus stops in Tab order. A modal session (#21) uses them
    /// to keep Tab inside the sheet.
    #[must_use]
    pub fn handles(&self) -> [FocusHandle; 2] {
        [self.cancel.clone(), self.accept.clone()]
    }
}

type Respond = Rc<dyn Fn(bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ConfirmationSheet {
    title: SharedString,
    message: SharedString,
    accept: SharedString,
    focus: ConfirmationFocus,
    respond: Respond,
}

impl ConfirmationSheet {
    pub fn new(
        title: impl Into<SharedString>,
        message: impl Into<SharedString>,
        accept: impl Into<SharedString>,
        focus: ConfirmationFocus,
        respond: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            accept: accept.into(),
            focus,
            respond: Rc::new(respond),
        }
    }
}

impl RenderOnce for ConfirmationSheet {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let keys = self.respond.clone();
        let cancel = self.respond.clone();
        let accept = self.respond;
        let focus = self.focus.clone();
        div()
            .id("silvestris-sheet")
            .role(gpui::Role::AlertDialog)
            .aria_label(self.title.clone())
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(58.0))
            .bg(hsla(theme.colors.scrim))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_action(|_: &InvokeCommand, _, cx| cx.stop_propagation())
            .on_key_down(move |event, window, cx| {
                cx.stop_propagation();
                match event.keystroke.key.as_str() {
                    "escape" => keys(false, window, cx),
                    "tab" if focus.cancel.is_focused(window) => window.focus(&focus.accept, cx),
                    "tab" => focus.focus_cancel(window, cx),
                    _ => {}
                }
            })
            .child(
                div()
                    .w(px(380.0))
                    .max_w_full()
                    .p(px(18.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(hsla(theme.colors.group_border))
                    .bg(hsla(theme.colors.window))
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(self.title),
                    )
                    .child(
                        div()
                            .text_size(px(theme.type_.size_small))
                            .child(self.message),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap(px(8.0))
                            .child(
                                super::Button::new("sheet-cancel", "Cancel")
                                    .focus_handle(self.focus.cancel)
                                    .default_button(true)
                                    .on_click(move |_, window, cx| cancel(false, window, cx)),
                            )
                            .child(
                                super::Button::new("sheet-accept", self.accept)
                                    .focus_handle(self.focus.accept)
                                    .destructive(true)
                                    .on_click(move |_, window, cx| accept(true, window, cx)),
                            ),
                    ),
            )
    }
}
