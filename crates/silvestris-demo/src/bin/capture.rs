//! Deterministic golden renders for the visual gate (#18) and M2.
//!
//! One process renders one appearance at one scale. It opens a fixed-size
//! window, draws the control samples in a fixed grid, reads the frame back with
//! `Window::render_to_image`, crops each sample and writes PNGs. Text uses
//! grayscale antialiasing and the bundled fonts; no control is focused, and
//! animated samples are frozen, so caret blink, the default-button throb and
//! progress phases cannot affect the pixels.
//!
//! Run through `tools/metrology/render.ps1`; it supplies every appearance and
//! scale and the output directory.

use std::path::{Path, PathBuf};

use gpui::{
    deferred, div, point, prelude::*, px, size, App, Bounds, Context, Entity, Render, SharedString,
    Window,
};
use silvestris::chrome::WindowFrame;
use silvestris::command::CommandRegistry;
use silvestris::controls::{
    Button, ButtonStyle, CheckState, Checkbox, ControlSize, Disclosure, DisclosureStyle, GroupBox,
    Orientation, PopUpButton, PopupStyle, ProgressIndicator, RadioGroup, RadioOption, RowAccessory,
    RowStatus, ScrollAxes, ScrollView, Segment, SegmentStyle, SegmentedControl, Separator, Slider,
    SourceList, SourceListStyle, TabView, TextField, ThumbStyle, Well,
};
use silvestris::core::command::{CommandDescriptor, MenuEntry, MenuModel};
use silvestris::core::source_list::SourceNode;
use silvestris::menu::PopupMenu;
use silvestris::window_role::WindowRole;
use silvestris::{ActiveTheme, ThemeId, ThemePreference};

const PAD: f32 = 16.;
const GAP: f32 = 12.;
const CELL_WIDTH: f32 = 244.;
const CELL_HEIGHT: f32 = 170.;
const COLUMNS: usize = 4;
const PAGE_SIZE: usize = 16;

struct Sample {
    id: &'static str,
    width: f32,
    height: f32,
}

const SAMPLES: [Sample; 48] = [
    Sample {
        id: "button-default",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-regular",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-small",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-mini",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-disabled",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-round",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-recessed",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-help",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "button-bevel",
        width: 120.,
        height: 40.,
    },
    Sample {
        id: "checkbox-on",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "checkbox-off",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "checkbox-mixed",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "checkbox-small",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "checkbox-mini",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "checkbox-disabled",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "radio-on",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "radio-off",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "radio-small",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "radio-disabled",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "popup-button",
        width: 200.,
        height: 30.,
    },
    Sample {
        id: "popup-textured",
        width: 200.,
        height: 30.,
    },
    Sample {
        id: "popup-disabled",
        width: 200.,
        height: 30.,
    },
    Sample {
        id: "segmented",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "segmented-textured",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "segmented-icon",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "segmented-small",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "slider",
        width: 200.,
        height: 40.,
    },
    Sample {
        id: "slider-pointed",
        width: 200.,
        height: 48.,
    },
    Sample {
        id: "slider-vertical",
        width: 200.,
        height: 130.,
    },
    Sample {
        id: "slider-disabled",
        width: 200.,
        height: 40.,
    },
    Sample {
        id: "progress",
        width: 220.,
        height: 24.,
    },
    Sample {
        id: "progress-indeterminate",
        width: 220.,
        height: 24.,
    },
    Sample {
        id: "spinner",
        width: 60.,
        height: 40.,
    },
    Sample {
        id: "progress-ring",
        width: 60.,
        height: 40.,
    },
    Sample {
        id: "disclosure",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "disclosure-expanded",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "disclosure-button",
        width: 200.,
        height: 24.,
    },
    Sample {
        id: "groupbox",
        width: 220.,
        height: 70.,
    },
    Sample {
        id: "well",
        width: 220.,
        height: 50.,
    },
    Sample {
        id: "separator",
        width: 220.,
        height: 20.,
    },
    Sample {
        id: "source-list",
        width: 200.,
        height: 160.,
    },
    Sample {
        id: "source-list-secondary",
        width: 200.,
        height: 160.,
    },
    Sample {
        id: "text-field",
        width: 240.,
        height: 32.,
    },
    Sample {
        id: "window-frame",
        width: 244.,
        height: 108.,
    },
    Sample {
        id: "window-frame-fixed",
        width: 244.,
        height: 108.,
    },
    Sample {
        id: "window-frame-edited",
        width: 244.,
        height: 108.,
    },
    Sample {
        id: "tab-view",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "scroll-view",
        width: 244.,
        height: 110.,
    },
];

