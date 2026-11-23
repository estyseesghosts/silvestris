//! Reusable views for interaction scenarios.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    deferred, div, point, prelude::*, px, App, Context, Entity, FocusHandle, Focusable, Render,
    SharedString, Subscription, WeakEntity, Window,
};

use crate::command::{CommandRegistry, CommandScope};
use crate::controls::table::virtual_table;
use crate::controls::{
    ConfirmationFocus, ConfirmationSheet, SourceList, SourceListEvent, TextField, TextFieldEvent,
};
use crate::core::command::{CommandDescriptor, MenuEntry, MenuModel};
use crate::core::source_list::SourceNode;
use crate::menu::PopupMenu;

/// A window containing one text field, its focus handle and its event log.
pub struct TextFieldFixture {
    field: Entity<TextField>,
    focus: FocusHandle,
    events: Rc<RefCell<Vec<TextFieldEvent>>>,
    _subscription: Subscription,
}

impl TextFieldFixture {
    /// Creates the field and subscribes to its events.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = cx.new(|cx| TextField::new(window, cx));
        let focus = field.read(cx).focus_handle(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        let subscription = cx.subscribe(&field, move |_, _, event: &TextFieldEvent, _| {
            sink.borrow_mut().push(event.clone());
        });
        Self {
            field,
            focus,
            events,
            _subscription: subscription,
        }
    }

    /// The field entity.
    #[must_use]
    pub fn field(&self) -> &Entity<TextField> {
        &self.field
    }

    /// The field's focus handle.
    #[must_use]
    pub fn focus(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// The current text.
    #[must_use]
    pub fn text(&self, cx: &App) -> String {
        self.field.read(cx).text().to_string()
    }

    /// Events received since the fixture was created.
    #[must_use]
    pub fn events(&self) -> Vec<TextFieldEvent> {
        self.events.borrow().clone()
    }
}

impl Render for TextFieldFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "fixture-text-field".into())
            .size_full()
            .p(px(20.))
            .child(self.field.clone())
    }
}

/// A window containing one two-level source list, its focus handle and its event log.
pub struct SourceListFixture {
    list: Entity<SourceList>,
    focus: FocusHandle,
    events: Rc<RefCell<Vec<SourceListEvent>>>,
    _subscription: Subscription,
}

impl SourceListFixture {
    /// Creates a root row with one child and subscribes to the list's events.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let mut root = SourceNode::new("root", "Collection");
        root.children = vec![SourceNode::new("child", "Document")];
        let list = cx.new(|cx| SourceList::new(vec![root], cx).expect("source list"));
        let focus = list.read(cx).focus_handle(cx);
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        let subscription = cx.subscribe(&list, move |_, _, event: &SourceListEvent, _| {
            sink.borrow_mut().push(event.clone());
        });
        Self {
            list,
            focus,
            events,
            _subscription: subscription,
        }
    }

    /// The list entity.
    #[must_use]
    pub fn list(&self) -> &Entity<SourceList> {
        &self.list
    }

    /// The list's focus handle.
    #[must_use]
    pub fn focus(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// The selected row id.
    #[must_use]
    pub fn selected(&self, cx: &App) -> Option<String> {
        self.list.read(cx).state().selected().map(str::to_owned)
    }

    /// Events received since the fixture was created.
    #[must_use]
    pub fn events(&self) -> Vec<SourceListEvent> {
        self.events.borrow().clone()
    }
}

impl Render for SourceListFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(220.)).h_full().child(self.list.clone())
    }
}

/// A plain document window with a title and one focus handle.
pub struct DocumentWindowFixture {
    focus: FocusHandle,
    title: SharedString,
}

impl DocumentWindowFixture {
    /// Creates a focusable document window.
    pub fn new(cx: &mut Context<Self>, title: impl Into<SharedString>) -> Self {
        Self {
            focus: cx.focus_handle(),
            title: title.into(),
        }
    }

    /// The document's focus handle.
    #[must_use]
    pub fn focus(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// The document title.
    #[must_use]
    pub fn title(&self) -> SharedString {
        self.title.clone()
    }
}

impl Render for DocumentWindowFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().track_focus(&self.focus).child(
            div()
                .debug_selector(|| "document-body".into())
                .p(px(20.))
                .child(self.title.clone()),
        )
    }
}

