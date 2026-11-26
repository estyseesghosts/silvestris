//! Component catalog (#135). Keys 1-4 switch Aqua / Dark Aqua / Geist Light
//! / Geist Dark while no text field is focused.

use gpui::{
    actions, canvas, div, point, prelude::*, px, size, AnyElement, App, Bounds, Context, Entity,
    KeyBinding, Window,
};
use silvestris::binding::{Binding, ListController};
use silvestris::bottom_bar::{
    BottomBar, BottomBarControlStyle, BottomBarItem, BottomBarSize, BottomBarStatus,
};
use silvestris::chrome::{open_window, window_options, WindowFrame};
use silvestris::command::{self, CommandButton, CommandRegistry, InvokeCommand};
use silvestris::controls::{
    BrowserNode, Button, ButtonStyle, CheckState, Checkbox, ColumnAlign, ColumnBrowser,
    ConfirmationFocus, ConfirmationSheet, ControlSize, Disclosure, DisclosureStyle, GroupBox,
    Notice, Orientation, PopUpButton, PopupStyle, ProgressIndicator, ProgressStyle, RadioGroup,
    RadioOption, RowAccessory, RowStatus, ScrollArrows, ScrollAxes, ScrollView, Scrollbar,
    ScrollerPreferences, Segment, SegmentMode, SegmentStyle, SegmentedControl, Separator,
    SeparatorAxis, ShortcutRecorder, Slider, SortDirection, SourceList, SourceListEvent,
    SourceListStyle, Stepper, TabView, TableColumn, TableView, TextArea, TextAreaEvent, TextField,
    TextFieldEvent, ThumbStyle, TrackClick, Well,
};
use silvestris::core::command::{CommandDescriptor, CommandId, MenuEntry, MenuModel};
use silvestris::file_panel::{MemoryPanelStateStore, OpenPanel, SavePanel};
use silvestris::menu::PopupMenu;
use silvestris::modal::{self, ModalResult, ModalScope, ModalSession};
use silvestris::preferences::{
    measure_content, pane_body_natural, pane_viewport, Form, FormRow, Navigation, PreferencePane,
    PreferencesController, PreferencesSizing, PreferencesToolbar,
};
use silvestris::responder::{
    Responder, ResponderChain, ResponderEntry, ResponderScope, ResponderState,
};
use silvestris::toolbar::{
    Toolbar, ToolbarDisplayMode, ToolbarItem, ToolbarItemStyle, ToolbarSegment,
};
use silvestris::transient::TransientLayer;
use silvestris::window_role::FrameStore;
use silvestris::window_role::{open_role, WindowRole};
use silvestris::{paint, ActiveTheme, ThemeId, ThemePreference};

actions!(gallery, [FocusNext, FocusPrev]);

/// One window opened from the gallery's role row. It draws the role's chrome so
/// the title-bar buttons and resizability are visible rather than asserted.
struct RoleWindow {
    role: WindowRole,
    title: gpui::SharedString,
}

impl gpui::Render for RoleWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let theme = cx.theme().clone();
        let label = format!(
            "{} window. Resizable: {}. Close: {}. Minimize: {}. Zoom: {}.",
            self.role.label(),
            self.role.spec().resizable,
            self.role.spec().close,
            self.role.spec().minimize,
            self.role.spec().zoom,
        );
        WindowFrame::new(self.title.clone()).role(self.role).child(
            div()
                .size_full()
                .bg(silvestris::convert::hsla(theme.colors.window))
                .p(px(16.))
                .text_size(px(13.))
                .text_color(silvestris::convert::hsla(theme.colors.text))
                .child(label),
        )
    }
}

/// A minimal coordinator-hosted preferences window. General and Advanced are
/// short panes; Privacy is taller than the display so only its body scrolls.
struct PreferencesDemo {
    controller: PreferencesController,
}

impl PreferencesDemo {
    fn new(cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        Self {
            controller: PreferencesController::new(
                "Gallery Preferences",
                vec![
                    PreferencePane::new("general", "General"),
                    PreferencePane::new("advanced", "Advanced"),
                    PreferencePane::new("privacy", "Privacy"),
                ],
                PreferencesSizing::new(668.0, 22.0 + 32.0),
            )
            .focus_return(focus),
        }
    }

    fn navigate(&mut self, navigation: Navigation, window: &mut Window, cx: &mut Context<Self>) {
        let changed = match navigation {
            Navigation::Back => self.controller.go_back(window, cx),
            Navigation::Forward => self.controller.go_forward(window, cx),
            Navigation::ShowAll => self.controller.show_all(window, cx),
        };
        if changed {
            cx.notify();
        }
    }
}

fn demo_form(rows: usize, height: bool) -> Form {
    let mut form = Form::new(120.0);
    for index in 0..rows {
        form = form.row(
            FormRow::new(
                format!("Option {index}"),
                Checkbox::new(format!("demo-option-{index}"), "Enabled")
                    .checked(height && index % 2 == 0)
                    .on_change(|_, _, _| {}),
            )
            .dependent_on(Binding::<bool>::new(|_| true, |_, _| {})),
        );
    }
    form = form.row(
        FormRow::new(
            "Font",
            Checkbox::new("demo-font", "Monospaced").checked(true),
        )
        .id("demo-font-row")
        .error("Unknown font family"),
    );
    form
}

impl Render for PreferencesDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let weak = this.downgrade();
        let on_navigate = {
            let weak = weak.clone();
            move |navigation: Navigation, window: &mut Window, cx: &mut App| {
                if let Some(this) = weak.upgrade() {
                    this.update(cx, |demo, cx| demo.navigate(navigation, window, cx));
                }
            }
        };
        let toolbar = PreferencesToolbar::new(self.controller.navigator()).on_navigate(on_navigate);
        let current = self.controller.current().cloned();
        let body: AnyElement = match current.as_ref().map(|id| id.as_ref()) {
            Some("advanced") => demo_form(8, false).into_any_element(),
            Some("privacy") => demo_form(28, true).into_any_element(),
            _ => demo_form(4, true).into_any_element(),
        };
        let scrolls = self.controller.pane_scrolls();
        let measured = {
            let weak = weak.clone();
            move |size: gpui::Size<gpui::Pixels>, window: &mut Window, cx: &mut App| {
                if let Some(this) = weak.upgrade() {
                    window.defer(cx, move |window, cx| {
                        this.update(cx, |demo, cx| {
                            demo.controller.measure(size, window, cx);
                            cx.notify();
                        });
                    });
                }
            }
        };
        // Advance an in-flight resize after layout, once per frame.
        let tick = weak.clone();
        window.defer(cx, move |window, cx| {
            if let Some(this) = tick.upgrade() {
                this.update(cx, |demo, cx| {
                    let more = demo.controller.tick(window);
                    if more {
                        cx.notify();
                    }
                });
            }
        });
        WindowFrame::new(self.controller.title())
            .role(WindowRole::Preferences)
            .fixed_size()
            .toolbar(toolbar, theme.metrics.toolbar_height)
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .pt(px(12.0))
                    .when_some(
                        self.controller.current().is_some().then_some(()),
                        |el, ()| {
                            el.child(div().flex_1().min_h_0().child(pane_viewport(
                                measure_content(pane_body_natural().child(body), measured),
                                scrolls,
                            )))
                        },
                    )
                    .when(self.controller.current().is_none(), |el| {
                        let open = weak.clone();
                        el.child(
                            div()
                                .flex_1()
                                .p(px(24.0))
                                .flex()
                                .flex_col()
                                .gap(px(12.0))
                                .text_size(px(theme.type_.size_regular))
                                .child("Choose a pane; the window resizes to its content.")
                                .child(
                                    Button::new("demo-open-general", "General")
                                        .size(ControlSize::Small)
                                        .on_click(move |_, window, cx| {
                                            if let Some(this) = open.upgrade() {
                                                this.update(cx, |demo, cx| {
                                                    if demo.controller.select(
                                                        Some("general".into()),
                                                        window,
                                                        cx,
                                                    ) {
                                                        cx.notify();
                                                    }
                                                });
                                            }
                                        }),
                                ),
                        )
                    }),
            )
    }
}

struct Gallery {
    menu: Entity<PopupMenu>,
    commands: Entity<CommandRegistry>,
    selected_view: gpui::SharedString,
    selected_tab: gpui::SharedString,
    sidebar: silvestris::core::layout::SidebarPreference,
    source_list: Entity<SourceList>,
    secondary_list: Entity<SourceList>,
    gallery_scroll: gpui::ScrollHandle,
    fidelity_scroll: gpui::ScrollHandle,
    files_table: Entity<TableView<GalleryFile>>,
    browser: Entity<ColumnBrowser>,
    fidelity_scroll_small: gpui::ScrollHandle,
    last_source: gpui::SharedString,
    details_expanded: bool,
    bottom_bar_small: bool,
    checks: [CheckState; 3],
    focus: gpui::FocusHandle,
    last_click: gpui::SharedString,
    fields: Vec<Entity<TextField>>,
    toolbar_search: Entity<TextField>,
    composer: Entity<TextArea>,
    last_field: gpui::SharedString,
    combo: Entity<TextField>,
    radio: usize,
    progress: f32,
    routing: Entity<ResponderChain>,
    routing_field: gpui::FocusHandle,
    routing_canvas: gpui::FocusHandle,
    routing_status: gpui::SharedString,
    transient: Entity<TransientLayer>,
    transient_status: gpui::SharedString,
    shortcut_status: gpui::SharedString,
    bound: Entity<BoundModel>,
    list: ListController<(u32, gpui::SharedString)>,
    list_sorted: bool,
    modal: Option<ModalDemo>,
    panel_state: std::rc::Rc<MemoryPanelStateStore>,
    panel_frames: std::rc::Rc<GalleryFrames>,
}