/// Open menus are larger than a grid cell, so they get their own last page,
/// laid out in one row. Appending a page keeps every other sample's
/// position, so no existing golden moves.
const MENU_SAMPLES: [Sample; 2] = [
    Sample {
        id: "menu-arrange-by",
        width: 360.,
        height: 390.,
    },
    Sample {
        id: "menu-marks",
        width: 260.,
        height: 390.,
    },
];

fn menu_page() -> usize {
    SAMPLES.len().div_ceil(PAGE_SIZE)
}

fn rows(page: usize) -> usize {
    page_samples(page).len().div_ceil(COLUMNS)
}

fn page_samples(page: usize) -> &'static [Sample] {
    if page == menu_page() {
        return &MENU_SAMPLES;
    }
    let start = (page * PAGE_SIZE).min(SAMPLES.len());
    let end = (start + PAGE_SIZE).min(SAMPLES.len());
    &SAMPLES[start..end]
}

fn root_size(page: usize) -> (f32, f32) {
    if page == menu_page() {
        let width = MENU_SAMPLES.iter().map(|s| s.width + GAP).sum::<f32>() - GAP;
        let height = MENU_SAMPLES.iter().map(|s| s.height).fold(0., f32::max);
        return (PAD + width + PAD, PAD + height + PAD);
    }
    (
        PAD + COLUMNS as f32 * (CELL_WIDTH + GAP) - GAP + PAD,
        PAD + rows(page) as f32 * (CELL_HEIGHT + GAP) - GAP + PAD,
    )
}

fn sample_rects(page: usize) -> Vec<(&'static str, f32, f32, f32, f32)> {
    if page == menu_page() {
        let mut x = PAD;
        return MENU_SAMPLES
            .iter()
            .map(|s| {
                let rect = (s.id, x, PAD, s.width, s.height);
                x += s.width + GAP;
                rect
            })
            .collect();
    }
    let samples = page_samples(page);
    let mut rects = Vec::with_capacity(samples.len());
    for (i, sample) in samples.iter().enumerate() {
        let column = i % COLUMNS;
        let row = i / COLUMNS;
        let x = PAD + column as f32 * (CELL_WIDTH + GAP) + (CELL_WIDTH - sample.width) / 2.0;
        let y = PAD + row as f32 * (CELL_HEIGHT + GAP);
        rects.push((sample.id, x, y, sample.width, sample.height));
    }
    rects
}

struct CaptureView {
    field: Entity<TextField>,
    list: Entity<SourceList>,
    secondary: Entity<SourceList>,
    // Both scroll bars and the corner, as in the Finder list view.
    scroll: gpui::ScrollHandle,
    menus: Vec<Entity<PopupMenu>>,
    // Finder content white under the Arrange By menu.
    content: silvestris::core::Rgba8,
    row_alternate: silvestris::core::Rgba8,
    page: usize,
}