/// A utility panel with a title and one focus handle.
pub struct UtilityPanelFixture {
    focus: FocusHandle,
    label: SharedString,
}

impl UtilityPanelFixture {
    /// Creates a focusable utility panel.
    pub fn new(cx: &mut Context<Self>, label: impl Into<SharedString>) -> Self {
        Self {
            focus: cx.focus_handle(),
            label: label.into(),
        }
    }

    /// The panel's focus handle.
    #[must_use]
    pub fn focus(&self) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for UtilityPanelFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| "utility-panel".into())
            .w(px(220.))
            .h_full()
            .track_focus(&self.focus)
            .p(px(12.))
            .child(self.label.clone())
    }
}

/// A table stub built on the real virtual-table helper.
pub struct TableStubFixture {
    rows: usize,
}

impl TableStubFixture {
    /// Creates a stub with `rows` alternating rows.
    #[must_use]
    pub fn new(rows: usize) -> Self {
        Self { rows }
    }
}

impl Render for TableStubFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let table = virtual_table("fixture-table", self.rows, |range, _, _| {
            range
                .map(|index| {
                    div()
                        .id(("fixture-row", index))
                        .debug_selector(move || format!("table-row-{index}"))
                        .h(px(20.))
                        .child(format!("Row {index}"))
                })
                .collect()
        })
        .size_full();
        div().size_full().child(table)
    }
}

/// A window with a confirmation sheet, its response log and a background focus.
pub struct ConfirmationSheetFixture {
    focus: ConfirmationFocus,
    background: FocusHandle,
    visible: bool,
    responses: Rc<RefCell<Vec<bool>>>,
    this: WeakEntity<Self>,
}

impl ConfirmationSheetFixture {
    /// Creates a window with the sheet already shown.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = ConfirmationFocus::new(cx);
        focus.focus_cancel(window, cx);
        Self {
            focus,
            background: cx.focus_handle(),
            visible: true,
            responses: Rc::new(RefCell::new(Vec::new())),
            this: cx.entity().downgrade(),
        }
    }

    /// Shows the sheet again and focuses Cancel.
    pub fn show(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.visible = true;
        self.focus.focus_cancel(window, cx);
        cx.notify();
    }

    /// Whether the sheet is currently shown.
    #[must_use]
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Responses in order: true for accept, false for cancel or Escape.
    #[must_use]
    pub fn responses(&self) -> Vec<bool> {
        self.responses.borrow().clone()
    }
}

impl Render for ConfirmationSheetFixture {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let focus = self.focus.clone();
        let responses = self.responses.clone();
        let this = self.this.clone();
        let sheet = self.visible.then(|| {
            ConfirmationSheet::new(
                "Delete document?",
                "This cannot be undone.",
                "Delete",
                focus,
                move |accepted, window, cx| {
                    responses.borrow_mut().push(accepted);
                    this.update(cx, |this, cx| {
                        this.visible = false;
                        window.focus(&this.background, cx);
                        cx.notify();
                    })
                    .ok();
                },
            )
            .into_any_element()
        });
        div()
            .relative()
            .size_full()
            .track_focus(&self.background)
            .child(
                div()
                    .debug_selector(|| "sheet-background".into())
                    .p(px(20.))
                    .child("Background"),
            )
            .children(sheet)
    }
}

/// A window with one open popup menu and a command hit counter.
pub struct PopupMenuFixture {
    menu: Entity<PopupMenu>,
    focus: FocusHandle,
    hits: Rc<Cell<usize>>,
}

impl PopupMenuFixture {
    /// Creates a registry, a one-command menu and opens it.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hits = Rc::new(Cell::new(0));
        let count = hits.clone();
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::action("fixture.run", "Run"),
                    move |_, _, _| count.set(count.get() + 1),
                    cx,
                )
                .unwrap();
        });
        let model = MenuModel {
            label: "View".into(),
            entries: vec![MenuEntry::Command("fixture.run".into())],
        };
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let menu = cx.new(|cx| PopupMenu::new(model, registry, window, cx));
        menu.update(cx, |menu, cx| {
            menu.open(point(px(10.), px(10.)), window, cx)
        });
        Self { menu, focus, hits }
    }

    /// The menu's focus restoration target.
    #[must_use]
    pub fn focus(&self) -> FocusHandle {
        self.focus.clone()
    }

    /// Number of times the fixture command ran.
    #[must_use]
    pub fn hits(&self) -> usize {
        self.hits.get()
    }

    /// Whether the menu is still open.
    #[must_use]
    pub fn is_open(&self, cx: &App) -> bool {
        self.menu.read(cx).is_open()
    }
}

