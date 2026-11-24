//! Typed bindings between an application model and a control (#61).
//!
//! A `Binding<T>` is two closures: read the model, write it. It is not a
//! reactive framework and has no macro DSL. Programmatic writes go through
//! `set`, which changes the model silently; controls only call it from their
//! own user callback. That is the whole feedback-loop rule: a setter never
//! emits a user event, so updating a control from the model cannot come back
//! as another model update.
//!
//! `ListController<T>` is the collection half: stable ids, selection by id,
//! sorting and filtering. A programmatic `select` does not call a UI callback;
//! the app wires its list/source-list callback separately and decides what to
//! do with a user selection.

use std::cmp::Ordering;
use std::rc::Rc;

use gpui::{prelude::*, App, ElementId, Entity, SharedString, Window};
use silvestris_core::source_list::SourceNode;

use crate::controls::{
    CheckState, Checkbox, ControlSize, PopUpButton, RadioGroup, RadioOption, Slider, TextField,
    TextFieldEvent,
};

type Getter<T> = Rc<dyn Fn(&App) -> T>;
type Setter<T> = Rc<dyn Fn(T, &mut App)>;
type Parser<T> = Rc<dyn Fn(&str) -> Option<T>>;
type Formatter<T> = Rc<dyn Fn(&T) -> String>;
type IdFn<T> = Rc<dyn Fn(&T) -> ItemId>;
type FilterFn<T> = Rc<dyn Fn(&T) -> bool>;
type SortFn<T> = Rc<dyn Fn(&T, &T) -> Ordering>;

/// A value in an application model.
pub struct Binding<T: Clone + 'static> {
    get: Getter<T>,
    set: Setter<T>,
}

impl<T: Clone + 'static> Clone for Binding<T> {
    fn clone(&self) -> Self {
        Self {
            get: self.get.clone(),
            set: self.set.clone(),
        }
    }
}

impl<T: Clone + 'static> Binding<T> {
    #[must_use]
    pub fn new(get: impl Fn(&App) -> T + 'static, set: impl Fn(T, &mut App) + 'static) -> Self {
        Self {
            get: Rc::new(get),
            set: Rc::new(set),
        }
    }

    /// A binding to one field of an entity.
    #[must_use]
    pub fn field<M: 'static>(
        entity: &Entity<M>,
        get: impl Fn(&M) -> T + 'static,
        set: impl Fn(&mut M, T) + 'static,
    ) -> Self {
        let read = entity.downgrade();
        let write = entity.downgrade();
        Self {
            get: Rc::new(move |cx| {
                read.upgrade()
                    .map(|entity| get(entity.read(cx)))
                    .expect("binding source entity was dropped")
            }),
            set: Rc::new(move |value, cx| {
                let _ = write.upgrade().map(|entity| {
                    entity.update(cx, |model, cx| {
                        set(model, value);
                        cx.notify();
                    })
                });
            }),
        }
    }

    #[must_use]
    pub fn value(&self, cx: &App) -> T {
        (self.get)(cx)
    }

    /// Programmatic write. It changes the model and asks observers to repaint;
    /// it does not fire any control callback.
    pub fn set(&self, value: T, cx: &mut App) {
        (self.set)(value, cx);
    }
}

impl Binding<bool> {
    /// A checkbox bound to a bool. Mixed reads as false; use
    /// [`Binding<CheckState>`] when the model needs three states.
    #[must_use]
    pub fn checkbox(
        self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
    ) -> BoundCheckbox {
        BoundCheckbox {
            id: id.into(),
            label: label.into(),
            binding: self,
        }
    }
}

impl Binding<usize> {
    /// A radio group bound to the selected index.
    #[must_use]
    pub fn radio(
        self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        options: Vec<RadioOption>,
    ) -> BoundRadio {
        BoundRadio {
            id: id.into(),
            label: label.into(),
            options,
            binding: self,
        }
    }

    /// A pop-up button bound to the selected index.
    #[must_use]
    pub fn popup(
        self,
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        items: Vec<SharedString>,
    ) -> BoundPopUp {
        BoundPopUp {
            id: id.into(),
            label: label.into(),
            items,
            binding: self,
        }
    }
}