impl CaptureView {
    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        out: PathBuf,
        scale: f32,
        page: usize,
    ) -> Self {
        let field = cx.new(|cx| TextField::new(window, cx));
        field.update(cx, |field, cx| field.set_text("Snow Leopard", cx));
        let mut root = SourceNode::new("home", "Home");
        root.children = vec![
            SourceNode::new("desktop", "Desktop"),
            SourceNode::new("documents", "Documents"),
        ];
        let list = cx.new(|cx| SourceList::new(vec![root], cx).expect("source list"));
        list.update(cx, |list, cx| {
            list.set_expanded("home", true, cx);
            list.select("documents", cx);
        });
        let secondary = cx.new(|cx| {
            let inbox = SourceNode::new("inbox", "Inbox");
            let sent = SourceNode::new("sent", "Sent");
            let mut rss = SourceNode::new("rss", "RSS");
            rss.children = vec![SourceNode::new("feed", "Apple Hot News")];
            SourceList::new(
                vec![SourceNode::section(
                    "mailboxes",
                    "Mailboxes",
                    vec![inbox, sent, rss],
                )],
                cx,
            )
            .expect("secondary source list")
        });
        secondary.update(cx, |list, cx| {
            list.style(SourceListStyle::Secondary, cx);
            list.set_row_accessories(
                |row| match row.id.as_str() {
                    "inbox" => vec![
                        RowAccessory::badge(4),
                        RowAccessory::status(RowStatus::Running),
                    ],
                    "sent" => vec![RowAccessory::Hover(Box::new(RowAccessory::Eject))],
                    "rss" => vec![RowAccessory::Spinner],
                    _ => Vec::new(),
                },
                cx,
            );
            list.select("inbox", cx);
        });

        let menus = if page == menu_page() {
            let rects = sample_rects(page);
            vec![
                arrange_by_menu(rects[0].1 + 10., rects[0].2, window, cx),
                marks_menu(rects[1].1 + 10., rects[1].2 + 10., window, cx),
            ]
        } else {
            Vec::new()
        };

        window.on_next_frame(move |window, cx| {
            capture(window, cx, &out, scale, page);
            cx.quit();
        });

        Self {
            field,
            list,
            secondary,
            scroll: gpui::ScrollHandle::new(),
            menus,
            content: cx.theme().colors.content,
            row_alternate: cx.theme().colors.row_alternate,
            page,
        }
    }

    fn sample(&self, id: &str) -> gpui::AnyElement {
        let button = |text: &'static str| Button::new(format!("golden-button-{text}"), text);
        match id {
            "button-default" => button("Save")
                .default_button(true)
                .throb(false)
                .size(ControlSize::Regular)
                .into_any_element(),
            "button-regular" => button("Cancel")
                .size(ControlSize::Regular)
                .into_any_element(),
            "button-small" => button("Replace")
                .size(ControlSize::Small)
                .into_any_element(),
            "button-mini" => button("Mini").size(ControlSize::Mini).into_any_element(),
            "button-disabled" => button("Disabled").disabled(true).into_any_element(),
            "button-round" => button("Round")
                .style(ButtonStyle::Round)
                .icon(|ink| div().text_color(ink).child("\u{2699}"))
                .into_any_element(),
            "button-recessed" => button("Recessed")
                .style(ButtonStyle::Recessed)
                .into_any_element(),
            "button-help" => button("Help").style(ButtonStyle::Help).into_any_element(),
            "button-bevel" => button("Bevel").style(ButtonStyle::Bevel).into_any_element(),
            "checkbox-on" => Checkbox::new("golden-checkbox-on", "Show hidden files")
                .checked(true)
                .into_any_element(),
            "checkbox-off" => Checkbox::new("golden-checkbox-off", "Unchecked").into_any_element(),
            "checkbox-mixed" => Checkbox::new("golden-checkbox-mixed", "Mixed")
                .state(CheckState::Mixed)
                .into_any_element(),
            "checkbox-small" => Checkbox::new("golden-checkbox-small", "Small")
                .size(ControlSize::Small)
                .checked(true)
                .into_any_element(),
            "checkbox-mini" => Checkbox::new("golden-checkbox-mini", "Mini")
                .size(ControlSize::Mini)
                .into_any_element(),
            "checkbox-disabled" => Checkbox::new("golden-checkbox-disabled", "Disabled")
                .checked(true)
                .disabled(true)
                .into_any_element(),
            "radio-on" => RadioGroup::new(
                "golden-radio-on",
                "Check for updates",
                vec![
                    RadioOption::new("Automatically"),
                    RadioOption::new("Manually"),
                ],
            )
            .selected(Some(0))
            .into_any_element(),
            "radio-off" => RadioGroup::new(
                "golden-radio-off",
                "Check for updates",
                vec![
                    RadioOption::new("Automatically"),
                    RadioOption::new("Manually"),
                ],
            )
            .selected(Some(1))
            .into_any_element(),
            "radio-small" => RadioGroup::new(
                "golden-radio-small",
                "Size",
                vec![RadioOption::new("Small"), RadioOption::new("Mini")],
            )
            .size(ControlSize::Small)
            .selected(Some(0))
            .into_any_element(),
            "radio-disabled" => RadioGroup::new(
                "golden-radio-disabled",
                "Disabled",
                vec![RadioOption::new("Unavailable").disabled(true)],
            )
            .into_any_element(),
            "popup-button" => PopUpButton::new(
                "golden-popup",
                "Appearance",
                vec![SharedString::from("Blue"), SharedString::from("Graphite")],
            )
            .selected(Some(0))
            .size(ControlSize::Regular)
            .min_width(140.)
            .into_any_element(),
            "popup-textured" => PopUpButton::new(
                "golden-popup-textured",
                "Scope",
                vec![
                    SharedString::from("Everything"),
                    SharedString::from("Selection"),
                ],
            )
            .selected(Some(0))
            .size(ControlSize::Regular)
            .min_width(140.)
            .bezel(PopupStyle::Textured)
            .into_any_element(),
            "popup-disabled" => PopUpButton::new(
                "golden-popup-disabled",
                "Unavailable",
                vec![SharedString::from("Nothing")],
            )
            .selected(Some(0))
            .size(ControlSize::Regular)
            .min_width(140.)
            .disabled(true)
            .into_any_element(),
            "segmented" => SegmentedControl::new(
                "golden-segmented",
                "View",
                vec![
                    Segment::new("icon", "Icons"),
                    Segment::new("list", "List"),
                    Segment::new("column", "Columns"),
                ],
            )
            .selected("list")
            .into_any_element(),
            "segmented-textured" => SegmentedControl::new(
                "golden-segmented-textured",
                "View",
                vec![Segment::new("grid", "Grid"), Segment::new("list", "List")],
            )
            .style(SegmentStyle::Textured)
            .selected("grid")
            .into_any_element(),
            "segmented-icon" => SegmentedControl::new(
                "golden-segmented-icon",
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
            })
            .into_any_element(),
            "segmented-small" => SegmentedControl::new(
                "golden-segmented-small",
                "Density",
                vec![
                    Segment::new("compact", "Compact"),
                    Segment::new("full", "Full"),
                ],
            )
            .size(ControlSize::Small)
            .selected("compact")
            .into_any_element(),
            "slider" => Slider::new("golden-slider", "Volume", 0.6).into_any_element(),
            // Tick labels centre on the end ticks and overhang the track, so the
            // sample leaves room for them (M: Dock "Small"/"Large").
            "slider-pointed" => div()
                .px(px(20.))
                .child(
                    Slider::new("golden-slider-pointed", "Dock size", 0.5)
                        .thumb(ThumbStyle::Pointed)
                        .ticks([0.0, 0.5, 1.0])
                        .tick_labels(["Small", "Medium", "Large"])
                        .snap_to_ticks(true),
                )
                .into_any_element(),
            "slider-vertical" => Slider::new("golden-slider-vertical", "Level", 0.35)
                .orientation(Orientation::Vertical)
                .thumb(ThumbStyle::Pointed)
                .ticks([0.0, 0.5, 1.0])
                .tick_labels(["Low", "Mid", "High"])
                .into_any_element(),
            "slider-disabled" => Slider::new("golden-slider-disabled", "Disabled", 0.4)
                .disabled(true)
                .into_any_element(),
            "progress" => {
                ProgressIndicator::new("golden-progress", "Copying", Some(0.55)).into_any_element()
            }
            "progress-indeterminate" => {
                ProgressIndicator::new("golden-progress-pole", "Preparing", None)
                    .animate(false)
                    .into_any_element()
            }
            "spinner" => ProgressIndicator::spinner("golden-spinner", "Working")
                .animate(false)
                .into_any_element(),
            "progress-ring" => {
                ProgressIndicator::circular("golden-ring", "Ring", 0.7).into_any_element()
            }
            "disclosure" => Disclosure::new("golden-disclosure", "Details").into_any_element(),
            "disclosure-expanded" => Disclosure::new("golden-disclosure-open", "Details")
                .expanded(true)
                .into_any_element(),
            "disclosure-button" => Disclosure::new("golden-disclosure-button", "Details")
                .style(DisclosureStyle::Button)
                .expanded(true)
                .into_any_element(),
            "groupbox" => GroupBox::new("golden-group")
                .title("Group Box")
                .child(div().text_size(px(11.)).child("Grouped content"))
                .into_any_element(),
            "well" => Well::new("golden-well")
                .child(div().text_size(px(11.)).child("Recessed well"))
                .into_any_element(),
            "separator" => Separator::horizontal().inset(8.0).into_any_element(),
            // A flex column so the tab view's box fills the sample cell.
            "menu-arrange-by" => div()
                .size_full()
                .bg(silvestris::convert::hsla(self.content))
                .into_any_element(),
            "menu-marks" => div().size_full().into_any_element(),
            "tab-view" => div()
                .size_full()
                .flex()
                .flex_col()
                .child(
                    TabView::new("golden-tab-view")
                        .tabs(
                            vec![
                                Segment::new("date", "Date & Time"),
                                Segment::new("zone", "Time Zone"),
                                Segment::new("clock", "Clock"),
                            ],
                            "date",
                        )
                        .child(div().text_size(px(11.)).child("Pane content")),
                )
                .into_any_element(),
            "scroll-view" => {
                let content = silvestris::convert::hsla(self.content);
                let alternate = silvestris::convert::hsla(self.row_alternate);
                let rows = (0..12).map(|i| {
                    div().flex_none().h(px(18.)).w(px(400.)).bg(if i % 2 == 0 {
                        content
                    } else {
                        alternate
                    })
                });
                ScrollView::new("golden-scroll-view", &self.scroll)
                    .axes(ScrollAxes::Both)
                    .autohide(false)
                    .child(
                        div()
                            .id("golden-scroll-content")
                            .size_full()
                            .flex()
                            .flex_col()
                            .overflow_scroll()
                            .track_scroll(&self.scroll)
                            .children(rows),
                    )
                    .into_any_element()
            }
            "text-field" => self.field.clone().into_any_element(),
            "source-list" => self.list.clone().into_any_element(),
            "source-list-secondary" => self.secondary.clone().into_any_element(),
            "window-frame" => WindowFrame::new("Document")
                .role(WindowRole::Document)
                .child(frame_body())
                .into_any_element(),
            "window-frame-fixed" => WindowFrame::new("About")
                .role(WindowRole::About)
                .child(frame_body())
                .into_any_element(),
            "window-frame-edited" => WindowFrame::new("Untitled")
                .role(WindowRole::Document)
                .edited(true)
                .child(frame_body())
                .into_any_element(),
            other => unreachable!("unknown sample {other}"),
        }
    }
}