impl Render for PopupMenuFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .child(deferred(self.menu.clone()).priority(1))
    }
}

/// A shell command and a text field, with the shell binding excluded while the
/// field has focus.
///
/// This is the generic shape of lotus's guest-input isolation: shell shortcuts
/// must not reach a focused text field.
pub struct KeyIsolationFixture {
    registry: Entity<CommandRegistry>,
    field: Entity<TextField>,
    shell_focus: FocusHandle,
    field_focus: FocusHandle,
    hits: Rc<Cell<usize>>,
}

impl KeyIsolationFixture {
    /// Registers one shortcut command and one text field.
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let hits = Rc::new(Cell::new(0));
        let count = hits.clone();
        let registry = cx.new(|_| CommandRegistry::default());
        registry.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::action("fixture.shell", "Shell Action")
                        .shortcut("secondary-r"),
                    move |_, _, _| count.set(count.get() + 1),
                    cx,
                )
                .unwrap();
        });
        let bindings = registry
            .read(cx)
            .bindings("FixtureShell && !SilvestrisTextField", cx)
            .expect("fixture bindings");
        cx.bind_keys(bindings);
        let field = cx.new(|cx| TextField::new(window, cx));
        let field_focus = field.read(cx).focus_handle(cx);
        let shell_focus = cx.focus_handle();
        window.focus(&shell_focus, cx);
        Self {
            registry,
            field,
            shell_focus,
            field_focus,
            hits,
        }
    }

    /// The text field's focus handle.
    #[must_use]
    pub fn field_focus(&self) -> FocusHandle {
        self.field_focus.clone()
    }

    /// The shell's focus handle.
    #[must_use]
    pub fn shell_focus(&self) -> FocusHandle {
        self.shell_focus.clone()
    }

    /// Number of times the shell command ran.
    #[must_use]
    pub fn hits(&self) -> usize {
        self.hits.get()
    }
}

impl Render for KeyIsolationFixture {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        CommandScope::new("fixture-shell", "FixtureShell", self.registry.clone()).child(
            div()
                .size_full()
                .track_focus(&self.shell_focus)
                .child(div().p(px(20.)).child(self.field.clone())),
        )
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::{
        ConfirmationSheetFixture, DocumentWindowFixture, KeyIsolationFixture, PopupMenuFixture,
        SourceListFixture, TableStubFixture, TextFieldFixture, UtilityPanelFixture,
    };
    use crate::controls::{SourceListEvent, TextFieldEvent};
    use crate::test_support::scenario::Scenario;

    #[test]
    fn types_and_submits_through_the_input_handler() {
        let mut scenario = Scenario::new();
        let window = scenario.open(TextFieldFixture::new);
        let fixture = window.entity(scenario.app()).expect("text field fixture");

        let focus = scenario.update(|_, cx| fixture.read(cx).focus());
        scenario.focus(&focus);
        scenario.type_text("hello");

        assert_eq!(scenario.update(|_, cx| fixture.read(cx).text(cx)), "hello");
        assert!(
            scenario.update(|_, cx| fixture.read(cx).events().contains(&TextFieldEvent::Changed))
        );

        scenario.press("enter");
        assert!(scenario.update(|_, cx| fixture
            .read(cx)
            .events()
            .contains(&TextFieldEvent::Submitted)));
    }

    #[test]
    fn navigates_a_source_list() {
        let mut scenario = Scenario::new();
        let window = scenario.open(|_, cx| SourceListFixture::new(cx));
        let fixture = window.entity(scenario.app()).expect("source list fixture");

        let focus = scenario.update(|_, cx| fixture.read(cx).focus());
        scenario.focus(&focus);
        scenario.assert_focused(&focus);

        scenario.press("home right right");
        assert_eq!(
            scenario.update(|_, cx| fixture.read(cx).selected(cx)),
            Some("child".into())
        );
        assert!(scenario.update(|_, cx| fixture
            .read(cx)
            .events()
            .contains(&SourceListEvent::Selected("child".into()))));

        scenario.press("left left");
        assert_eq!(
            scenario.update(|_, cx| fixture.read(cx).selected(cx)),
            Some("root".into())
        );
    }

