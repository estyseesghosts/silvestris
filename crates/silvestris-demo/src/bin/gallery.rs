//! Component catalog (#135). Keys 1-4 switch Aqua / Dark Aqua / Geist Light
//! / Geist Dark while no text field is focused.

use gpui::{
    actions, canvas, div, point, prelude::*, px, size, App, Bounds, Context, Entity, KeyBinding,
    Window,
};
use silvestris::chrome::{open_window, window_options, WindowFrame};
use silvestris::command::{CommandButton, CommandRegistry, InvokeCommand};
use silvestris::controls::{
    Button, ButtonStyle, ControlSize, Disclosure, Segment, SegmentedControl, SourceList,
    SourceListEvent, TextField, TextFieldEvent,
};
use silvestris::core::command::{CommandDescriptor, CommandId};
use silvestris::{paint, ActiveTheme, ThemeId, ThemePreference};

actions!(
    gallery,
    [AquaLight, AquaDark, GeistLight, GeistDark, FocusNext, FocusPrev]
);

struct Gallery {
    commands: Entity<CommandRegistry>,
    selected_view: gpui::SharedString,
    sidebar: silvestris::core::layout::SidebarPreference,
    source_list: Entity<SourceList>,
    last_source: gpui::SharedString,
    details_expanded: bool,
    focus: gpui::FocusHandle,
    last_click: gpui::SharedString,
    fields: Vec<Entity<TextField>>,
    last_field: gpui::SharedString,
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
        Gallery {
            commands,
            selected_view: "grid".into(),
            sidebar: silvestris::core::layout::SidebarPreference {
                expanded: true,
                width: 240.,
            },
            source_list,
            last_source: "No source-list event".into(),
            details_expanded: false,
            focus: cx.focus_handle(),
            last_click: "none".into(),
            fields: vec![name, password, small, disabled],
            last_field: "none".into(),
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
            );
        div()
            .size_full()
            .key_context("Gallery")
            .on_action(move |action: &InvokeCommand, window, cx| {
                cx.stop_propagation();
                CommandRegistry::invoke(&commands, &CommandId::new(&action.id), window, cx);
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
                        CommandButton::new(
                            "sidebar-toolbar",
                            "view.sidebar",
                            self.commands.clone(),
                        ),
                        theme.metrics.toolbar_height,
                    )
                    .bottom_bar(div())
                    .child(
                        div()
                            .size_full()
                            .p(px(20.))
                            .flex()
                            .flex_col()
                            .gap(px(20.))
                            .child(buttons)
                            .child(fields)
                            .child(disclosure)
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
                                .on_select(cx.listener(
                                    |g, selected: &gpui::SharedString, _, cx| {
                                        g.selected_view = selected.clone();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(div().text_size(small_text).child(self.last_source.clone()))
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
                                                        .on_click(cx.listener(|g, _, _, cx| {
                                                            g.source_list.update(cx, |list, cx| {
                                                                list.select("collection", cx)
                                                            });
                                                            g.last_source =
                                                                "Collection selected from rail"
                                                                    .into();
                                                            cx.notify();
                                                        })),
                                                )
                                                .child(
                                                    Button::new("rail-archive", "Arc")
                                                        .size(ControlSize::Mini)
                                                        .on_click(cx.listener(|g, _, _, cx| {
                                                            g.source_list.update(cx, |list, cx| {
                                                                list.select("archive", cx)
                                                            });
                                                            g.last_source =
                                                                "Archive selected from rail".into();
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
                                            .on_click(
                                                move |_, window, cx| {
                                                    CommandRegistry::invoke(
                                                        &footer_commands,
                                                        &"view.sidebar".into(),
                                                        window,
                                                        cx,
                                                    );
                                                },
                                            ),
                                        )
                                        .on_change(
                                            cx.listener(|g, preference, _, cx| {
                                                g.sidebar = *preference;
                                                g.sync_sidebar_command(cx);
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(
                                        canvas(
                                            |_, _, _| (),
                                            |bounds, _, window, cx| {
                                                let t = cx.theme().clone();
                                                let m = &t.metrics;
                                                let o = bounds.origin;
                                                let at = |x: f32, y: f32, w: f32, h: f32| {
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
            )
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