fn frame_body() -> impl IntoElement {
    div().size_full().p(px(8.)).child("Content")
}

impl Render for CaptureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (width, height) = root_size(self.page);
        let mut root = div()
            .relative()
            .w(px(width))
            .h(px(height))
            .bg(silvestris::convert::hsla(theme.colors.window));
        for (id, x, y, w, h) in sample_rects(self.page) {
            root = root.child(
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .child(self.sample(id)),
            );
        }
        for menu in &self.menus {
            root = root.child(deferred(menu.clone()).priority(1));
        }
        root
    }
}

/// Register no-op commands and build a menu model from `(id, label)` rows;
/// an empty id is a separator.
fn menu_commands(
    registry: &Entity<CommandRegistry>,
    rows: &[(&str, &str)],
    cx: &mut App,
) -> Vec<MenuEntry> {
    registry.update(cx, |registry, cx| {
        rows.iter()
            .map(|&(id, label)| {
                if id.is_empty() {
                    return MenuEntry::Separator;
                }
                registry
                    .register(CommandDescriptor::action(id, label), |_, _, _| {}, cx)
                    .expect("unique golden command");
                MenuEntry::Command(id.into())
            })
            .collect()
    })
}

/// The Finder Action menu with Arrange By open, as in
/// `right-click-context-menu-nested.png` (M: menu.context.rows): a pop-down
/// menu hanging from the toolbar, its submenu beside the highlighted row.
fn arrange_by_menu(x: f32, y: f32, window: &mut Window, cx: &mut App) -> Entity<PopupMenu> {
    let registry = cx.new(|_| CommandRegistry::default());
    let sort = menu_commands(
        &registry,
        &[
            ("sort.name", "Name"),
            ("sort.modified", "Date Modified"),
            ("sort.created", "Date Created"),
            ("sort.size", "Size"),
            ("sort.kind", "Kind"),
            ("sort.label", "Label"),
        ],
        cx,
    );
    let mut entries = menu_commands(
        &registry,
        &[
            ("file.new_folder", "New Folder"),
            ("file.open", "Open"),
            ("", ""),
            ("file.info", "Get Info"),
            ("file.burn", "Burn \u{201c}terezi\u{201d} to Disc\u{2026}"),
            ("", ""),
            ("view.clean_up", "Clean Up"),
        ],
        cx,
    );
    entries.push(MenuEntry::Submenu {
        label: "Arrange By".into(),
        entries: sort,
    });
    entries.extend(menu_commands(
        &registry,
        &[("view.options", "Show View Options")],
        cx,
    ));
    let model = MenuModel {
        label: "Action".into(),
        entries,
    };
    let menu = cx.new(|cx| PopupMenu::new(model, registry, window, cx));
    menu.update(cx, |menu, cx| {
        menu.open_pop_down(point(px(x), px(y)), window, cx);
        menu.highlight_path(&[7], window, cx);
    });
    menu
}

