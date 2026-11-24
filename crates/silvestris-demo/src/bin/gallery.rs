//! Component catalog (#135). Keys 1-4 switch Aqua / Dark Aqua / Geist Light
//! / Geist Dark while no text field is focused.

use gpui::{
    actions, canvas, div, point, prelude::*, px, size, App, Bounds, Context, Entity, KeyBinding,
    Window,
};
use silvestris::binding::{Binding, ListController};
use silvestris::chrome::{open_window, window_options, WindowFrame};
use silvestris::command::{self, CommandButton, CommandRegistry, InvokeCommand};
use silvestris::controls::{
    Button, ButtonStyle, CheckState, Checkbox, ConfirmationFocus, ConfirmationSheet, ControlSize,
    Disclosure, DisclosureStyle, GroupBox, Notice, Orientation, PopUpButton, PopupStyle,
    ProgressIndicator, ProgressStyle, RadioGroup, RadioOption, RowAccessory, RowStatus, Scrollbar,
    Segment, SegmentMode, SegmentStyle, SegmentedControl, Separator, SeparatorAxis,
    ShortcutRecorder, Slider, SourceList, SourceListEvent, SourceListStyle, Stepper, TextArea,
    TextAreaEvent, TextField, TextFieldEvent, ThumbStyle, Well,
};
use silvestris::core::command::{CommandDescriptor, CommandId, MenuEntry, MenuModel};
use silvestris::menu::PopupMenu;
use silvestris::modal::{self, ModalResult, ModalScope, ModalSession};
use silvestris::responder::{
    Responder, ResponderChain, ResponderEntry, ResponderScope, ResponderState,
};
use silvestris::transient::TransientLayer;
use silvestris::window_role::{open_role, WindowRole};
use silvestris::{paint, ActiveTheme, ThemeId, ThemePreference};

actions!(
    gallery,
    [AquaLight, AquaDark, GeistLight, GeistDark, FocusNext, FocusPrev]
);

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