impl Binding<f32> {
    /// A slider bound to its value.
    #[must_use]
    pub fn slider(self, id: impl Into<ElementId>, label: impl Into<SharedString>) -> BoundSlider {
        BoundSlider {
            id: id.into(),
            label: label.into(),
            binding: self,
        }
    }
}

/// A checkbox whose value lives in a binding.
#[derive(IntoElement)]
pub struct BoundCheckbox {
    id: ElementId,
    label: SharedString,
    binding: Binding<bool>,
}

impl RenderOnce for BoundCheckbox {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let checked = self.binding.value(cx);
        let binding = self.binding;
        Checkbox::new(self.id, self.label)
            .checked(checked)
            .on_change(move |state, _, cx| binding.set(*state == CheckState::On, cx))
    }
}

/// A radio group whose selected index lives in a binding.
#[derive(IntoElement)]
pub struct BoundRadio {
    id: ElementId,
    label: SharedString,
    options: Vec<RadioOption>,
    binding: Binding<usize>,
}

impl RenderOnce for BoundRadio {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let selected = self.binding.value(cx);
        let binding = self.binding;
        RadioGroup::new(self.id, self.label, self.options)
            .selected(Some(selected))
            .on_select(move |index, _, cx| binding.set(*index, cx))
    }
}

/// A pop-up button whose selected index lives in a binding.
#[derive(IntoElement)]
pub struct BoundPopUp {
    id: ElementId,
    label: SharedString,
    items: Vec<SharedString>,
    binding: Binding<usize>,
}

impl RenderOnce for BoundPopUp {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let selected = self.binding.value(cx);
        let binding = self.binding;
        PopUpButton::new(self.id, self.label, self.items)
            .selected(Some(selected))
            .size(ControlSize::Regular)
            .on_choose(move |index, _, cx| binding.set(*index, cx))
    }
}

/// A slider whose value lives in a binding.
#[derive(IntoElement)]
pub struct BoundSlider {
    id: ElementId,
    label: SharedString,
    binding: Binding<f32>,
}

impl RenderOnce for BoundSlider {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let value = self.binding.value(cx);
        let binding = self.binding;
        Slider::new(self.id, self.label, value)
            .on_change(move |value, _, cx| binding.set(*value, cx))
    }
}

/// A numeric or enum value shown in a `TextField`, with a parser and a
/// formatter. `sync` writes the model into the field without a `Changed`
/// event; `commit` parses the field text back into the model after a user edit.
pub struct TextBinding<T: Clone + 'static> {
    field: Entity<TextField>,
    binding: Binding<T>,
    parse: Parser<T>,
    format: Formatter<T>,
}

impl<T: Clone + 'static> Clone for TextBinding<T> {
    fn clone(&self) -> Self {
        Self {
            field: self.field.clone(),
            binding: self.binding.clone(),
            parse: self.parse.clone(),
            format: self.format.clone(),
        }
    }
}

impl<T: Clone + 'static> TextBinding<T> {
    #[must_use]
    pub fn new(
        field: Entity<TextField>,
        binding: Binding<T>,
        parse: impl Fn(&str) -> Option<T> + 'static,
        format: impl Fn(&T) -> String + 'static,
    ) -> Self {
        Self {
            field,
            binding,
            parse: Rc::new(parse),
            format: Rc::new(format),
        }
    }

    /// Write the model into the field when it differs. Silent: `set_text` does
    /// not emit `TextFieldEvent::Changed`.
    pub fn sync(&self, cx: &mut App) {
        let text = (self.format)(&self.binding.value(cx));
        if self.field.read(cx).text() != text {
            let field = self.field.clone();
            field.update(cx, |field, cx| field.set_text(text, cx));
        }
    }

    /// Parse the field and write it into the model. Call it from a
    /// `TextFieldEvent::Changed` subscription; an unparsable value leaves the
    /// model alone.
    pub fn commit(&self, cx: &mut App) -> bool {
        let text = self.field.read(cx).text().to_string();
        let Some(value) = (self.parse)(&text) else {
            return false;
        };
        self.binding.set(value, cx);
        true
    }

    #[must_use]
    pub fn field(&self) -> &Entity<TextField> {
        &self.field
    }

    /// Subscribe to the field's user edits. The returned subscription belongs
    /// to the caller; keep it alive for as long as the binding.
    pub fn subscribe(
        &self,
        cx: &mut App,
        mut on_commit: impl FnMut(bool, &mut App) + 'static,
    ) -> gpui::Subscription {
        let binding = self.clone();
        cx.subscribe(&self.field, move |_, event: &TextFieldEvent, cx| {
            if matches!(event, TextFieldEvent::Changed) {
                let changed = binding.commit(cx);
                on_commit(changed, cx);
            }
        })
    }
}