#[derive(Default)]
struct GalleryFrames(std::cell::RefCell<std::collections::BTreeMap<String, Bounds<gpui::Pixels>>>);

impl FrameStore for GalleryFrames {
    fn load(&self, key: &str, _: &App) -> Option<Bounds<gpui::Pixels>> {
        self.0.borrow().get(key).copied()
    }

    fn save(&self, key: &str, bounds: Bounds<gpui::Pixels>, _: &mut App) {
        self.0.borrow_mut().insert(key.to_string(), bounds);
    }
}

/// S5: a small document window whose File menu opens the Save sheet
/// (minimal and expanded) and the application-modal Open dialog. It is a real
/// consumer: Save writes the text, Open reads it, and the accessory's backup
/// checkbox writes a second copy.
struct DocumentDemo {
    state: std::rc::Rc<MemoryPanelStateStore>,
    frames: std::rc::Rc<GalleryFrames>,
    content: std::rc::Rc<std::cell::RefCell<String>>,
    status: std::rc::Rc<std::cell::RefCell<String>>,
    backup: std::rc::Rc<std::cell::Cell<bool>>,
}

impl DocumentDemo {
    fn new(state: std::rc::Rc<MemoryPanelStateStore>, frames: std::rc::Rc<GalleryFrames>) -> Self {
        Self {
            state,
            frames,
            content: std::rc::Rc::new(std::cell::RefCell::new(
                "The quick brown fox jumps over the lazy dog.\n\n\
                 File > Save As writes this text to the chosen file.\n\
                 File > Open reads a text file back into this document."
                    .to_string(),
            )),
            status: std::rc::Rc::new(std::cell::RefCell::new(String::new())),
            backup: std::rc::Rc::new(std::cell::Cell::new(false)),
        }
    }

    fn provider() -> std::rc::Rc<silvestris::file_panel::StdFsProvider> {
        std::rc::Rc::new(silvestris::file_panel::StdFsProvider::new())
    }

    fn directory(
        provider: &silvestris::file_panel::StdFsProvider,
    ) -> silvestris::file_panel::LocationId {
        provider.home().unwrap_or_else(|| provider.root())
    }
}

impl gpui::Render for DocumentDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let state = self.state.clone();
        let frames = self.frames.clone();
        let content = self.content.clone();
        let status = self.status.clone();
        let backup = self.backup.clone();
        let file = PopUpButton::new(
            "document-file-menu",
            "File",
            vec![
                "Save\u{2026}".into(),
                "Save As (expanded)\u{2026}".into(),
                "Open\u{2026}".into(),
            ],
        )
        .size(ControlSize::Regular)
        .min_width(76.0)
        .on_choose(move |index, window, cx| {
            let content_for_save = content.clone();
            let status_for_save = status.clone();
            let backup_for_save = backup.clone();
            let content_for_open = content.clone();
            let status_for_open = status.clone();
            let this_for_save = this.clone();
            let this_for_open = this.clone();
            match *index {
                0 | 1 => {
                    let expanded = *index == 1;
                    let provider = Self::provider();
                    let directory = Self::directory(&provider);
                    let accessory_backup = backup_for_save.clone();
                    SavePanel::new(provider)
                        .directory(directory)
                        .name("Untitled.txt")
                        .message("Choose a destination for this document.")
                        .allowed_types(["txt", "md"])
                        .expanded(expanded)
                        .state_store(state.clone())
                        .accessory(move |_, _cx| {
                            let backup = accessory_backup.clone();
                            Checkbox::new("save-backup", "Keep a backup copy")
                                .size(ControlSize::Small)
                                .checked(backup.get())
                                .on_change(move |state, _, _| {
                                    backup.set(*state == CheckState::On);
                                })
                                .into_any_element()
                        })
                        .on_finish(move |outcome, cx| {
                            let Some(path) = outcome.first_path() else {
                                return;
                            };
                            let text = content_for_save.borrow().clone();
                            let result = match outcome.disposition {
                                Some(silvestris::file_panel::SaveDisposition::New) => {
                                    std::fs::OpenOptions::new()
                                        .write(true)
                                        .create_new(true)
                                        .open(path)
                                        .and_then(|mut file| {
                                            std::io::Write::write_all(&mut file, text.as_bytes())
                                        })
                                }
                                _ => std::fs::write(path, text.as_bytes()),
                            };
                            match result {
                                Ok(()) => {
                                    if backup_for_save.get() {
                                        let mut backup_path = path.clone();
                                        let name = backup_path
                                            .file_name()
                                            .map(|name| name.to_string_lossy().into_owned())
                                            .unwrap_or_else(|| "document".into());
                                        backup_path.set_file_name(format!("{name}.bak"));
                                        let _ = std::fs::write(&backup_path, text.as_bytes());
                                    }
                                    *status_for_save.borrow_mut() =
                                        format!("Saved {}", path.display());
                                }
                                Err(error) => {
                                    *status_for_save.borrow_mut() = format!("Save failed: {error}");
                                }
                            }
                            this_for_save.update(cx, |_, cx| cx.notify());
                        })
                        .begin_sheet(window.window_handle(), window, cx);
                }
                _ => {
                    let provider = Self::provider();
                    let directory = Self::directory(&provider);
                    OpenPanel::new(provider)
                        .directory(directory)
                        .title("Gallery: Open")
                        .state_store(state.clone())
                        .frame_store(frames.clone())
                        .on_finish(move |outcome, cx| {
                            let Some(path) = outcome.first_path() else {
                                return;
                            };
                            match std::fs::read_to_string(path) {
                                Ok(text) => {
                                    *content_for_open.borrow_mut() = text;
                                    *status_for_open.borrow_mut() =
                                        format!("Opened {}", path.display());
                                }
                                Err(error) => {
                                    *status_for_open.borrow_mut() = format!("Open failed: {error}");
                                }
                            }
                            this_for_open.update(cx, |_, cx| cx.notify());
                        })
                        .run_modal(window, cx);
                }
            }
        });
        let body = self.content.borrow().clone();
        let status = self.status.borrow().clone();
        WindowFrame::new("Untitled")
            .role(WindowRole::Document)
            .toolbar(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .size_full()
                    .child(file)
                    .child(
                        div()
                            .text_size(px(theme.type_.size_small))
                            .text_color(silvestris::convert::hsla(theme.colors.text_secondary))
                            .child("Untitled"),
                    ),
                32.0,
            )
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .bg(silvestris::convert::hsla(theme.colors.content))
                    .p(px(18.))
                    .text_size(px(13.))
                    .text_color(silvestris::convert::hsla(theme.colors.text))
                    .child(div().w(px(440.)).child(body))
                    .when(!status.is_empty(), |el| {
                        el.child(
                            div()
                                .text_size(px(11.))
                                .text_color(silvestris::convert::hsla(theme.colors.text_secondary))
                                .child(status),
                        )
                    }),
            )
    }
}

#[derive(Clone)]
struct GalleryFile {
    id: u32,
    name: gpui::SharedString,
    size: u32,
    done: bool,
}

struct BoundModel {
    primary: bool,
    level: f32,
}

#[derive(Clone)]
struct ModalDemo {
    session: Entity<ModalSession>,
    focus: ConfirmationFocus,
}

impl Responder for Gallery {
    fn handles(&self, id: &CommandId) -> bool {
        id.as_str() == "edit.copy"
    }

    fn state(&self, _id: &CommandId, _cx: &App) -> ResponderState {
        ResponderState::enabled()
    }

    fn perform(&mut self, _id: &CommandId, _window: &mut Window, cx: &mut Context<Self>) {
        self.routing_status = "window".into();
        cx.notify();
    }
}