/// A contextual menu with a checked, a mixed and a disabled item and key
/// equivalents, for the mark and key columns.
fn marks_menu(x: f32, y: f32, window: &mut Window, cx: &mut App) -> Entity<PopupMenu> {
    let registry = cx.new(|_| CommandRegistry::default());
    registry.update(cx, |registry, cx| {
        for descriptor in [
            CommandDescriptor::toggle("format.bold", "Bold", true).shortcut("secondary-b"),
            CommandDescriptor::toggle("format.italic", "Italic", false)
                .mixed(true)
                .shortcut("secondary-i"),
            CommandDescriptor::toggle("format.underline", "Underline", false)
                .shortcut("secondary-u"),
            CommandDescriptor::action("format.copy_style", "Copy Style")
                .shortcut("alt-secondary-c"),
            CommandDescriptor::action("format.paste_style", "Paste Style")
                .shortcut("alt-secondary-v")
                .enabled(false),
            CommandDescriptor::action("format.show_fonts", "Show Fonts").needs_input(),
        ] {
            registry
                .register(descriptor, |_, _, _| {}, cx)
                .expect("unique golden command");
        }
    });
    let command = |id: &str| MenuEntry::Command(id.into());
    let model = MenuModel {
        label: "Format".into(),
        entries: vec![
            command("format.bold"),
            command("format.italic"),
            command("format.underline"),
            MenuEntry::Separator,
            command("format.copy_style"),
            command("format.paste_style"),
            MenuEntry::Separator,
            command("format.show_fonts"),
        ],
    };
    let menu = cx.new(|cx| PopupMenu::new(model, registry, window, cx));
    menu.update(cx, |menu, cx| {
        menu.open(point(px(x), px(y)), window, cx);
        menu.highlight_path(&[2], window, cx);
    });
    menu
}