/// One item's stable identity.
pub type ItemId = SharedString;

/// Stable-id list state: items, selection, sort and filter.
pub struct ListController<T> {
    id: IdFn<T>,
    items: Vec<T>,
    visible: Vec<usize>,
    /// Selected ids in selection order; the last one is the lead.
    selected: Vec<ItemId>,
    /// Where a Shift-extension starts (HIG p. 333 list selection).
    anchor: Option<ItemId>,
    filter: Option<FilterFn<T>>,
    sort: Option<SortFn<T>>,
}

impl<T: 'static> ListController<T> {
    #[must_use]
    pub fn new(id: impl Fn(&T) -> ItemId + 'static) -> Self {
        Self {
            id: Rc::new(id),
            items: Vec::new(),
            visible: Vec::new(),
            selected: Vec::new(),
            anchor: None,
            filter: None,
            sort: None,
        }
    }

    /// Replace the items, keeping the selection and the sort/filter. Selection
    /// survives a reload because it is an id, not an index.
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.refresh();
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// How many items pass the filter.
    #[must_use]
    pub fn visible_len(&self) -> usize {
        self.visible.len()
    }

    pub fn set_filter(&mut self, filter: Option<impl Fn(&T) -> bool + 'static>) {
        self.filter = filter.map(|f| Rc::new(f) as Rc<dyn Fn(&T) -> bool>);
        self.refresh();
    }

    pub fn set_sort(&mut self, sort: Option<impl Fn(&T, &T) -> Ordering + 'static>) {
        self.sort = sort.map(|f| Rc::new(f) as Rc<dyn Fn(&T, &T) -> Ordering>);
        self.refresh();
    }

    fn refresh(&mut self) {
        let mut visible: Vec<usize> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| self.filter.as_ref().is_none_or(|filter| filter(item)))
            .map(|(index, _)| index)
            .collect();
        if let Some(sort) = &self.sort {
            visible.sort_by(|left, right| sort(&self.items[*left], &self.items[*right]));
        }
        self.visible = visible;
    }

    /// Items in current sort/filter order.
    pub fn visible(&self) -> impl Iterator<Item = &T> {
        self.visible.iter().map(|index| &self.items[*index])
    }

    #[must_use]
    pub fn visible_items(&self) -> Vec<&T> {
        self.visible().collect()
    }

    /// Ids in current order, for a source list or table model.
    #[must_use]
    pub fn visible_ids(&self) -> Vec<ItemId> {
        self.visible().map(|item| (self.id)(item)).collect()
    }

    /// The lead selected id: the only one for a single selection, the most
    /// recently added for a multiple one.
    #[must_use]
    pub fn selection(&self) -> Option<&ItemId> {
        self.selected.last()
    }

    /// Every selected id, in the order it was selected.
    #[must_use]
    pub fn selected_ids(&self) -> &[ItemId] {
        &self.selected
    }

    #[must_use]
    pub fn is_selected(&self, id: &ItemId) -> bool {
        self.selected.contains(id)
    }

    /// Where the next Shift-extension starts.
    #[must_use]
    pub fn anchor(&self) -> Option<&ItemId> {
        self.anchor.as_ref()
    }

    /// Select by id, programmatically. This does not call any UI callback.
    /// Replaces any multiple selection. Returns whether the selection changed.
    pub fn select(&mut self, id: impl Into<ItemId>) -> bool {
        let id = id.into();
        self.anchor = Some(id.clone());
        if self.selected.len() == 1 && self.selected[0] == id {
            return false;
        }
        self.selected = vec![id];
        true
    }

    /// Replace the selection. The last id becomes the lead and the anchor.
    /// Returns whether the selection changed.
    pub fn set_selection(&mut self, ids: Vec<ItemId>) -> bool {
        let mut unique: Vec<ItemId> = Vec::with_capacity(ids.len());
        for id in ids {
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        self.anchor = unique.last().cloned();
        if unique == self.selected {
            return false;
        }
        self.selected = unique;
        true
    }

    /// Add or remove one id (Command/Control-click). The id becomes the
    /// anchor either way.
    pub fn toggle(&mut self, id: impl Into<ItemId>) {
        let id = id.into();
        if let Some(position) = self.selected.iter().position(|s| s == &id) {
            self.selected.remove(position);
        } else {
            self.selected.push(id.clone());
        }
        self.anchor = Some(id);
    }

    /// Select the visible range from the anchor to `id` (Shift-click or
    /// Shift-arrow), keeping the anchor. Without an anchor this is `select`.
    /// `id` becomes the lead.
    pub fn extend_to(&mut self, id: impl Into<ItemId>) {
        let id = id.into();
        let anchor = self.anchor.clone();
        let (Some(anchor), Some(to)) = (
            anchor.as_ref().and_then(|a| self.visible_index_of(a)),
            self.visible_index_of(&id),
        ) else {
            self.select(id);
            return;
        };
        let range: Vec<usize> = if anchor <= to {
            (anchor..=to).collect()
        } else {
            (to..=anchor).rev().collect()
        };
        self.selected = range
            .into_iter()
            .map(|visible| (self.id)(&self.items[self.visible[visible]]))
            .collect();
    }

    /// Select every visible item.
    pub fn select_all(&mut self) {
        self.selected = self.visible_ids();
        self.anchor = self.selected.first().cloned();
    }

    pub fn clear_selection(&mut self) {
        self.selected.clear();
        self.anchor = None;
    }

    /// The lead selected item, when it is still present.
    #[must_use]
    pub fn selected(&self) -> Option<&T> {
        let id = self.selected.last()?;
        self.items.iter().find(|item| &(self.id)(item) == id)
    }

    /// Selected items still present, in visible order.
    #[must_use]
    pub fn selected_items(&self) -> Vec<&T> {
        self.visible()
            .filter(|item| self.selected.contains(&(self.id)(item)))
            .collect()
    }

    /// The item at `index` in visible order.
    #[must_use]
    pub fn visible_item(&self, index: usize) -> Option<&T> {
        self.visible.get(index).map(|item| &self.items[*item])
    }

    /// Position of `id` in visible order.
    #[must_use]
    pub fn visible_index_of(&self, id: &ItemId) -> Option<usize> {
        self.visible
            .iter()
            .position(|item| &(self.id)(&self.items[*item]) == id)
    }

    /// The stable id of `item`.
    #[must_use]
    pub fn id_of(&self, item: &T) -> ItemId {
        (self.id)(item)
    }

    #[must_use]
    pub fn index_of(&self, id: &ItemId) -> Option<usize> {
        self.items.iter().position(|item| &(self.id)(item) == id)
    }

    /// Feed a source list without changing selection semantics.
    #[must_use]
    pub fn source_nodes(&self, label: impl Fn(&T) -> String) -> Vec<SourceNode> {
        self.visible
            .iter()
            .map(|index| {
                let item = &self.items[*index];
                SourceNode::new((self.id)(item), label(item))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext, Window};

    use super::*;
    use crate::controls::TextField;

    struct Empty;

    impl Render for Empty {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div().size_full()
        }
    }

    struct BoundView {
        binding: Binding<bool>,
    }

    impl Render for BoundView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            gpui::div().size_full().child(
                self.binding
                    .clone()
                    .checkbox("bound-checkbox", "Primary")
                    .into_any_element(),
            )
        }
    }

    fn draw(cx: &mut VisualTestContext) {
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
    }

    #[test]
    fn a_programmatic_write_repaints_without_firing_the_control_callback() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let writes = Rc::new(Cell::new(0));
        let counter = writes.clone();
        let (model, binding) = app.update(|cx| {
            let model = cx.new(|_| false);
            let binding = Binding::field(
                &model,
                |value: &bool| *value,
                move |value: &mut bool, next| {
                    counter.set(counter.get() + 1);
                    *value = next;
                },
            );
            (model, binding)
        });
        let (view, cx) = app.add_window_view(|_, _| BoundView {
            binding: binding.clone(),
        });
        draw(cx);

        cx.update(|_, cx| binding.set(true, cx));
        draw(cx);
        assert!(cx.update(|_, cx| *model.read(cx)));
        assert_eq!(writes.get(), 1, "the setter ran once");
        assert!(view.read_with(cx, |view, cx| view.binding.value(cx)));
        assert_eq!(
            writes.get(),
            1,
            "rendering a new value does not write it back"
        );

        let bounds = cx.debug_bounds("bound-checkbox").expect("checkbox");
        cx.simulate_click(bounds.center(), Modifiers::default());
        draw(cx);
        assert_eq!(writes.get(), 2);
        assert!(!cx.update(|_, cx| *model.read(cx)));
    }

    #[test]
    fn selection_survives_sort_filter_and_reload() {
        let mut controller = ListController::new(|item: &(u32, String)| item.0.to_string().into());
        controller.set_items(vec![
            (3, "Charlie".into()),
            (1, "Alpha".into()),
            (2, "Bravo".into()),
        ]);
        controller.select("2");
        controller.set_sort(Some(|a: &(u32, String), b: &(u32, String)| a.1.cmp(&b.1)));
        assert_eq!(controller.visible_ids(), vec!["1", "2", "3"]);
        assert_eq!(controller.selected().map(|item| item.0), Some(2));
        controller.set_filter(Some(|item: &(u32, String)| item.0 != 1));
        assert_eq!(controller.visible_ids(), vec!["2", "3"]);
        assert_eq!(controller.selected().map(|item| item.0), Some(2));
        controller.set_items(vec![(2, "Bravo".into()), (4, "Delta".into())]);
        assert_eq!(controller.selected().map(|item| item.0), Some(2));
        assert_eq!(controller.len(), 2);
        controller.set_filter(Some(|item: &(u32, String)| item.0 == 2));
        assert_eq!(controller.visible_len(), 1, "the new filter applies");
        assert_eq!(controller.visible_ids(), vec!["2"]);
        controller.clear_selection();
        assert!(controller.selection().is_none());
    }

    #[test]
    fn text_binding_formats_programmatic_values_and_parses_user_edits() {
        let mut app = TestAppContext::single();
        app.update(|cx| crate::theme::install(crate::ThemePreference::default(), cx));
        let model = app.update(|cx| cx.new(|_| 3u32));
        let window = app.add_window(|_, _| Empty);
        let field = window
            .update(&mut app, |_, window, cx| {
                cx.new(|cx| TextField::new(window, cx))
            })
            .unwrap();
        let binding = Binding::field(
            &model,
            |value: &u32| *value,
            |value: &mut u32, next| *value = next,
        );
        let text = TextBinding::new(
            field.clone(),
            binding,
            |text| text.trim().parse::<u32>().ok(),
            |value| value.to_string(),
        );
        app.update(|cx| {
            model.update(cx, |model, _| *model = 7);
            text.sync(cx);
        });
        assert_eq!(app.update(|cx| field.read(cx).text().to_string()), "7");

        app.update(|cx| {
            field.update(cx, |field, cx| field.set_text("12", cx));
            assert!(text.commit(cx));
            assert_eq!(*model.read(cx), 12);
            field.update(cx, |field, cx| field.set_text("nope", cx));
            assert!(!text.commit(cx));
            assert_eq!(*model.read(cx), 12);
        });
    }
}