impl Gallery {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = |label: &'static str,
                     placeholder: &'static str,
                     window: &mut Window,
                     cx: &mut Context<Self>| {
            let f = cx.new(|cx| TextField::new(window, cx));
            f.update(cx, |f, cx| {
                f.set_label(label, cx);
                f.set_placeholder(placeholder, cx);
            });
            cx.subscribe(&f, move |g, f, event: &TextFieldEvent, cx| {
                let text = f.read(cx).text().to_string();
                g.last_field = match event {
                    TextFieldEvent::Changed => format!("{label} changed: {text}"),
                    TextFieldEvent::Submitted => format!("{label} submitted: {text}"),
                    TextFieldEvent::EndedEditing => format!("{label} ended editing: {text}"),
                }
                .into();
                cx.notify();
            })
            .detach();
            f
        };
        let name = field("Name", "Name", window, cx);
        let password = field("Password", "Password", window, cx);
        password.update(cx, |f, cx| f.set_masked(true, cx));
        let small = field("Small", "Small field", window, cx);
        small.update(cx, |f, cx| f.set_size(ControlSize::Small, cx));
        let disabled = field("Disabled", "Disabled", window, cx);
        disabled.update(cx, |f, cx| {
            f.set_text("Not editable", cx);
            f.set_disabled(true, window, cx);
        });
        let search = field("Search", "Search", window, cx);
        search.update(cx, |field, cx| field.set_search(true, cx));
        let toolbar_search = field("Toolbar Search", "Search", window, cx);
        let combo = field("Location", "Location", window, cx);
        let mut collection =
            silvestris::core::source_list::SourceNode::new("collection", "Collection");
        collection.icon = Some("folder".into());
        collection.badge = Some("10000".into());
        collection.children = (0..10000)
            .map(|i| {
                silvestris::core::source_list::SourceNode::new(
                    format!("item-{i}"),
                    format!("Document {i}: a long title to exercise truncation"),
                )
            })
            .collect();
        let mut unavailable =
            silvestris::core::source_list::SourceNode::new("unavailable", "Unavailable");
        unavailable.disabled = true;
        let source_list = cx.new(|cx| {
            SourceList::new(
                vec![
                    silvestris::core::source_list::SourceNode::section(
                        "library",
                        "Library",
                        vec![collection, unavailable],
                    ),
                    silvestris::core::source_list::SourceNode::new("archive", "Archive"),
                ],
                cx,
            )
            .expect("unique gallery source IDs")
        });
        source_list.update(cx, |list, cx| {
            list.set_icon_renderer(
                |role, _, disabled, _, _| {
                    let folder = role == "folder";
                    // Original gallery-only vector fixture, not extracted Apple art.
                    canvas(
                        |_, _, _| (),
                        move |b, _, window, _| {
                            let c = if disabled {
                                gpui::rgb(0x9a9a9a)
                            } else if folder {
                                gpui::rgb(0x58a9da)
                            } else {
                                gpui::rgb(0xf3f3f3)
                            };
                            let body = Bounds::new(
                                point(b.origin.x + px(1.), b.origin.y + px(4.)),
                                size(px(14.), px(10.)),
                            );
                            window.paint_quad(gpui::fill(body, c));
                            if folder {
                                window.paint_quad(gpui::fill(
                                    Bounds::new(
                                        point(b.origin.x + px(2.), b.origin.y + px(2.)),
                                        size(px(6.), px(3.)),
                                    ),
                                    c,
                                ));
                            }
                        },
                    )
                    .size_full()
                    .into_any_element()
                },
                cx,
            );
        });
        cx.subscribe(&source_list, |g, _, event: &SourceListEvent, cx| {
            g.last_source = format!("{event:?}").into();
            cx.notify();
        })
        .detach();
        let secondary_list = cx.new(|cx| {
            let inbox = silvestris::core::source_list::SourceNode::new("inbox", "Inbox");
            let sent = silvestris::core::source_list::SourceNode::new("sent", "Sent");
            let mut rss = silvestris::core::source_list::SourceNode::new("rss", "RSS");
            rss.children = vec![silvestris::core::source_list::SourceNode::new(
                "feed",
                "Apple Hot News",
            )];
            SourceList::new(
                vec![silvestris::core::source_list::SourceNode::section(
                    "mailboxes",
                    "Mailboxes",
                    vec![inbox, sent, rss],
                )],
                cx,
            )
            .expect("unique secondary source IDs")
        });
        secondary_list.update(cx, |list, cx| {
            list.style(SourceListStyle::Secondary, cx);
            list.set_row_accessories(
                |row| match row.id.as_str() {
                    "inbox" => vec![
                        RowAccessory::badge(4),
                        RowAccessory::status(RowStatus::Running),
                    ],
                    "sent" => vec![
                        RowAccessory::status(RowStatus::Paused),
                        RowAccessory::Hover(Box::new(RowAccessory::Eject)),
                    ],
                    "rss" => vec![
                        RowAccessory::status(RowStatus::Error),
                        RowAccessory::Spinner,
                    ],
                    "feed" => vec![
                        RowAccessory::status(RowStatus::Idle),
                        RowAccessory::button("refresh", "Refresh feed", "\u{21bb}", |_, _| {}),
                    ],
                    _ => Vec::new(),
                },
                cx,
            );
            list.set_expanded("rss", true, cx);
            list.select("inbox", cx);
        });
        let composer = cx.new(|cx| {
            let mut area = TextArea::new(window, cx);
            area.set_placeholder(
                "Message the assistant. Return submits, Shift-Return adds a line.",
                cx,
            );
            area.set_label("Composer", cx);
            area
        });
        cx.subscribe(&composer, |g, _, _: &TextAreaEvent, cx| {
            g.last_field = "composer changed".into();
            cx.notify();
        })
        .detach();
        let commands = cx.new(|_| CommandRegistry::default());
        let toggle_sidebar = cx.listener(|g, _: &CommandId, _, cx| {
            g.sidebar.expanded = !g.sidebar.expanded;
            g.sync_sidebar_command(cx);
            cx.notify();
        });
        let toggle_bottom_bar = cx.listener(|g, _: &CommandId, _, cx| {
            g.bottom_bar_small = !g.bottom_bar_small;
            g.sync_bottom_bar_command(cx);
            cx.notify();
        });
        commands.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::toggle("view.sidebar", "Sidebar", true)
                        .shortcut("secondary-b"),
                    toggle_sidebar,
                    cx,
                )
                .unwrap()
        });
        commands.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::toggle("gallery.bottom-small", "Small Bar", false),
                    toggle_bottom_bar,
                    cx,
                )
                .unwrap()
        });
        commands.update(cx, |registry, cx| {
            // Demonstrates the mixed (dash) menu state, HIG pp. 170-171.
            registry
                .register(
                    CommandDescriptor::toggle("view.badges", "Row Badges", false).mixed(true),
                    |_, _, _| {},
                    cx,
                )
                .unwrap()
        });
        cx.observe(&commands, |_, _, cx| cx.notify()).detach();
        commands.update(cx, |registry, cx| {
            // Bare digits switch themes. The registry keeps typing keys away
            // from text clients, so no `!SilvestrisText` here (#57).
            for (id, label, key, theme) in [
                ("theme.aqua-light", "Aqua Light", "1", ThemeId::AQUA_LIGHT),
                ("theme.aqua-dark", "Aqua Dark", "2", ThemeId::AQUA_DARK),
                (
                    "theme.geist-light",
                    "Geist Light",
                    "3",
                    ThemeId::GEIST_LIGHT,
                ),
                ("theme.geist-dark", "Geist Dark", "4", ThemeId::GEIST_DARK),
            ] {
                registry
                    .register(
                        CommandDescriptor::action(id, label).shortcut(key),
                        move |_, _, cx| set(theme, cx),
                        cx,
                    )
                    .unwrap();
            }
        });
        let bindings = commands
            .read(cx)
            .bindings(GALLERY_KEYS, cx)
            .expect("gallery command bindings");
        cx.bind_keys(bindings);
        let routing_field = cx.focus_handle();
        let routing_canvas = cx.focus_handle();
        let this = cx.entity();
        let routing = cx.new(|_| ResponderChain::new());
        let field_weak = this.downgrade();
        let canvas_weak = this.downgrade();
        routing.update(cx, |chain, _| {
            chain.push(ResponderEntry::new(
                ResponderScope::Application,
                "window",
                &this,
            ));
            chain.push(
                ResponderEntry::custom(ResponderScope::Control, "field")
                    .when_focused(&routing_field)
                    .handles(|id, _| id.as_str() == "edit.copy")
                    .perform(move |_, _, cx| {
                        let _ = field_weak.update(cx, |gallery, cx| {
                            gallery.routing_status = "field".into();
                            cx.notify();
                        });
                    }),
            );
            chain.push(
                ResponderEntry::custom(ResponderScope::Control, "canvas")
                    .when_focused(&routing_canvas)
                    .capture_keys([])
                    .perform(move |_, _, cx| {
                        let _ = canvas_weak.update(cx, |gallery, cx| {
                            gallery.routing_status = "canvas".into();
                            cx.notify();
                        });
                    }),
            );
        });
        commands.update(cx, |registry, cx| {
            registry
                .register(
                    CommandDescriptor::action("edit.copy", "Copy"),
                    |_, _, _| {},
                    cx,
                )
                .unwrap()
        });
        commands.update(cx, |registry, cx| {
            for (id, label) in [
                ("gallery.add", "Add Item"),
                ("gallery.remove", "Remove Item"),
                ("gallery.action", "Action"),
            ] {
                registry
                    .register(CommandDescriptor::action(id, label), |_, _, _| {}, cx)
                    .unwrap();
            }
        });
        let files_table = cx.new(|cx| {
            let files = cx.new(|_| {
                let mut c = ListController::new(|f: &GalleryFile| f.id.to_string().into());
                c.set_items(
                    (0..2000)
                        .map(|i| GalleryFile {
                            id: i,
                            name: format!("Document {i}").into(),
                            size: (i * 37) % 900,
                            done: i % 3 == 0,
                        })
                        .collect(),
                );
                c
            });
            let rename_target = files.clone();
            let columns = vec![
                TableColumn::text("name", "Name", |f: &GalleryFile| f.name.clone())
                    .width(150.)
                    .sort_by_text()
                    .editable(move |f: &GalleryFile, text: &str, cx| {
                        let id = f.id;
                        let text: gpui::SharedString = text.to_string().into();
                        let target = rename_target.clone();
                        // The controller is leased during commit; rename on
                        // the next turn of the loop.
                        cx.defer(move |cx| {
                            target.update(cx, |c, cx| {
                                let mut items: Vec<GalleryFile> = c.visible().cloned().collect();
                                if let Some(item) = items.iter_mut().find(|f| f.id == id) {
                                    item.name = text.clone();
                                }
                                c.set_items(items);
                                cx.notify();
                            });
                        });
                        true
                    }),
                TableColumn::custom("done", "Done", |f: &GalleryFile, _, _, _| {
                    Checkbox::new(("gallery-done", f.id as usize), "")
                        .state(CheckState::from(f.done))
                        .size(ControlSize::Small)
                        .into_any_element()
                })
                .width(44.)
                .resizable(false)
                .align(ColumnAlign::Center),
                TableColumn::text("size", "Size", |f: &GalleryFile| {
                    format!("{} KB", f.size).into()
                })
                .width(70.)
                .align(ColumnAlign::Trailing)
                .sortable(|a: &GalleryFile, b: &GalleryFile| a.size.cmp(&b.size)),
            ];
            let mut table = TableView::new(files, columns, cx);
            table.set_sort("name", SortDirection::Ascending, cx);
            table
        });
        let browser = cx.new(|cx| {
            let mut browser = ColumnBrowser::new(
                vec![
                    BrowserNode::branch("home", "Home"),
                    BrowserNode::leaf("readme", "Read Me"),
                ],
                cx,
            );
            browser.set_children(
                "home",
                vec![
                    BrowserNode::branch("desktop", "Desktop"),
                    BrowserNode::branch("documents", "Documents"),
                    BrowserNode::leaf("notes", "Notes.txt"),
                ],
                cx,
            );
            browser.set_path(&["home".into(), "documents".into()], cx);
            browser
        });
        let bound = cx.new(|_| BoundModel {
            primary: true,
            level: 0.4,
        });
        let mut list =
            ListController::new(|item: &(u32, gpui::SharedString)| item.0.to_string().into());
        list.set_items(vec![
            (3, "Charlie".into()),
            (1, "Alpha".into()),
            (2, "Bravo".into()),
        ]);
        list.select("2");
        let transient = cx.new(|cx| TransientLayer::new(window, cx));
        cx.observe(&transient, |gallery, layer, cx| {
            gallery.transient_status = match (layer.read(cx).len(), layer.read(cx).top_kind()) {
                (0, _) => "none".into(),
                (depth, Some(kind)) => format!("{depth} (top: {})", kind.label()).into(),
                (depth, None) => format!("{depth}").into(),
            };
            cx.notify();
        })
        .detach();
        let menu = cx.new(|cx| {
            PopupMenu::new(
                MenuModel {
                    label: "View".into(),
                    entries: vec![
                        MenuEntry::Command("view.sidebar".into()),
                        MenuEntry::Command("view.badges".into()),
                        MenuEntry::Separator,
                        MenuEntry::Submenu {
                            label: "Appearance".into(),
                            entries: [
                                "theme.aqua-light",
                                "theme.aqua-dark",
                                "theme.geist-light",
                                "theme.geist-dark",
                            ]
                            .into_iter()
                            .map(|id| MenuEntry::Command(id.into()))
                            .collect(),
                        },
                    ],
                },
                commands.clone(),
                window,
                cx,
            )
        });
        menu.update(cx, |menu, cx| menu.set_layer(transient.clone(), cx));
        menu.update(cx, |menu, cx| menu.set_chain(routing.clone(), cx));
        Gallery {
            menu,
            commands,
            selected_view: "grid".into(),
            selected_tab: "date".into(),
            sidebar: silvestris::core::layout::SidebarPreference {
                expanded: true,
                width: 240.,
            },
            source_list,
            secondary_list,
            gallery_scroll: gpui::ScrollHandle::new(),
            fidelity_scroll: gpui::ScrollHandle::new(),
            files_table,
            browser,
            fidelity_scroll_small: gpui::ScrollHandle::new(),
            last_source: "No source-list event".into(),
            details_expanded: false,
            bottom_bar_small: false,
            focus: cx.focus_handle(),
            checks: [CheckState::Off, CheckState::On, CheckState::Mixed],
            last_click: "none".into(),
            fields: vec![name, password, small, disabled, search],
            toolbar_search,
            composer,
            combo,
            radio: 0,
            progress: 0.42,
            last_field: "none".into(),
            routing,
            routing_field,
            routing_canvas,
            routing_status: "none".into(),
            transient,
            transient_status: "none".into(),
            shortcut_status: "unchanged".into(),
            bound,
            list,
            list_sorted: false,
            modal: None,
            panel_state: std::rc::Rc::new(MemoryPanelStateStore::new()),
            panel_frames: std::rc::Rc::new(GalleryFrames::default()),
        }
    }

    fn sync_sidebar_command(&mut self, cx: &mut Context<Self>) {
        let descriptor =
            CommandDescriptor::toggle("view.sidebar", "Sidebar", self.sidebar.expanded)
                .shortcut("secondary-b");
        self.commands
            .update(cx, |registry, cx| registry.publish(descriptor, cx).unwrap());
    }

    fn sync_bottom_bar_command(&mut self, cx: &mut Context<Self>) {
        let checked = self.bottom_bar_small;
        let descriptor = CommandDescriptor::toggle("gallery.bottom-small", "Small Bar", checked);
        self.commands
            .update(cx, |registry, cx| registry.publish(descriptor, cx).unwrap());
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let commands = self.commands.clone();
        let chain = self.routing.clone();
        let footer_commands = self.commands.clone();
        let label = theme.id.label();
        let view = cx.entity();
        let button = |id: &'static str, label: &'static str| {
            let view = view.clone();
            Button::new(id, label).on_click(move |_, _, cx| {
                view.update(cx, |g, cx| {
                    g.last_click = label.into();
                    cx.notify();
                })
            })
        };
        // Row 1 mirrors the measured 10.6 buttons: Force Quit (regular default)
        // and the Copy panel's equal-width small pair (M: button.push.widths).
        let buttons = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(button("force-quit", "Force Quit").default_button(true))
            .child(button("cancel", "Cancel"))
            .child(
                button("gradient", "Add")
                    .style(ButtonStyle::Gradient)
                    .tooltip("Add")
                    .icon(|color| div().text_color(color).child("+")),
            )
            .child(
                button("stop", "Stop")
                    .size(ControlSize::Small)
                    .min_width(86.),
            )
            .child(
                button("replace", "Replace")
                    .size(ControlSize::Small)
                    .min_width(86.),
            )
            .child(button("mini", "Mini").size(ControlSize::Mini))
            .child(button("disabled", "Disabled").disabled(true))
            .child(button("textured", "Textured").style(ButtonStyle::Textured))
            .child(
                div()
                    .text_size(px(theme.type_.size_small))
                    .text_color(silvestris::convert::hsla(theme.colors.text_secondary))
                    .child(format!("last click: {}", self.last_click)),
            );
        let disclosure = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                Disclosure::new(
                    "details",
                    if self.details_expanded {
                        "Hide details"
                    } else {
                        "Show details"
                    },
                )
                .expanded(self.details_expanded)
                .on_toggle(cx.listener(|g, expanded, _, cx| {
                    g.details_expanded = *expanded;
                    cx.notify();
                })),
            )
            .child(if self.details_expanded {
                "Hide details"
            } else {
                "Show details"
            })
            .child(Disclosure::new("disabled-details", "Unavailable details").disabled(true))
            .when(self.details_expanded, |el| {
                el.child("Additional information")
            });
        let small_text = px(theme.type_.size_small);
        let secondary = silvestris::convert::hsla(theme.colors.text_secondary);
        let fields = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .children(
                self.fields
                    .iter()
                    .map(|f| div().w(px(150.)).child(f.clone())),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child(self.last_field.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_end()
                    .gap(px(8.))
                    .child(div().w(px(360.)).child(self.composer.clone()))
                    .child(
                        Button::new("composer-undo", "Undo")
                            .size(ControlSize::Mini)
                            .on_click(cx.listener(|g, _, _, cx| {
                                let history = g.composer.read(cx).history();
                                history.update(cx, |history, cx| {
                                    history.undo(cx);
                                });
                            })),
                    ),
            );
        let routing_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                CommandButton::new("routing-copy", "edit.copy", self.commands.clone())
                    .chain(self.routing.clone()),
            )
            .child(
                Button::new("focus-routing-field", "Focus field")
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|g, _, window, cx| {
                        window.focus(&g.routing_field, cx);
                    })),
            )
            .child(
                div()
                    .track_focus(&self.routing_field)
                    .debug_selector(|| "routing-field".into())
                    .w(px(64.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(small_text)
                    .child("field"),
            )
            .child(
                Button::new("focus-routing-canvas", "Focus canvas")
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|g, _, window, cx| {
                        window.focus(&g.routing_canvas, cx);
                    })),
            )
            .child(
                div()
                    .track_focus(&self.routing_canvas)
                    .debug_selector(|| "routing-canvas".into())
                    .w(px(64.))
                    .h(px(22.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(small_text)
                    .child("canvas"),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child(format!("Copy handled by: {}", self.routing_status)),
            );
        let transient_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                Button::new("open-transient-menu", "Open View menu")
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|g, _, window, cx| {
                        g.menu.update(cx, |menu, cx| {
                            menu.open(point(px(120.), px(220.)), window, cx)
                        });
                    })),
            )
            .child(
                Button::new("open-modal-sheet", "Open modal sheet")
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|g, _, window, cx| {
                        let focus = ConfirmationFocus::new(cx);
                        let stops = focus.handles().to_vec();
                        let session = cx.new(|_| {
                            ModalSession::new(ModalScope::Document)
                                .with_focus_stops(stops)
                                .on_end(|_| {})
                        });
                        g.modal = Some(ModalDemo {
                            session: session.clone(),
                            focus,
                        });
                        modal::present(session.clone(), window, cx);
                        cx.observe(&session, |gallery, session, cx| {
                            if session.read(cx).is_ended() {
                                gallery.modal = None;
                                cx.notify();
                            }
                        })
                        .detach();
                    })),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child(format!("transient surfaces: {}", self.transient_status)),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child("Escape closes the innermost; the combo list shares this layer"),
            );

        let role_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .children(WindowRole::ALL.into_iter().map(|role| {
                Button::new(
                    match role {
                        WindowRole::Document => "role-document",
                        WindowRole::Utility => "role-utility",
                        WindowRole::Panel => "role-panel",
                        WindowRole::Preferences => "role-preferences",
                        WindowRole::Dialog => "role-dialog",
                        WindowRole::OpenPanel => "role-open-panel",
                        WindowRole::Assistant => "role-assistant",
                        WindowRole::About => "role-about",
                    },
                    role.label(),
                )
                .size(ControlSize::Small)
                .on_click(cx.listener(move |_, _, _, cx| {
                    let bounds = Bounds::centered(None, size(px(460.), px(280.)), cx);
                    let title: gpui::SharedString = format!("{} window", role.label()).into();
                    open_role(role, title.clone(), bounds, cx, |_, cx| {
                        cx.new(|_| RoleWindow { role, title })
                    })
                    .expect("open role window");
                }))
            }))
            .child(
                Button::new("role-preferences-demo", "Coordinator")
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|_, _, _, cx| {
                        let bounds = Bounds::centered(None, size(px(668.), px(360.)), cx);
                        open_role(
                            WindowRole::Preferences,
                            "Gallery Preferences",
                            bounds,
                            cx,
                            |_, cx| cx.new(PreferencesDemo::new),
                        )
                        .expect("open preferences demo");
                    })),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child("each button opens its role"),
            );

        let sidebar_shortcut = self
            .commands
            .read(cx)
            .shortcut(&CommandId::new("view.sidebar"))
            .map(gpui::SharedString::from);
        let shortcut_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                div().w(px(240.)).child(
                    ShortcutRecorder::new("sidebar-shortcut", "Toggle sidebar")
                        .shortcut(sidebar_shortcut)
                        .on_record({
                            let commands = self.commands.clone();
                            let view = cx.entity();
                            move |chord, _, cx| {
                                let id = CommandId::new("view.sidebar");
                                let result = commands.update(cx, |registry, cx| {
                                    registry.rebind(&id, chord.as_deref(), cx)
                                });
                                let outcome = match &result {
                                    Ok(warnings) if warnings.is_empty() => "rebound".to_string(),
                                    Ok(_) => "rebound (reserved shortcut)".to_string(),
                                    Err(error) => format!("refused: {error:?}"),
                                };
                                if result.is_ok() {
                                    let contexts = [GALLERY_KEYS];
                                    let _ = command::apply_bindings(&commands, &contexts, cx);
                                }
                                view.update(cx, |gallery, cx| {
                                    gallery.shortcut_status = outcome.into();
                                    cx.notify();
                                });
                            }
                        }),
                ),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child(format!("shortcut: {}", self.shortcut_status)),
            );

        let bound_primary = Binding::field(
            &self.bound,
            |model: &BoundModel| model.primary,
            |model: &mut BoundModel, value| model.primary = value,
        )
        .checkbox("bound-primary", "Primary");
        let bound_level = Binding::field(
            &self.bound,
            |model: &BoundModel| model.level,
            |model: &mut BoundModel, value| model.level = value,
        )
        .slider("bound-level", "Level");
        let bound_state = self.bound.read(cx);
        let binding_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(div().w(px(160.)).child(bound_primary))
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child(format!(
                        "primary={} level={:.1}",
                        bound_state.primary, bound_state.level
                    )),
            );

        let list_ids = self.list.visible_ids().join(", ");
        let list_selection = self
            .list
            .selection()
            .map(|id| id.to_string())
            .unwrap_or_else(|| "none".into());
        let list_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .child(
                Button::new(
                    "sort-list",
                    if self.list_sorted {
                        "Original order"
                    } else {
                        "Sort A–Z"
                    },
                )
                .size(ControlSize::Small)
                .on_click(cx.listener(|g, _, _, cx| {
                    g.list_sorted = !g.list_sorted;
                    if g.list_sorted {
                        g.list.set_sort(Some(
                            |a: &(u32, gpui::SharedString), b: &(u32, gpui::SharedString)| {
                                a.1.cmp(&b.1)
                            },
                        ));
                    } else {
                        g.list.set_sort(
                            None::<
                                fn(
                                    &(u32, gpui::SharedString),
                                    &(u32, gpui::SharedString),
                                ) -> std::cmp::Ordering,
                            >,
                        );
                    }
                    cx.notify();
                })),
            )
            .child(
                Button::new("filter-list", "Hide Alpha")
                    .size(ControlSize::Small)
                    .on_click(cx.listener(|g, _, _, cx| {
                        g.list
                            .set_filter(Some(|item: &(u32, gpui::SharedString)| item.0 != 1));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(small_text)
                    .text_color(secondary)
                    .child(format!("ids: {list_ids} · selected: {list_selection}")),
            );

        let cell = |caption: &'static str, content: gpui::AnyElement| {
            div()
                .flex_none()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(6.))
                .child(content)
                .child(
                    div()
                        .text_size(small_text)
                        .text_color(secondary)
                        .child(caption),
                )
        };
        let scroll = self.fidelity_scroll.clone();
        let scroll_small = self.fidelity_scroll_small.clone();
        let scroller_prefs = ScrollerPreferences::get(cx);
        let push_styles = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(24.))
            .child(cell(
                "Help",
                Button::new("fidelity-help", "Help")
                    .style(ButtonStyle::Help)
                    .into_any_element(),
            ))
            .child(cell(
                "Round",
                Button::new("fidelity-round", "Round")
                    .style(ButtonStyle::Round)
                    .icon(|ink| div().text_color(ink).child("\u{2699}"))
                    .into_any_element(),
            ))
            .child(cell(
                "Recessed",
                Button::new("fidelity-recessed", "Recessed")
                    .style(ButtonStyle::Recessed)
                    .into_any_element(),
            ))
            .child(cell(
                "Bevel",
                Button::new("fidelity-bevel", "Bevel")
                    .style(ButtonStyle::Bevel)
                    .into_any_element(),
            ))
            .child(cell(
                "Disclosure button",
                Disclosure::new("fidelity-disclosure-button", "Disclosure button")
                    .style(DisclosureStyle::Button)
                    .expanded(true)
                    .into_any_element(),
            ));
        let popups = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(24.))
            .child(cell(
                "Pop-up",
                PopUpButton::new(
                    "fidelity-popup",
                    "Appearance",
                    vec!["Blue".into(), "Graphite".into()],
                )
                .selected(Some(0))
                .size(ControlSize::Regular)
                .into_any_element(),
            ))
            .child(cell(
                "Textured pop-up",
                PopUpButton::new(
                    "fidelity-popup-textured",
                    "Scope",
                    vec!["Everything".into(), "Selection".into()],
                )
                .selected(Some(0))
                .size(ControlSize::Regular)
                .bezel(PopupStyle::Textured)
                .into_any_element(),
            ))
            .child(cell(
                "Disabled pop-up",
                PopUpButton::new(
                    "fidelity-popup-disabled",
                    "Unavailable",
                    vec!["Nothing".into()],
                )
                .disabled(true)
                .into_any_element(),
            ))
            .child(cell(
                "Combo box",
                div()
                    .w(px(180.))
                    .child(
                        silvestris::controls::ComboBox::new(
                            "locations",
                            "Location",
                            self.combo.clone(),
                            vec!["Home".into(), "Documents".into(), "Desktop".into()],
                        )
                        .layer(self.transient.clone()),
                    )
                    .into_any_element(),
            ));
        let checks = div()
            .flex()
            .items_center()
            .gap(px(16.))
            .children(
                [ControlSize::Regular, ControlSize::Small, ControlSize::Mini]
                    .into_iter()
                    .enumerate()
                    .map(|(i, size)| {
                        Checkbox::new(
                            ("catalog-check", i),
                            ["Regular", "Small", "Mini / mixed"][i],
                        )
                        .size(size)
                        .state(self.checks[i])
                        .on_change(cx.listener(
                            move |g, state: &CheckState, _, cx| {
                                g.checks[i] = *state;
                                cx.notify();
                            },
                        ))
                    }),
            )
            .child(
                Checkbox::new("disabled-check", "Disabled")
                    .checked(true)
                    .disabled(true),
            );
        let radios = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(32.))
            .child(cell(
                "Regular",
                RadioGroup::new(
                    "radio-group",
                    "Export scope",
                    vec![
                        RadioOption::new("Current Item"),
                        RadioOption::new("All Items"),
                        RadioOption::new("Selection").disabled(true),
                    ],
                )
                .selected(Some(self.radio))
                .on_select(cx.listener(|g, selected, _, cx| {
                    g.radio = *selected;
                    cx.notify();
                }))
                .into_any_element(),
            ))
            .child(cell(
                "Regular, preset",
                RadioGroup::new(
                    "fidelity-radio-regular",
                    "Regular radios",
                    vec![RadioOption::new("First"), RadioOption::new("Second")],
                )
                .selected(Some(0))
                .into_any_element(),
            ))
            .child(cell(
                "Mini",
                RadioGroup::new(
                    "fidelity-radio-mini",
                    "Mini radios",
                    vec![RadioOption::new("On"), RadioOption::new("Off")],
                )
                .size(ControlSize::Mini)
                .selected(Some(1))
                .into_any_element(),
            ));
        let sliders = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(32.))
            .child(cell(
                "Pointed, labelled ticks",
                div()
                    .w(px(240.))
                    .px(px(20.))
                    .child(
                        Slider::new("fidelity-slider-ticks", "Dock size", 0.6)
                            .thumb(ThumbStyle::Pointed)
                            .ticks([0.0, 1.0])
                            .tick_labels(["Small", "Large"])
                            .snap_to_ticks(true),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Round, drives the progress bars",
                div()
                    .w(px(200.))
                    .child(
                        Slider::new("export-progress", "Progress preview", self.progress)
                            .on_change(cx.listener(|g, value, _, cx| {
                                g.progress = *value;
                                cx.notify();
                            })),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Bound to a model",
                div().w(px(200.)).child(bound_level).into_any_element(),
            ))
            .child(cell(
                "Vertical",
                div()
                    .h(px(120.))
                    .w(px(80.))
                    .child(
                        Slider::new("fidelity-slider-vertical", "Level", 0.35)
                            .orientation(Orientation::Vertical)
                            .thumb(ThumbStyle::Pointed)
                            .ticks([0.0, 0.5, 1.0]),
                    )
                    .into_any_element(),
            ));
        let segments = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(24.))
            .child(cell(
                "Select one",
                SegmentedControl::new(
                    "view-selector",
                    "View",
                    vec![
                        Segment::new("grid", "Grid"),
                        Segment::new("list", "List"),
                        Segment::new("columns", "Columns").disabled(true),
                    ],
                )
                .selected(self.selected_view.clone())
                .on_select(cx.listener(|g, selected: &gpui::SharedString, _, cx| {
                    g.selected_view = selected.clone();
                    cx.notify();
                }))
                .into_any_element(),
            ))
            .child(cell(
                "Icons",
                SegmentedControl::new(
                    "fidelity-segments-icon",
                    "View",
                    vec![
                        Segment::new("grid", "Grid")
                            .icon(silvestris::icons::TemplateImage::IconView.role())
                            .show_label(false),
                        Segment::new("list", "List")
                            .icon(silvestris::icons::TemplateImage::ListView.role())
                            .show_label(false),
                        Segment::new("columns", "Columns")
                            .icon(silvestris::icons::TemplateImage::ColumnView.role())
                            .show_label(false)
                            .disabled(true),
                    ],
                )
                .selected("grid")
                .icon_renderer(silvestris::icons::role_renderer(
                    silvestris::icons::TemplateImageSlot::Segmented,
                ))
                .into_any_element(),
            ))
            .child(cell(
                "Template image roles (19 px toolbar slot)",
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(4.))
                    .children(
                        [
                            silvestris::icons::TemplateImage::Back,
                            silvestris::icons::TemplateImage::Forward,
                            silvestris::icons::TemplateImage::IconView,
                            silvestris::icons::TemplateImage::ListView,
                            silvestris::icons::TemplateImage::ColumnView,
                            silvestris::icons::TemplateImage::Search,
                            silvestris::icons::TemplateImage::Stop,
                            silvestris::icons::TemplateImage::Refresh,
                            silvestris::icons::TemplateImage::Add,
                            silvestris::icons::TemplateImage::Remove,
                            silvestris::icons::TemplateImage::Action,
                            silvestris::icons::TemplateImage::Info,
                            silvestris::icons::TemplateImage::Sidebar,
                            silvestris::icons::TemplateImage::Inspector,
                            silvestris::icons::TemplateImage::Eject,
                        ]
                        .into_iter()
                        .map(|image| {
                            image.element(
                                silvestris::icons::TemplateImageSlot::Toolbar,
                                theme.colors.text,
                            )
                        }),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Rectangular template on a textured button",
                Button::new("fidelity-template-button", "Action")
                    .style(ButtonStyle::Textured)
                    .icon(
                        silvestris::icons::TemplateImage::Action
                            .button_icon_renderer(silvestris::icons::TemplateImageSlot::Toolbar),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "File artwork: active, inactive and disabled",
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .children(
                        [
                            silvestris::icons::FileIconState::normal(),
                            silvestris::icons::FileIconState::selected(),
                            silvestris::icons::FileIconState {
                                inactive: true,
                                ..silvestris::icons::FileIconState::normal()
                            },
                            silvestris::icons::FileIconState {
                                disabled: true,
                                ..silvestris::icons::FileIconState::normal()
                            },
                        ]
                        .into_iter()
                        .flat_map(|state| {
                            [
                                silvestris::icons::FileArtwork::Folder.file_element(16.0, state),
                                silvestris::icons::FileArtwork::Folder.file_element(32.0, state),
                            ]
                        }),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Consumer pictorial artwork: 32 px toolbar/category slots",
                {
                    let artwork = silvestris::icons::ConsumerArtwork::new(
                        "Example application artwork",
                        |_, _| {
                            silvestris::icons::FileArtwork::Application
                                .file_element(32.0, silvestris::icons::FileIconState::normal())
                        },
                    );
                    div()
                        .flex()
                        .gap(px(8.))
                        .child(artwork.toolbar_element(window, cx))
                        .child(artwork.category_element(window, cx))
                        .into_any_element()
                },
            ))
            .child(cell(
                "Small",
                SegmentedControl::new(
                    "fidelity-segments-small",
                    "Density",
                    vec![
                        Segment::new("compact", "Compact"),
                        Segment::new("full", "Full"),
                    ],
                )
                .size(ControlSize::Small)
                .selected("compact")
                .into_any_element(),
            ))
            .child(cell(
                "Textured, momentary",
                SegmentedControl::new(
                    "fidelity-segments-momentary",
                    "History",
                    vec![
                        Segment::new("back", "\u{2039}"),
                        Segment::new("forward", "\u{203a}"),
                    ],
                )
                .mode(SegmentMode::Momentary)
                .style(SegmentStyle::Textured)
                .into_any_element(),
            ))
            .child(cell(
                "Select any",
                SegmentedControl::new(
                    "fidelity-segments-any",
                    "Columns",
                    vec![Segment::new("name", "Name"), Segment::new("size", "Size")],
                )
                .mode(SegmentMode::SelectAny)
                .selected_many(["name"])
                .into_any_element(),
            ));
        let progress = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(32.))
            .child(cell(
                "Determinate",
                div()
                    .w(px(200.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        ProgressIndicator::new("fidelity-progress-bar", "Copying", Some(0.55))
                            .style(ProgressStyle::Bar),
                    )
                    .child(ProgressIndicator::new(
                        "percent",
                        "Export Progress",
                        Some(self.progress),
                    ))
                    .into_any_element(),
            ))
            .child(cell(
                "Indeterminate",
                div()
                    .w(px(200.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        ProgressIndicator::new("fidelity-progress-pole", "Preparing", None)
                            .style(ProgressStyle::Bar)
                            .animate(false),
                    )
                    .child(ProgressIndicator::new("busy", "Preparing Export", None))
                    .into_any_element(),
            ))
            .child(cell(
                "Spinner",
                ProgressIndicator::spinner("fidelity-spinner", "Working")
                    .style(ProgressStyle::Spinner)
                    .into_any_element(),
            ))
            .child(cell(
                "Ring",
                ProgressIndicator::circular("fidelity-ring", "Ring", 0.7)
                    .style(ProgressStyle::Circular)
                    .into_any_element(),
            ));
        let containers = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(24.))
            .child(cell(
                "Group box",
                div()
                    .w(px(260.))
                    .child(
                        GroupBox::new("fidelity-group").title("Group Box").child(
                            div()
                                .text_size(small_text)
                                .child("Grouped content with a measured border."),
                        ),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Tab view",
                div()
                    .w(px(300.))
                    .h(px(130.))
                    .flex()
                    .flex_col()
                    .child(
                        TabView::new("fidelity-tabs")
                            .tabs(
                                vec![
                                    Segment::new("date", "Date & Time"),
                                    Segment::new("zone", "Time Zone"),
                                    Segment::new("clock", "Clock"),
                                ],
                                self.selected_tab.clone(),
                            )
                            .on_select(cx.listener(|g, tab: &gpui::SharedString, _, cx| {
                                g.selected_tab = tab.clone();
                                cx.notify();
                            }))
                            .child(div().text_size(small_text).child(
                                match self.selected_tab.as_ref() {
                                    "zone" => "Time Zone pane",
                                    "clock" => "Clock pane",
                                    _ => "Date & Time pane",
                                },
                            )),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Well and separators",
                div()
                    .w(px(260.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        Well::new("fidelity-well")
                            .child(div().text_size(small_text).child("Recessed well")),
                    )
                    .child(Separator::horizontal().inset(8.0))
                    .into_any_element(),
            ))
            .child(cell(
                "Scroll view: both bars and the corner",
                div()
                    .w(px(220.))
                    .h(px(96.))
                    .border_1()
                    .border_color(silvestris::convert::hsla(theme.colors.group_border))
                    .child(
                        ScrollView::new("fidelity-scroll-view", &scroll)
                            .axes(ScrollAxes::Both)
                            .child(
                                div()
                                    .id("fidelity-scroll-content")
                                    .size_full()
                                    .overflow_scroll()
                                    .track_scroll(&scroll)
                                    .child(
                                        div()
                                            .w(px(420.))
                                            .h(px(300.))
                                            .p(px(8.))
                                            .text_size(small_text)
                                            .child("Content wider and taller than the view."),
                                    ),
                            ),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Table view: 2,000 rows; sort, resize, reorder, type-select, Return to rename",
                div()
                    .w(px(300.))
                    .h(px(140.))
                    .border_1()
                    .border_color(silvestris::convert::hsla(theme.colors.group_border))
                    .child(self.files_table.clone())
                    .into_any_element(),
            ))
            .child(cell(
                "Column browser: branch arrows, grey parent selection, blue focused column",
                div()
                    .w(px(300.))
                    .h(px(140.))
                    .border_1()
                    .border_color(silvestris::convert::hsla(theme.colors.group_border))
                    .child(self.browser.clone())
                    .into_any_element(),
            ))
            .child(cell(
                "File panels: an Untitled document with File > Save As (sheet) and File > Open",
                {
                    let state = self.panel_state.clone();
                    let frames = self.panel_frames.clone();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            Button::new("gallery-document", "Open Untitled document")
                                .size(ControlSize::Small)
                                .on_click(move |_, _, cx| {
                                    let bounds =
                                        Bounds::centered(None, size(px(560.), px(420.)), cx);
                                    let options = silvestris::window_role::role_options(
                                        WindowRole::Document,
                                        "Untitled",
                                        bounds,
                                        cx,
                                    );
                                    let state = state.clone();
                                    let frames = frames.clone();
                                    let _ = open_window(options, cx, move |_, cx| {
                                        cx.new(|_| DocumentDemo::new(state, frames))
                                    });
                                }),
                        )
                        .into_any_element()
                },
            ))
            .child(cell(
                "Small scroll view; Appearance scroller preferences",
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .w(px(120.))
                            .h(px(80.))
                            .border_1()
                            .border_color(silvestris::convert::hsla(theme.colors.group_border))
                            .child(
                                ScrollView::new("fidelity-scroll-small", &scroll_small)
                                    .size(ControlSize::Small)
                                    .child(
                                        div()
                                            .id("fidelity-scroll-small-content")
                                            .size_full()
                                            .overflow_y_scroll()
                                            .track_scroll(&scroll_small)
                                            .child(
                                                div()
                                                    .h(px(240.))
                                                    .p(px(6.))
                                                    .text_size(small_text)
                                                    .child("Small bar for small controls."),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                Checkbox::new("fidelity-arrows-at-ends", "Arrows at both ends")
                                    .state(CheckState::from(
                                        scroller_prefs.arrows == ScrollArrows::AtEnds,
                                    ))
                                    .on_change(move |state, _, cx| {
                                        let arrows = if *state == CheckState::On {
                                            ScrollArrows::AtEnds
                                        } else {
                                            ScrollArrows::Together
                                        };
                                        ScrollerPreferences {
                                            arrows,
                                            ..ScrollerPreferences::get(cx)
                                        }
                                        .set(cx);
                                    }),
                            )
                            .child(
                                Checkbox::new("fidelity-jump-to-spot", "Jump to the spot clicked")
                                    .state(CheckState::from(
                                        scroller_prefs.track_click == TrackClick::JumpToSpot,
                                    ))
                                    .on_change(move |state, _, cx| {
                                        let track_click = if *state == CheckState::On {
                                            TrackClick::JumpToSpot
                                        } else {
                                            TrackClick::JumpToPage
                                        };
                                        ScrollerPreferences {
                                            track_click,
                                            ..ScrollerPreferences::get(cx)
                                        }
                                        .set(cx);
                                    }),
                            ),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Steppers",
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(Stepper::new("fidelity-stepper", "Copies").limits(true, false))
                    .child(
                        Stepper::new("fidelity-stepper-bounded", "At limit").limits(false, false),
                    )
                    .child(
                        Separator::new(SeparatorAxis::Vertical)
                            .length(40.0)
                            .inset(4.0),
                    )
                    .into_any_element(),
            ))
            .child(cell("Disclosure triangle", disclosure.into_any_element()));
        let notices = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(24.))
            .child(cell(
                "Empty state",
                div()
                    .w(px(260.))
                    .child(
                        Notice::new("No items", "Drop a file here or choose File > Open.")
                            .failure(false),
                    )
                    .into_any_element(),
            ))
            .child(cell(
                "Failure",
                div()
                    .w(px(260.))
                    .child(
                        Notice::new("Import failed", "The file could not be read.").failure(true),
                    )
                    .into_any_element(),
            ));
        let sidebar = silvestris::sidebar::Sidebar::new(
            "catalog-sidebar",
            self.sidebar,
            self.source_list.clone(),
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .pt(px(8.))
                .child(
                    Button::new("rail-docs", "Docs")
                        .size(ControlSize::Mini)
                        .on_click(cx.listener(|g, _, _, cx| {
                            g.source_list
                                .update(cx, |list, cx| list.select("collection", cx));
                            g.last_source = "Collection selected from rail".into();
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("rail-archive", "Arc")
                        .size(ControlSize::Mini)
                        .on_click(cx.listener(|g, _, _, cx| {
                            g.source_list
                                .update(cx, |list, cx| list.select("archive", cx));
                            g.last_source = "Archive selected from rail".into();
                            cx.notify();
                        })),
                ),
        )
        .footer(
            Button::new(
                "toggle-sidebar",
                if self.sidebar.expanded {
                    "Hide"
                } else {
                    "Show"
                },
            )
            .size(ControlSize::Mini)
            .on_click(move |_, window, cx| {
                CommandRegistry::invoke(&footer_commands, &"view.sidebar".into(), window, cx);
            }),
        )
        .on_change(cx.listener(|g, preference, _, cx| {
            g.sidebar = *preference;
            g.sync_sidebar_command(cx);
            cx.notify();
        }));
        let materials = canvas(
            |_, _, _| (),
            |bounds, _, window, cx| {
                let t = cx.theme().clone();
                let m = &t.metrics;
                let o = bounds.origin;
                let at = |x: f32, y: f32, w: f32, h: f32| {
                    Bounds::new(point(o.x + px(x), o.y + px(y)), size(px(w), px(h)))
                };
                let c = &t.controls;
                paint::bevel(
                    window,
                    at(0., 0., 90., m.push_height),
                    m.push_radius,
                    &c.push.normal,
                );
                paint::bevel(
                    window,
                    at(110., 0., 90., m.push_height),
                    m.push_radius,
                    &c.push_default,
                );
                paint::bevel(
                    window,
                    at(220., 0., 90., m.push_height),
                    m.push_radius,
                    &c.push.disabled,
                );
                paint::bevel(
                    window,
                    at(0., 40., 40., m.textured_height),
                    m.textured_radius,
                    &c.textured.normal,
                );
                paint::bevel(
                    window,
                    at(50., 40., 40., m.textured_height),
                    m.textured_radius,
                    &c.textured.selected,
                );
                paint::bevel(
                    window,
                    at(110., 40., 80., m.segment_height),
                    m.push_radius,
                    &c.segment.normal,
                );
                paint::bevel(
                    window,
                    at(190., 40., 80., m.segment_height),
                    m.push_radius,
                    &c.segment.selected,
                );
                paint::bevel(
                    window,
                    at(0., 80., m.checkbox_size, m.checkbox_size),
                    m.checkbox_radius,
                    &c.checkbox.normal,
                );
                paint::bevel(
                    window,
                    at(30., 80., m.checkbox_size, m.checkbox_size),
                    m.checkbox_radius,
                    &c.checkbox.selected,
                );
                let f = at(0., 120., 200., m.field_height);
                paint::field(window, f, 0.0, &c.field, true);
                let s = at(220., 120., 160., m.field_height);
                paint::field(window, s, m.field_height / 2.0, &c.search, true);
                paint::focus_ring(
                    window,
                    s,
                    m.field_height / 2.0,
                    t.colors.focus_ring,
                    m.focus_ring,
                );
                paint::selection(
                    window,
                    at(0., 170., 200., m.source_row_height),
                    &t.lists.source_active,
                );
                paint::selection(
                    window,
                    at(0., 200., 200., m.table_row_height),
                    &t.lists.table_active,
                );
                paint::selection(
                    window,
                    at(0., 230., 200., m.menu_item_height),
                    &t.lists.menu,
                );
            },
        );
        let modal_sheet = self.modal.as_ref().and_then(|demo| {
            if !demo.session.read(cx).is_active() {
                return None;
            }
            let session = demo.session.clone();
            Some(
                ConfirmationSheet::new(
                    "Apply changes?",
                    "This sheet is a document-modal session. Save is blocked in this window; \
                     the button and Escape end it.",
                    "Apply",
                    demo.focus.clone(),
                    move |accept, window, cx| {
                        modal::finish(
                            session.clone(),
                            if accept {
                                ModalResult::Accepted
                            } else {
                                ModalResult::Rejected
                            },
                            window,
                            cx,
                        );
                    },
                )
                .into_any_element(),
            )
        });
        let toolbar_view = cx.entity();
        let toolbar = Toolbar::new(
            "gallery-toolbar",
            "Gallery",
            self.commands.clone(),
            vec![
                ToolbarItem::group(
                    "toolbar-window-controls",
                    "Window",
                    vec![
                        ToolbarItem::toggle("toolbar-sidebar", "Sidebar", "view.sidebar")
                            .image(silvestris::icons::TemplateImage::Sidebar)
                            .toggle_help("Hide the sidebar", "Show the sidebar"),
                        ToolbarItem::segmented(
                            "toolbar-view",
                            "View",
                            vec![
                                ToolbarSegment::new("grid", "Icon View")
                                    .image(silvestris::icons::TemplateImage::IconView),
                                ToolbarSegment::new("list", "List View")
                                    .image(silvestris::icons::TemplateImage::ListView),
                                ToolbarSegment::new("columns", "Column View")
                                    .image(silvestris::icons::TemplateImage::ColumnView),
                            ],
                            Some(self.selected_view.clone()),
                            move |selected, _, cx| {
                                toolbar_view.update(cx, |gallery, cx| {
                                    gallery.selected_view = selected.clone();
                                    cx.notify();
                                });
                            },
                        )
                        .priority(160),
                        ToolbarItem::toggle(
                            "toolbar-small-bar",
                            "Small Bar",
                            "gallery.bottom-small",
                        )
                        .toggle_help("Use the small bottom bar", "Use the regular bottom bar")
                        .priority(200),
                    ],
                ),
                ToolbarItem::pop_down(
                    "toolbar-action",
                    "Action",
                    MenuModel {
                        label: "Action".into(),
                        entries: vec![
                            MenuEntry::Command("view.sidebar".into()),
                            MenuEntry::Command("view.badges".into()),
                            MenuEntry::Separator,
                            MenuEntry::Submenu {
                                label: "Appearance".into(),
                                entries: [
                                    "theme.aqua-light",
                                    "theme.aqua-dark",
                                    "theme.geist-light",
                                    "theme.geist-dark",
                                ]
                                .into_iter()
                                .map(|id| MenuEntry::Command(id.into()))
                                .collect(),
                            },
                        ],
                    },
                )
                .image(silvestris::icons::TemplateImage::Action)
                .style(ToolbarItemStyle::Capsule)
                .priority(100),
                ToolbarItem::flexible_space("toolbar-search-gap"),
                ToolbarItem::search(
                    "toolbar-search",
                    "Search Gallery",
                    self.toolbar_search.clone(),
                )
                .priority(255),
            ],
        )
        .chain(self.routing.clone())
        .display_mode(ToolbarDisplayMode::IconAndLabel);
        let bottom_bar = BottomBar::new(
            "gallery-bottom-bar",
            "Gallery status",
            vec![
                BottomBarItem::status(
                    BottomBarStatus::new("gallery-bottom-selection", "3 of 2,000 items selected")
                        .help("Selection status; status values may expose details"),
                ),
                BottomBarItem::status(BottomBarStatus::new("gallery-bottom-state", "Ready")),
                BottomBarItem::flexible_space("gallery-bottom-gap"),
                BottomBarItem::command("gallery-bottom-badges", "Row Badges", "view.badges")
                    .image(silvestris::icons::TemplateImage::ListView),
                BottomBarItem::command("gallery-bottom-add", "Add", "gallery.add")
                    .image(silvestris::icons::TemplateImage::Add)
                    .style(BottomBarControlStyle::Gradient),
                BottomBarItem::command("gallery-bottom-remove", "Remove", "gallery.remove")
                    .image(silvestris::icons::TemplateImage::Remove)
                    .style(BottomBarControlStyle::Gradient),
                BottomBarItem::command("gallery-bottom-action", "Action", "gallery.action")
                    .image(silvestris::icons::TemplateImage::Action)
                    .style(BottomBarControlStyle::Gradient),
            ],
        )
        .registry(self.commands.clone())
        .chain(self.routing.clone())
        .size(if self.bottom_bar_small {
            BottomBarSize::Small
        } else {
            BottomBarSize::Regular
        });

        div()
            .size_full()
            .key_context("Gallery")
            .on_action(move |action: &InvokeCommand, window, cx| {
                cx.stop_propagation();
                let chain = chain.read(cx).clone();
                chain.dispatch(&commands, &CommandId::new(&action.id), window, cx);
            })
            .track_focus(&self.focus)
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            .child(
                WindowFrame::new(format!("Silvestris Gallery — {label}"))
                    .toolbar_component(toolbar, cx)
                    .bottom_bar_component(bottom_bar, cx)
                    .child(
                        div()
                            // The frame's content slot is a block box, so fill it
                            // explicitly; flex_1 would let the scroller grow to
                            // the content height and never overflow.
                            .size_full()
                            .flex()
                            .child(
                                div()
                                    .id("gallery-scroll")
                                    .flex_1()
                                    .min_w_0()
                                    .h_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.gallery_scroll)
                            .child(
                                div()
                                    .w_full()
                                    .flex_none()
                                    .p(px(20.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(28.))
                                    .child(section(
                                        "Push buttons",
                                        "HIG pp. 265–268",
                                        &theme,
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(buttons)
                                            .child(push_styles),
                                    ))
                                    .child(section(
                                        "Pop-ups and combo boxes",
                                        "HIG pp. 279–284",
                                        &theme,
                                        popups,
                                    ))
                                    .child(section(
                                        "Checkboxes and radio buttons",
                                        "HIG pp. 272–276",
                                        &theme,
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(16.))
                                            .child(checks)
                                            .child(radios),
                                    ))
                                    .child(section("Sliders", "HIG pp. 286–288", &theme, sliders))
                                    .child(section(
                                        "Segmented controls",
                                        "HIG pp. 291–293",
                                        &theme,
                                        segments,
                                    ))
                                    .child(section("Progress", "HIG pp. 305–309", &theme, progress))
                                    .child(section(
                                        "Text",
                                        "fields, search, composer",
                                        &theme,
                                        fields,
                                    ))
                                    .child(section(
                                        "Containers",
                                        "HIG pp. 361–366",
                                        &theme,
                                        containers,
                                    ))
                                    .child(section(
                                        "Notices",
                                        "empty and failure states",
                                        &theme,
                                        notices,
                                    ))
                                    .child(section(
                                        "Source lists and sidebar",
                                        self.last_source.clone(),
                                        &theme,
                                        div().h(px(280.)).flex().gap(px(24.)).child(sidebar).child(
                                            div()
                                                .w(px(220.))
                                                .h_full()
                                                .child(self.secondary_list.clone()),
                                        ),
                                    ))
                                    .child(section(
                                        "Behaviour",
                                        "commands, focus, transient surfaces, bindings, windows",
                                        &theme,
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(14.))
                                            .child(routing_row)
                                            .child(transient_row)
                                            .child(shortcut_row)
                                            .child(binding_row)
                                            .child(list_row)
                                            .child(role_row),
                                    ))
                                    .child(section(
                                        "Materials",
                                        "raw bevels, fields and selections",
                                        &theme,
                                        div().h(px(260.)).flex().child(materials.flex_1()),
                                    )),
                            ))
            .child(Scrollbar::new("gallery-scrollbar", &self.gallery_scroll)),
                    ),
            )
            .child(gpui::deferred(self.menu.clone()).priority(1))
            .children(modal_sheet)
    }
}

fn set(id: ThemeId, cx: &mut App) {
    silvestris::set_theme_preference(ThemePreference::explicit(id), cx);
}

/// The gallery's command binding context. Text clients need no exclusion here:
/// `CommandRegistry::bindings` scopes typing keys away from them (#57).
const GALLERY_KEYS: &str = "Gallery";

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        if let Err(e) = silvestris::init(ThemePreference::default(), cx) {
            eprintln!("silvestris: {e:#}");
            cx.quit();
            return;
        }
        cx.bind_keys([
            KeyBinding::new("tab", FocusNext, None),
            KeyBinding::new("shift-tab", FocusPrev, None),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(900.), px(600.)), cx);
        let options = window_options("Silvestris Gallery", bounds, size(px(480.), px(320.)), cx);
        open_window(options, cx, |window, cx| {
            let view = cx.new(|cx| Gallery::new(window, cx));
            let focus = view.read(cx).focus.clone();
            window.focus(&focus, cx);
            view
        })
        .expect("open gallery window");
        cx.activate(true);
    });
}

/// A titled gallery section: bold heading, a secondary note (HIG page or
/// purpose), a separator, then the samples.
fn section(
    title: &'static str,
    note: impl Into<gpui::SharedString>,
    theme: &silvestris::core::Theme,
    body: impl IntoElement,
) -> gpui::Div {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(
            div()
                .flex()
                .items_end()
                .gap(px(10.))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(silvestris::convert::hsla(theme.colors.text))
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(theme.type_.size_small))
                        .text_color(silvestris::convert::hsla(theme.colors.text_secondary))
                        .child(note.into()),
                ),
        )
        .child(Separator::horizontal())
        .child(body)
}