fn capture(window: &mut Window, cx: &mut App, out: &Path, scale: f32, page: usize) {
    let image = window.render_to_image().expect("frame readback");
    let (width, height) = root_size(page);
    let expected_width = (width * scale).round() as u32;
    let expected_height = (height * scale).round() as u32;
    assert!(
        image.width() >= expected_width && image.height() >= expected_height,
        "frame {}x{} is smaller than the scaled root {expected_width}x{expected_height}",
        image.width(),
        image.height()
    );
    std::fs::create_dir_all(out).expect("create render directory");
    for (id, x, y, w, h) in sample_rects(page) {
        let crop = image::imageops::crop_imm(
            &image,
            (x * scale).round() as u32,
            (y * scale).round() as u32,
            (w * scale).round() as u32,
            (h * scale).round() as u32,
        )
        .to_image();
        crop.save(out.join(format!("{id}.png")))
            .expect("write sample png");
    }
    let layout = serde_json::json!({
        "page": page,
        "scale": scale,
        "logical_root": [width, height],
        "physical_frame": [image.width(), image.height()],
        "samples": sample_rects(page).iter().map(|(id, x, y, w, h)| {
            serde_json::json!({
                "id": id,
                "logical": [x, y, w, h],
                "physical": [
                    (x * scale).round() as u32,
                    (y * scale).round() as u32,
                    (w * scale).round() as u32,
                    (h * scale).round() as u32,
                ],
            })
        }).collect::<Vec<_>>(),
    });
    std::fs::write(
        out.join(format!("layout-{page}.json")),
        serde_json::to_vec_pretty(&layout).expect("serialize layout"),
    )
    .expect("write layout");
    let _ = cx;
}