struct Gallery {
    menu: Entity<PopupMenu>,
    commands: Entity<CommandRegistry>,
    selected_view: gpui::SharedString,
    sidebar: silvestris::core::layout::SidebarPreference,
    source_list: Entity<SourceList>,
    secondary_list: Entity<SourceList>,
    gallery_scroll: gpui::ScrollHandle,
    last_source: gpui::SharedString,
    details_expanded: bool,
    checks: [CheckState; 3],
    focus: gpui::FocusHandle,
    last_click: gpui::SharedString,
    fields: Vec<Entity<TextField>>,
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
        let bindings = commands
            .read(cx)
            .bindings("Gallery && !SilvestrisTextField", cx)
            .expect("gallery command bindings");
        cx.bind_keys(bindings);
        cx.observe(&commands, |_, _, cx| cx.notify()).detach();
        commands.update(cx, |registry, cx| {
            for (id, label, theme) in [
                ("theme.aqua-light", "Aqua Light", ThemeId::AQUA_LIGHT),
                ("theme.aqua-dark", "Aqua Dark", ThemeId::AQUA_DARK),
                ("theme.geist-light", "Geist Light", ThemeId::GEIST_LIGHT),
                ("theme.geist-dark", "Geist Dark", ThemeId::GEIST_DARK),
            ] {
                registry
                    .register(
                        CommandDescriptor::action(id, label),
                        move |_, _, cx| set(theme, cx),
                        cx,
                    )
                    .unwrap();
            }
        });
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
            sidebar: silvestris::core::layout::SidebarPreference {
                expanded: true,
                width: 240.,
            },
            source_list,
            secondary_list,
            gallery_scroll: gpui::ScrollHandle::new(),
            last_source: "No source-list event".into(),
            details_expanded: false,
            focus: cx.focus_handle(),
            checks: [CheckState::Off, CheckState::On, CheckState::Mixed],
            last_click: "none".into(),
            fields: vec![name, password, small, disabled, search],
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
        }
    }

    fn sync_sidebar_command(&mut self, cx: &mut Context<Self>) {
        let descriptor =
            CommandDescriptor::toggle("view.sidebar", "Sidebar", self.sidebar.expanded)
                .shortcut("secondary-b");
        self.commands
            .update(cx, |registry, cx| registry.publish(descriptor, cx).unwrap());
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
        let scroll = gpui::ScrollHandle::new();
        let fidelity_row = div()
            .flex()
            .flex_wrap()
            .items_start()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                PopUpButton::new(
                                    "fidelity-popup",
                                    "Appearance",
                                    vec!["Blue".into(), "Graphite".into()],
                                )
                                .selected(Some(0))
                                .size(ControlSize::Regular),
                            )
                            .child(
                                PopUpButton::new(
                                    "fidelity-popup-textured",
                                    "Scope",
                                    vec!["Everything".into(), "Selection".into()],
                                )
                                .selected(Some(0))
                                .size(ControlSize::Regular)
                                .bezel(PopupStyle::Textured),
                            )
                            .child(
                                PopUpButton::new(
                                    "fidelity-popup-disabled",
                                    "Unavailable",
                                    vec!["Nothing".into()],
                                )
                                .disabled(true),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(Button::new("fidelity-help", "Help").style(ButtonStyle::Help))
                            .child(
                                Button::new("fidelity-round", "Round")
                                    .style(ButtonStyle::Round)
                                    .icon(|ink| div().text_color(ink).child("\u{2699}")),
                            )
                            .child(
                                Button::new("fidelity-recessed", "Recessed")
                                    .style(ButtonStyle::Recessed),
                            )
                            .child(Button::new("fidelity-bevel", "Bevel").style(ButtonStyle::Bevel))
                            .child(
                                Disclosure::new("fidelity-disclosure-button", "Disclosure button")
                                    .style(DisclosureStyle::Button)
                                    .expanded(true),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(240.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .child(
                                RadioGroup::new(
                                    "fidelity-radio-regular",
                                    "Regular radios",
                                    vec![RadioOption::new("First"), RadioOption::new("Second")],
                                )
                                .selected(Some(0)),
                            )
                            .child(
                                RadioGroup::new(
                                    "fidelity-radio-mini",
                                    "Mini radios",
                                    vec![RadioOption::new("On"), RadioOption::new("Off")],
                                )
                                .size(ControlSize::Mini)
                                .selected(Some(1)),
                            ),
                    )
                    .child(
                        Slider::new("fidelity-slider-ticks", "Dock size", 0.6)
                            .thumb(ThumbStyle::Pointed)
                            .ticks([0.0, 1.0])
                            .tick_labels(["Small", "Large"])
                            .snap_to_ticks(true),
                    )
                    .child(
                        div().flex().items_center().gap(px(12.)).child(
                            div().h(px(90.)).child(
                                Slider::new("fidelity-slider-vertical", "Level", 0.35)
                                    .orientation(Orientation::Vertical)
                                    .thumb(ThumbStyle::Pointed)
                                    .ticks([0.0, 0.5, 1.0]),
                            ),
                        ),
                    ),
            )
            .child(
                div()
                    .w(px(260.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        SegmentedControl::new(
                            "fidelity-segments-icon",
                            "View",
                            vec![
                                Segment::new("grid", "Grid").icon("grid").show_label(false),
                                Segment::new("list", "List").icon("list").show_label(false),
                                Segment::new("columns", "Columns")
                                    .icon("columns")
                                    .show_label(false)
                                    .disabled(true),
                            ],
                        )
                        .selected("grid")
                        .icon_renderer(|role, ink| {
                            div()
                                .text_color(silvestris::convert::hsla(ink))
                                .child(match role {
                                    "grid" => "\u{25a6}",
                                    "list" => "\u{2630}",
                                    _ => "\u{25a4}",
                                })
                                .into_any_element()
                        }),
                    )
                    .child(
                        SegmentedControl::new(
                            "fidelity-segments-small",
                            "Density",
                            vec![
                                Segment::new("compact", "Compact"),
                                Segment::new("full", "Full"),
                            ],
                        )
                        .size(ControlSize::Small)
                        .selected("compact"),
                    )
                    .child(
                        SegmentedControl::new(
                            "fidelity-segments-momentary",
                            "History",
                            vec![
                                Segment::new("back", "\u{2039}"),
                                Segment::new("forward", "\u{203a}"),
                            ],
                        )
                        .mode(SegmentMode::Momentary)
                        .style(SegmentStyle::Textured),
                    )
                    .child(
                        SegmentedControl::new(
                            "fidelity-segments-any",
                            "Columns",
                            vec![Segment::new("name", "Name"), Segment::new("size", "Size")],
                        )
                        .mode(SegmentMode::SelectAny)
                        .selected_many(["name"]),
                    ),
            )
            .child(
                div()
                    .w(px(240.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        ProgressIndicator::new("fidelity-progress-bar", "Copying", Some(0.55))
                            .style(ProgressStyle::Bar),
                    )
                    .child(
                        ProgressIndicator::new("fidelity-progress-pole", "Preparing", None)
                            .style(ProgressStyle::Bar)
                            .animate(false),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                ProgressIndicator::spinner("fidelity-spinner", "Working")
                                    .style(ProgressStyle::Spinner),
                            )
                            .child(
                                ProgressIndicator::circular("fidelity-ring", "Ring", 0.7)
                                    .style(ProgressStyle::Circular),
                            ),
                    ),
            )
            .child(
                div()
                    .w(px(260.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        GroupBox::new("fidelity-group").title("Group Box").child(
                            div()
                                .text_size(px(theme.type_.size_small))
                                .child("Grouped content with a measured border."),
                        ),
                    )
                    .child(
                        Well::new("fidelity-well").child(
                            div()
                                .text_size(px(theme.type_.size_small))
                                .child("Recessed well"),
                        ),
                    )
                    .child(Separator::horizontal().inset(8.0)),
            )
            .child(
                div()
                    .w(px(220.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        Notice::new("No items", "Drop a file here or choose File > Open.")
                            .failure(false),
                    )
                    .child(
                        Notice::new("Import failed", "The file could not be read.").failure(true),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(Stepper::new("fidelity-stepper", "Copies").limits(true, false))
                            .child(
                                Stepper::new("fidelity-stepper-bounded", "At limit")
                                    .limits(false, false),
                            )
                            .child(
                                Separator::new(SeparatorAxis::Vertical)
                                    .length(40.0)
                                    .inset(4.0),
                            ),
                    )
                    .child(
                        div()
                            .w(px(220.))
                            .h(px(80.))
                            .flex()
                            .border_1()
                            .border_color(silvestris::convert::hsla(theme.colors.group_border))
                            .child(
                                div()
                                    .id("fidelity-scroll-content")
                                    .flex_1()
                                    .h_full()
                                    .overflow_y_scroll()
                                    .track_scroll(&scroll)
                                    .child(
                                        div()
                                            .h(px(300.))
                                            .p(px(8.))
                                            .text_size(px(theme.type_.size_small))
                                            .child("Scrollable content with a themed scroller."),
                                    ),
                            )
                            .child(Scrollbar::new("fidelity-scrollbar", &scroll)),
                    ),
            )
            .child(
                div()
                    .w(px(220.))
                    .h(px(170.))
                    .child(self.secondary_list.clone()),
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
                                    let contexts = ["Gallery && !SilvestrisTextField"];
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
            .child(div().w(px(180.)).child(bound_level))
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

        div()
            .size_full()
            .key_context("Gallery")
            .on_action(move |action: &InvokeCommand, window, cx| {
                cx.stop_propagation();
                let chain = chain.read(cx).clone();
                chain.dispatch(&commands, &CommandId::new(&action.id), window, cx);
            })
            .track_focus(&self.focus)
            .on_action(cx.listener(|_, _: &AquaLight, _, cx| set(ThemeId::AQUA_LIGHT, cx)))
            .on_action(cx.listener(|_, _: &AquaDark, _, cx| set(ThemeId::AQUA_DARK, cx)))
            .on_action(cx.listener(|_, _: &GeistLight, _, cx| set(ThemeId::GEIST_LIGHT, cx)))
            .on_action(cx.listener(|_, _: &GeistDark, _, cx| set(ThemeId::GEIST_DARK, cx)))
            .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
            .on_action(|_: &FocusPrev, window, cx| window.focus_prev(cx))
            .child(
                WindowFrame::new(format!("Silvestris Gallery — {label}"))
                    .toolbar(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(CommandButton::new(
                                "sidebar-toolbar",
                                "view.sidebar",
                                self.commands.clone(),
                            ))
                            .child(
                                Button::new("view-menu", "View")
                                    .style(ButtonStyle::Textured)
                                    .on_click(cx.listener(
                                        |g, event: &gpui::ClickEvent, window, cx| {
                                            let position =
                                                event.position() + point(px(0.), px(12.));
                                            g.menu.update(cx, |menu, cx| {
                                                menu.open(position, window, cx)
                                            });
                                        },
                                    )),
                            ),
                        theme.metrics.toolbar_height,
                    )
                    .bottom_bar(div())
                    .child(
                        div()
                            .id("gallery-scroll")
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.gallery_scroll)
                            .child(
                                div()
                                    .w_full()
                                    .flex_none()
                                    .p(px(20.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(20.))
                                    .child(buttons)
                                    .child(fidelity_row)
                                    .child(fields)
                                    .child(disclosure)
                                    .child(routing_row)
                                    .child(transient_row)
                                    .child(shortcut_row)
                                    .child(binding_row)
                                    .child(list_row)
                                    .child(role_row)
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(12.))
                                            .children(
                                                [
                                                    ControlSize::Regular,
                                                    ControlSize::Small,
                                                    ControlSize::Mini,
                                                ]
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
                                            ),
                                    )
                                    .child(
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
                                        .on_select(
                                            cx.listener(
                                                |g, selected: &gpui::SharedString, _, cx| {
                                                    g.selected_view = selected.clone();
                                                    cx.notify();
                                                },
                                            ),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(24.0))
                                            .child(
                                                RadioGroup::new(
                                                    "radio-group",
                                                    "Export scope",
                                                    vec![
                                                        RadioOption::new("Current Item"),
                                                        RadioOption::new("All Items"),
                                                        RadioOption::new("Selection")
                                                            .disabled(true),
                                                    ],
                                                )
                                                .selected(Some(self.radio))
                                                .on_select(cx.listener(|g, selected, _, cx| {
                                                    g.radio = *selected;
                                                    cx.notify();
                                                })),
                                            )
                                            .child(
                                                div().w(px(180.)).child(
                                                    silvestris::controls::ComboBox::new(
                                                        "locations",
                                                        "Location",
                                                        self.combo.clone(),
                                                        vec![
                                                            "Home".into(),
                                                            "Documents".into(),
                                                            "Desktop".into(),
                                                        ],
                                                    )
                                                    .layer(self.transient.clone()),
                                                ),
                                            )
                                            .child(
                                                div()
                                                    .w(px(180.))
                                                    .flex()
                                                    .flex_col()
                                                    .gap(px(8.0))
                                                    .child(
                                                        Slider::new(
                                                            "export-progress",
                                                            "Progress preview",
                                                            self.progress,
                                                        )
                                                        .on_change(cx.listener(
                                                            |g, value, _, cx| {
                                                                g.progress = *value;
                                                                cx.notify();
                                                            },
                                                        )),
                                                    )
                                                    .child(ProgressIndicator::new(
                                                        "percent",
                                                        "Export Progress",
                                                        Some(self.progress),
                                                    ))
                                                    .child(ProgressIndicator::new(
                                                        "busy",
                                                        "Preparing Export",
                                                        None,
                                                    )),
                                            ),
                                    )
                                    .child(
                                        div().text_size(small_text).child(self.last_source.clone()),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .flex_1()
                                            .min_h_0()
                                            .gap(px(20.))
                                            .child(
                                                silvestris::sidebar::Sidebar::new(
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
                                                                .on_click(cx.listener(
                                                                    |g, _, _, cx| {
                                                                        g.source_list.update(
                                                                            cx,
                                                                            |list, cx| {
                                                                                list.select(
                                                                                    "collection",
                                                                                    cx,
                                                                                )
                                                                            },
                                                                        );
                                                                        g.last_source =
                                                                "Collection selected from rail"
                                                                    .into();
                                                                        cx.notify();
                                                                    },
                                                                )),
                                                        )
                                                        .child(
                                                            Button::new("rail-archive", "Arc")
                                                                .size(ControlSize::Mini)
                                                                .on_click(cx.listener(
                                                                    |g, _, _, cx| {
                                                                        g.source_list.update(
                                                                            cx,
                                                                            |list, cx| {
                                                                                list.select(
                                                                                    "archive", cx,
                                                                                )
                                                                            },
                                                                        );
                                                                        g.last_source =
                                                                "Archive selected from rail".into();
                                                                        cx.notify();
                                                                    },
                                                                )),
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
                                                        CommandRegistry::invoke(
                                                            &footer_commands,
                                                            &"view.sidebar".into(),
                                                            window,
                                                            cx,
                                                        );
                                                    }),
                                                )
                                                .on_change(cx.listener(|g, preference, _, cx| {
                                                    g.sidebar = *preference;
                                                    g.sync_sidebar_command(cx);
                                                    cx.notify();
                                                })),
                                            )
                                            .child(
                                                canvas(
                                                    |_, _, _| (),
                                                    |bounds, _, window, cx| {
                                                        let t = cx.theme().clone();
                                                        let m = &t.metrics;
                                                        let o = bounds.origin;
                                                        let at =
                                                            |x: f32, y: f32, w: f32, h: f32| {
                                                                Bounds::new(
                                                                    point(o.x + px(x), o.y + px(y)),
                                                                    size(px(w), px(h)),
                                                                )
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
                                                            at(
                                                                0.,
                                                                80.,
                                                                m.checkbox_size,
                                                                m.checkbox_size,
                                                            ),
                                                            m.checkbox_radius,
                                                            &c.checkbox.normal,
                                                        );
                                                        paint::bevel(
                                                            window,
                                                            at(
                                                                30.,
                                                                80.,
                                                                m.checkbox_size,
                                                                m.checkbox_size,
                                                            ),
                                                            m.checkbox_radius,
                                                            &c.checkbox.selected,
                                                        );
                                                        let f = at(0., 120., 200., m.field_height);
                                                        paint::field(
                                                            window, f, 0.0, &c.field, true,
                                                        );
                                                        let s =
                                                            at(220., 120., 160., m.field_height);
                                                        paint::field(
                                                            window,
                                                            s,
                                                            m.field_height / 2.0,
                                                            &c.search,
                                                            true,
                                                        );
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
                                                )
                                                .flex_1(),
                                            ),
                                    ),
                            ),
                    ),
            )
            .child(gpui::deferred(self.menu.clone()).priority(1))
            .children(modal_sheet)
    }
}

fn set(id: ThemeId, cx: &mut App) {
    silvestris::set_theme_preference(ThemePreference::explicit(id), cx);
}

/// Bare digits switch themes only while no text field has focus; `!` checks
/// the whole context stack (gpui keymap/context.rs, `Not` arm of eval_inner).
const THEME_KEYS: Option<&str> = Some("Gallery && !SilvestrisTextField");

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        if let Err(e) = silvestris::init(ThemePreference::default(), cx) {
            eprintln!("silvestris: {e:#}");
            cx.quit();
            return;
        }
        cx.bind_keys([
            KeyBinding::new("1", AquaLight, THEME_KEYS),
            KeyBinding::new("2", AquaDark, THEME_KEYS),
            KeyBinding::new("3", GeistLight, THEME_KEYS),
            KeyBinding::new("4", GeistDark, THEME_KEYS),
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