    #[test]
    fn documents_keep_focus_per_window() {
        let mut scenario = Scenario::new();
        let first = scenario.open(|_, cx| DocumentWindowFixture::new(cx, "First"));
        let second = scenario.open(|_, cx| DocumentWindowFixture::new(cx, "Second"));
        let first_fixture = first.entity(scenario.app()).expect("first document");
        let second_fixture = second.entity(scenario.app()).expect("second document");
        let first_focus = scenario.update(|_, cx| first_fixture.read(cx).focus());
        let second_focus = scenario.update(|_, cx| second_fixture.read(cx).focus());

        assert_eq!(scenario.windows().len(), 2);

        scenario.activate(*first);
        scenario.focus(&first_focus);
        scenario.assert_focused(&first_focus);

        scenario.activate(*second);
        scenario.focus(&second_focus);
        scenario.assert_focused(&second_focus);

        let first_is_focused = scenario.update(|window, _| first_focus.is_focused(window));
        assert!(!first_is_focused);
    }

    #[test]
    fn utility_panel_takes_focus() {
        let mut scenario = Scenario::new();
        let _document = scenario.open(|_, cx| DocumentWindowFixture::new(cx, "Document"));
        let panel = scenario.open(|_, cx| UtilityPanelFixture::new(cx, "Inspector"));
        let panel_fixture = panel.entity(scenario.app()).expect("utility panel");
        let panel_focus = scenario.update(|_, cx| panel_fixture.read(cx).focus());

        scenario.focus(&panel_focus);
        scenario.assert_focused(&panel_focus);
        assert_eq!(scenario.bounds("utility-panel").size.width, px(220.));
    }

    #[test]
    fn table_stub_renders_rows() {
        let mut scenario = Scenario::new();
        scenario.open(|_, _| TableStubFixture::new(100));

        assert_eq!(scenario.bounds("table-row-0").size.height, px(20.));
    }

    #[test]
    fn confirmation_sheet_accepts_and_cancels() {
        let mut scenario = Scenario::new();
        let window = scenario.open(ConfirmationSheetFixture::new);
        let fixture = window.entity(scenario.app()).expect("sheet fixture");

        assert!(scenario.update(|_, cx| fixture.read(cx).visible()));
        scenario.click("sheet-accept");
        assert_eq!(
            scenario.update(|_, cx| fixture.read(cx).responses()),
            vec![true]
        );
        assert!(!scenario.update(|_, cx| fixture.read(cx).visible()));

        scenario.update(|window, cx| fixture.update(cx, |f, cx| f.show(window, cx)));
        scenario.draw();
        scenario.press("escape");
        assert_eq!(
            scenario.update(|_, cx| fixture.read(cx).responses()),
            vec![true, false]
        );
    }

    #[test]
    fn popup_menu_dispatches_and_restores_focus() {
        let mut scenario = Scenario::new();
        let window = scenario.open(PopupMenuFixture::new);
        let fixture = window.entity(scenario.app()).expect("menu fixture");
        let focus = scenario.update(|_, cx| fixture.read(cx).focus());

        scenario.press("home enter");
        assert_eq!(scenario.update(|_, cx| fixture.read(cx).hits()), 1);
        assert!(!scenario.update(|_, cx| fixture.read(cx).is_open(cx)));
        scenario.assert_focused(&focus);
    }

    #[test]
    fn shell_shortcut_does_not_reach_a_focused_text_field() {
        let mut scenario = Scenario::new();
        let window = scenario.open(KeyIsolationFixture::new);
        let fixture = window
            .entity(scenario.app())
            .expect("key isolation fixture");
        let field_focus = scenario.update(|_, cx| fixture.read(cx).field_focus());
        let shell_focus = scenario.update(|_, cx| fixture.read(cx).shell_focus());
        let shortcut = if cfg!(target_os = "macos") {
            "cmd-r"
        } else {
            "ctrl-r"
        };

        scenario.focus(&field_focus);
        scenario.press(shortcut);
        assert_eq!(scenario.update(|_, cx| fixture.read(cx).hits()), 0);

        scenario.focus(&shell_focus);
        scenario.press(shortcut);
        assert_eq!(scenario.update(|_, cx| fixture.read(cx).hits()), 1);
    }
}