fn appearance(name: &str) -> ThemeId {
    match name {
        "aqua-light" => ThemeId::AQUA_LIGHT,
        "aqua-dark" => ThemeId::AQUA_DARK,
        "geist-light" => ThemeId::GEIST_LIGHT,
        "geist-dark" => ThemeId::GEIST_DARK,
        other => panic!("unknown appearance {other}"),
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let appearance_name = args.next().expect("appearance name");
    let scale: f32 = args.next().expect("scale").parse().expect("scale number");
    let out = PathBuf::from(args.next().expect("output directory"));
    let page: usize = args
        .next()
        .unwrap_or_else(|| "0".into())
        .parse()
        .expect("page number");
    let theme = appearance(&appearance_name);
    let (width, height) = root_size(page);
    let request_width = width * scale + 32.;
    let request_height = height * scale + 80.;

    gpui_platform::application().run(move |cx: &mut App| {
        silvestris::init(ThemePreference::explicit(theme), cx).expect("init Silvestris");
        cx.set_text_rendering_mode(gpui::TextRenderingMode::Grayscale);
        let options = gpui::WindowOptions {
            window_bounds: Some(gpui::WindowBounds::Windowed(Bounds::new(
                point(px(0.), px(0.)),
                size(px(request_width), px(request_height)),
            ))),
            titlebar: None,
            window_decorations: Some(gpui::WindowDecorations::Client),
            focus: true,
            show: true,
            ..Default::default()
        };
        cx.open_window(options, |window, cx| {
            window.set_scale_factor(scale);
            cx.new(|cx| CaptureView::new(window, cx, out.clone(), scale, page))
        })
        .expect("open capture window");
    });
}
