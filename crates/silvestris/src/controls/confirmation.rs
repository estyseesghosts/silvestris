//! Document-modal confirmation sheet (HIG pp. 233-239).
//!
//! The public API is unchanged; the body is now [`Alert`], so confirmations
//! share one alert layout, button policy and Escape/Return mapping with the
//! Save panel's replace-existing alert.

use gpui::{App, AppContext, FocusHandle, SharedString, Window};
use std::rc::Rc;

use super::alert::{Alert, AlertButton, AlertRole};

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

#[derive(gpui::IntoElement)]
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

impl gpui::RenderOnce for ConfirmationSheet {
    fn render(self, window: &mut Window, cx: &mut App) -> impl gpui::IntoElement {
        let [cancel_focus, _accept_focus] = self.focus.handles();
        let respond = self.respond;
        let accept_label = self.accept;
        let title = self.title;
        let message = self.message;
        let alert = window
            .use_keyed_state(
                gpui::ElementId::Name("confirmation-sheet-alert".into()),
                cx,
                move |_, cx| {
                    cx.new(|_| {
                        Alert::new(title)
                            .message(message)
                            // The old sheet made Cancel the default button; keep
                            // that policy for callers that rely on Return choosing
                            // Cancel.
                            .button(
                                AlertButton::new("sheet-cancel", "Cancel").role(AlertRole::Default),
                            )
                            .button(
                                AlertButton::new("sheet-accept", accept_label)
                                    .role(AlertRole::Destructive),
                            )
                            .focus_pair(cancel_focus.clone(), cancel_focus)
                            .on_result(move |result, window, cx| {
                                let accepted = !result.cancelled
                                    && result.choice.as_deref() == Some("sheet-accept");
                                respond(accepted, window, cx);
                            })
                    })
                },
            )
            .read(cx)
            .clone();
        alert
    }
}
