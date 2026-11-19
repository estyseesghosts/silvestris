//! Independent shell consumer (#135): a small document explorer.
//! Uses public Silvestris APIs only; no Lotus or emulator dependencies.
use gpui::{div, prelude::*, px, size, App, Bounds, Context, Entity, FocusHandle, Window};
use silvestris::{
    chrome::{open_window, window_options, WindowFrame},
    command::{CommandButton, CommandRegistry, CommandScope},
    controls::{Button, ButtonStyle, ControlSize, SourceList, SourceListEvent},
    core::{
        command::{CommandDescriptor, CommandId},
        layout::{InspectorPreference, SidebarPreference},
        source_list::SourceNode,
    },
    shell::Shell,
    ActiveTheme, ThemeId, ThemePreference,
};
gpui::actions!(explorer, [FocusNext, FocusPrevious]);

struct Explorer {
    sources: Entity<SourceList>,
    commands: Entity<CommandRegistry>,
    focus: FocusHandle,
    selected: String,
    sidebar: SidebarPreference,
    inspector: InspectorPreference,
}
impl Explorer {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let sources = cx.new(|cx| {
            SourceList::new(
                vec![SourceNode::section(
                    "documents",
                    "Documents",
                    vec![
                        SourceNode::new("notes", "Research Notes"),
                        SourceNode::new("sketch", "Interface Sketch"),
                        SourceNode::new("report", "Field Report"),
                    ],
                )],
                cx,
            )
            .unwrap()
        });
        sources.update(cx, |list, cx| list.select("notes", cx));
        cx.subscribe(&sources, |view, _, event: &SourceListEvent, cx| {
            if let SourceListEvent::Selected(id) = event {
                view.selected = id.clone();
                cx.notify();
            }
        })
        .detach();
        let commands = cx.new(|_| CommandRegistry::default());
        let sidebar = cx.listener(|view, _: &CommandId, _, cx| {
            view.sidebar.expanded = !view.sidebar.expanded;
            view.sync(cx);
            cx.notify();
        });
        let inspector = cx.listener(|view, _: &CommandId, _, cx| {
            view.inspector.visible = !view.inspector.visible;
            view.sync(cx);
            cx.notify();
        });
        commands.update(cx, |r, cx| {
            r.register(
                CommandDescriptor::toggle("view.sidebar", "Sidebar", true).shortcut("secondary-b"),
                sidebar,
                cx,
            )
            .unwrap();
            r.register(
                CommandDescriptor::toggle("view.inspector", "Inspector", false)
                    .shortcut("secondary-i"),
                inspector,
                cx,
            )
            .unwrap();
            for (id, key, theme) in [
                ("theme.aqua-light", "1", ThemeId::AQUA_LIGHT),
                ("theme.aqua-dark", "2", ThemeId::AQUA_DARK),
                ("theme.geist-light", "3", ThemeId::GEIST_LIGHT),
                ("theme.geist-dark", "4", ThemeId::GEIST_DARK),
            ] {
                r.register(
                    CommandDescriptor::action(id, theme.label()).shortcut(key),
                    move |_, _, cx| {
                        silvestris::set_theme_preference(ThemePreference::explicit(theme), cx);
                    },
                    cx,
                )
                .unwrap();
            }
        });
        let keys = commands
            .read(cx)
            .bindings("Explorer && !SilvestrisTextField", cx)
            .unwrap();
        cx.bind_keys(keys);
        cx.observe(&commands, |_, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        Self {
            sources,
            commands,
            focus,
            selected: "notes".into(),
            sidebar: SidebarPreference {
                expanded: true,
                width: 264.,
            },
            inspector: InspectorPreference {
                visible: false,
                pinned: false,
                width: 420.,
            },
        }
    }
    fn sync(&mut self, cx: &mut Context<Self>) {
        let sidebar = CommandDescriptor::toggle("view.sidebar", "Sidebar", self.sidebar.expanded)
            .shortcut("secondary-b");
        let inspector =
            CommandDescriptor::toggle("view.inspector", "Inspector", self.inspector.visible)
                .shortcut("secondary-i");
        self.commands.update(cx, |r, cx| {
            r.publish(sidebar, cx).unwrap();
            r.publish(inspector, cx).unwrap();
        });
    }
}
impl Render for Explorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let rail_commands = self.commands.clone();
        let sidebar_commands = self.commands.clone();
        CommandScope::new("explorer-commands", "Explorer", self.commands.clone()).child(
            div()
                .size_full()
                .track_focus(&self.focus)
                .on_action(|_: &FocusNext, window, cx| window.focus_next(cx))
                .on_action(|_: &FocusPrevious, window, cx| window.focus_prev(cx))
                .child(
                    WindowFrame::new("Document Explorer")
                        .toolbar(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(CommandButton::new(
                                    "sidebar",
                                    "view.sidebar",
                                    self.commands.clone(),
                                ))
                                .child(CommandButton::new(
                                    "inspector",
                                    "view.inspector",
                                    self.commands.clone(),
                                )),
                            theme.metrics.toolbar_height,
                        )
                        .bottom_bar(
                            div()
                                .px(px(12.))
                                .child(format!("Selected: {} · Themes: 1–4", self.selected)),
                        )
                        .child(
                            Shell::new(
                                "explorer-shell",
                                self.sidebar,
                                self.inspector,
                                self.sources.clone(),
                                Button::new("sources-rail", "Docs")
                                    .size(ControlSize::Mini)
                                    .on_click(move |_, w, cx| {
                                        CommandRegistry::invoke(
                                            &sidebar_commands,
                                            &"view.sidebar".into(),
                                            w,
                                            cx,
                                        );
                                    }),
                                div()
                                    .size_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(format!("Document preview: {}", self.selected)),
                            )
                            .inspector(
                                format!("Details — {}", self.selected),
                                div()
                                    .p(px(16.))
                                    .child(format!("Document ID: {}", self.selected)),
                            )
                            .inspector_rail(
                                Button::new("details-rail", "Info")
                                    .style(ButtonStyle::Textured)
                                    .size(ControlSize::Mini)
                                    .on_click(move |_, w, cx| {
                                        CommandRegistry::invoke(
                                            &rail_commands,
                                            &"view.inspector".into(),
                                            w,
                                            cx,
                                        );
                                    }),
                            )
                            .on_sidebar_change(cx.listener(|view, p, _, cx| {
                                view.sidebar = *p;
                                view.sync(cx);
                                cx.notify();
                            }))
                            .on_inspector_change(cx.listener(|view, p, _, cx| {
                                view.inspector = *p;
                                view.sync(cx);
                                cx.notify();
                            })),
                        ),
                ),
        )
    }
}
fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        if let Err(error) = silvestris::init(ThemePreference::default(), cx) {
            eprintln!("{error:#}");
            cx.quit();
            return;
        }
        cx.bind_keys([
            gpui::KeyBinding::new("tab", FocusNext, Some("Explorer")),
            gpui::KeyBinding::new("shift-tab", FocusPrevious, Some("Explorer")),
        ]);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1200.), px(720.)), cx);
        let options = window_options("Document Explorer", bounds, size(px(480.), px(320.)), cx);
        open_window(options, cx, |window, cx| {
            cx.new(|cx| Explorer::new(window, cx))
        })
        .expect("open explorer");
        cx.activate(true);
    });
}
