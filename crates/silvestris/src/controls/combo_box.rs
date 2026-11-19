//! Editable combo box (HIG pp. 296–298). Text entry reuses the field's IME,
//! clipboard and undo-free editing model; the popup offers discrete suggestions.
use gpui::{App, ElementId, Entity, SharedString, Window, div, prelude::*, px};
use std::rc::Rc;

use super::{ControlSize, PopUpButton, TextField};

type Choose = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ComboBox {
    id: ElementId,
    label: SharedString,
    field: Entity<TextField>,
    suggestions: Vec<SharedString>,
    disabled: bool,
    on_choose: Option<Choose>,
}

impl ComboBox {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        field: Entity<TextField>,
        suggestions: Vec<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            field,
            suggestions,
            disabled: false,
            on_choose: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// A suggestion selection also emits the field's ordinary Changed event.
    pub fn on_choose(
        mut self,
        callback: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_choose = Some(Rc::new(callback));
        self
    }
}

impl RenderOnce for ComboBox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let field = self.field.clone();
        let label = self.label.clone();
        self.field.update(cx, |field, cx| {
            field.set_disabled(self.disabled, window, cx);
            field.set_size(ControlSize::Small, cx);
            field.set_label(label, cx);
        });
        let selected = self
            .suggestions
            .iter()
            .position(|text| text.as_ref() == field.read(cx).text());
        let options = self.suggestions.clone();
        let choose = self.on_choose;
        div()
            .id(self.id)
            .flex()
            .items_center()
            .gap(px(4.0))
            .min_w_0()
            .role(gpui::Role::ComboBox)
            .aria_label(self.label.clone())
            .child(div().flex_1().min_w_0().child(self.field))
            .child(
                PopUpButton::new("suggestions", self.label, self.suggestions)
                    .selected(selected)
                    .arrow_only()
                    .size(ControlSize::Small)
                    .disabled(self.disabled || options.is_empty())
                    .on_choose(move |index, window, cx| {
                        if let Some(value) = options.get(*index) {
                            field.update(cx, |field, cx| {
                                field.replace_from_control(value.to_string(), window, cx)
                            });
                            if let Some(choose) = &choose {
                                choose(value, window, cx);
                            }
                        }
                    }),
            )
    }
}
