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
use silvestris::binding::ListController;
use silvestris::bottom_bar::{
    BottomBar, BottomBarControlStyle, BottomBarItem, BottomBarSize, BottomBarStatus,
};
use silvestris::chrome::WindowFrame;
use silvestris::command::CommandRegistry;
use silvestris::controls::{
    ActionMenu, BrowserNode, Button, ButtonStyle, CheckState, Checkbox, ColumnAlign, ColumnBrowser,
    CommandPopDown, ControlSize, Disclosure, DisclosureStyle, GroupBox, Orientation, PopUpButton,
    PopupStyle, ProgressIndicator, RadioGroup, RadioOption, RowAccessory, RowStatus, Scope,
    ScopeBar, ScrollAxes, ScrollView, Segment, SegmentStyle, SegmentedControl, Separator, Slider,
    SortDirection, SourceList, SourceListStyle, TabView, TableColumn, TableView, TextField,
    ThumbStyle, Well,
};
use silvestris::core::command::{CommandDescriptor, MenuEntry, MenuModel};
use silvestris::core::source_list::SourceNode;
use silvestris::core::split::{SplitDividerStyle, SplitOrientation, SplitPane, SplitState};
use silvestris::file_panel::{
    FileEntry, FilePanel, FilePlace, FileViewMode, MemoryProvider, PanelMode, PanelOptions,
    PlaceSection,
};
use silvestris::icons::{
    ConsumerArtwork, FileArtwork, FileIconState, Icon, TemplateImage, TemplateImageSlot,
};
use silvestris::menu::PopupMenu;
use silvestris::preferences::{
    pane_body, pane_viewport, Form, FormRow, PreferencesNavigator, PreferencesToolbar,
};
use silvestris::split::SplitView;
use silvestris::toolbar::{
    Toolbar, ToolbarDisplayMode, ToolbarItem, ToolbarItemStyle, ToolbarLayout, ToolbarPalette,
};
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

const SAMPLES: [Sample; 60] = [
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
    Sample {
        id: "table-view",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "column-browser",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "artwork-file-states",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "artwork-template-states",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "artwork-consumer",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "toolbar-rectangular",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "toolbar-capsule",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "toolbar-disabled",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "toolbar-icon-only",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "toolbar-text-only",
        width: 244.,
        height: 110.,
    },
    Sample {
        id: "bottom-bar-regular",
        width: 236.,
        height: 96.,
    },
    Sample {
        id: "bottom-bar-small",
        width: 236.,
        height: 96.,
    },
];

/// File panels are larger than a grid cell; they get their own page after the
/// menus so neither the grid nor the menu samples move.
const PANEL_SAMPLES: [Sample; 8] = [
    Sample {
        id: "open-dialog-columns",
        width: 527.,
        height: 398.,
    },
    Sample {
        id: "open-dialog-list",
        width: 527.,
        height: 398.,
    },
    Sample {
        id: "save-sheet-minimal",
        width: 760.,
        height: 398.,
    },
    Sample {
        id: "save-sheet-expanded",
        width: 760.,
        height: 560.,
    },
    Sample {
        // At rest after the 400 x 300 document grew to fit the 527-wide
        // sheet; the growth and exact restore are covered by tests/sheet.rs.
        id: "save-sheet-grow",
        width: 560.,
        height: 480.,
    },
    Sample {
        // The Open panel in icon view: folders, documents, a deliberately
        // long filename, one selected item and one dimmed item.
        id: "open-dialog-icons",
        width: 527.,
        height: 398.,
    },
    Sample {
        // Expanded Save in icon view: the chooser's icon grid.
        id: "save-sheet-expanded-icons",
        width: 760.,
        height: 560.,
    },
    Sample {
        // The minimal Save sheet alone, cropped to its own card so any dead
        // vertical space is obvious in review.
        id: "save-sheet-minimal-crop",
        width: 560.,
        height: 200.,
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

/// Preferences windows are wider than the control grid, so each gets its own
/// page after the panels. Appending pages keeps every existing sample's
/// position.
const PREFERENCE_SAMPLES: [Sample; 2] = [
    Sample {
        id: "preferences-general",
        width: 668.,
        height: 380.,
    },
    Sample {
        id: "preferences-scroll",
        width: 668.,
        height: 560.,
    },
];

/// Split views get their own page so the thin and thick dividers are shown at
/// a readable size rather than inside a control cell.
const SPLIT_SAMPLES: [Sample; 2] = [
    Sample {
        id: "split-view-thin",
        width: 360.,
        height: 220.,
    },
    Sample {
        id: "split-view-thick",
        width: 360.,
        height: 220.,
    },
];

/// Action menu, command pop-down and scope bar, closed, on their own page so
/// the arrow and scope strip are readable.
const ACTION_SAMPLES: [Sample; 1] = [Sample {
    id: "action-menu-scope-bar",
    width: 460.,
    height: 260.,
}];

fn menu_page() -> usize {
    SAMPLES.len().div_ceil(PAGE_SIZE)
}

fn panel_page_start() -> usize {
    menu_page() + 1
}

fn on_panel_page(page: usize) -> bool {
    (panel_page_start()..panel_page_start() + PANEL_SAMPLES.len()).contains(&page)
}

fn panel_index(page: usize) -> usize {
    page - panel_page_start()
}

fn preference_page_start() -> usize {
    panel_page_start() + PANEL_SAMPLES.len()
}

fn on_preference_page(page: usize) -> bool {
    (preference_page_start()..preference_page_start() + PREFERENCE_SAMPLES.len()).contains(&page)
}

fn preference_index(page: usize) -> usize {
    page - preference_page_start()
}

fn split_page_start() -> usize {
    preference_page_start() + PREFERENCE_SAMPLES.len()
}

fn on_split_page(page: usize) -> bool {
    (split_page_start()..split_page_start() + SPLIT_SAMPLES.len()).contains(&page)
}

fn split_index(page: usize) -> usize {
    page - split_page_start()
}

fn action_page_start() -> usize {
    split_page_start() + SPLIT_SAMPLES.len()
}

fn on_action_page(page: usize) -> bool {
    (action_page_start()..action_page_start() + ACTION_SAMPLES.len()).contains(&page)
}

fn action_index(page: usize) -> usize {
    page - action_page_start()
}

fn rows(page: usize) -> usize {
    page_samples(page).len().div_ceil(COLUMNS)
}

fn page_samples(page: usize) -> &'static [Sample] {
    if page == menu_page() {
        return &MENU_SAMPLES;
    }
    if on_panel_page(page) {
        let index = panel_index(page);
        return &PANEL_SAMPLES[index..index + 1];
    }
    if on_preference_page(page) {
        let index = preference_index(page);
        return &PREFERENCE_SAMPLES[index..index + 1];
    }
    if on_split_page(page) {
        let index = split_index(page);
        return &SPLIT_SAMPLES[index..index + 1];
    }
    if on_action_page(page) {
        let index = action_index(page);
        return &ACTION_SAMPLES[index..index + 1];
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
    if on_panel_page(page) {
        let sample = &PANEL_SAMPLES[panel_index(page)];
        return (PAD + sample.width + PAD, PAD + sample.height + PAD);
    }
    if on_preference_page(page) {
        let sample = &PREFERENCE_SAMPLES[preference_index(page)];
        return (PAD + sample.width + PAD, PAD + sample.height + PAD);
    }
    if on_split_page(page) {
        let sample = &SPLIT_SAMPLES[split_index(page)];
        return (PAD + sample.width + PAD, PAD + sample.height + PAD);
    }
    if on_action_page(page) {
        let sample = &ACTION_SAMPLES[action_index(page)];
        return (PAD + sample.width + PAD, PAD + sample.height + PAD);
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
    if on_panel_page(page) {
        let sample = &PANEL_SAMPLES[panel_index(page)];
        return vec![(sample.id, PAD, PAD, sample.width, sample.height)];
    }
    if on_preference_page(page) {
        let sample = &PREFERENCE_SAMPLES[preference_index(page)];
        return vec![(sample.id, PAD, PAD, sample.width, sample.height)];
    }
    if on_split_page(page) {
        let sample = &SPLIT_SAMPLES[split_index(page)];
        return vec![(sample.id, PAD, PAD, sample.width, sample.height)];
    }
    if on_action_page(page) {
        let sample = &ACTION_SAMPLES[action_index(page)];
        return vec![(sample.id, PAD, PAD, sample.width, sample.height)];
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
    // Finder list view: 16 px header, 19 px rows, sorted by Name.
    table: Entity<TableView<(u32, &'static str, &'static str)>>,
    browser: Entity<ColumnBrowser>,
    panels: Vec<Entity<FilePanel>>,
    menus: Vec<Entity<PopupMenu>>,
    toolbar_commands: Entity<CommandRegistry>,
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
        let toolbar_commands = cx.new(|_| CommandRegistry::default());
        for (id, label, enabled, checked) in [
            ("golden.sidebar", "Sidebar", true, true),
            ("golden.action", "Action", true, false),
            ("golden.info", "Info", true, false),
            ("golden.disabled", "Unavailable", false, false),
            ("golden.add", "Add", true, false),
            ("golden.remove", "Remove", true, false),
        ] {
            toolbar_commands.update(cx, |registry, cx| {
                let descriptor = if checked {
                    CommandDescriptor::toggle(id, label, checked)
                } else {
                    CommandDescriptor::action(id, label)
                };
                registry
                    .register(descriptor.enabled(enabled), |_, _, _| {}, cx)
                    .expect("unique capture command");
            });
        }
        let mut root = SourceNode::new("home", "Home");
        root.children = vec![
            SourceNode::new("desktop", "Desktop"),
            SourceNode::new("documents", "Documents"),
        ];
        let table = cx.new(|cx| {
            let controller = cx.new(|_| {
                let mut c = ListController::new(|row: &(u32, &'static str, &'static str)| {
                    row.0.to_string().into()
                });
                c.set_items(vec![
                    (0, "Music", "--"),
                    (1, "Desktop", "--"),
                    (2, "Send Registration", "4 KB"),
                    (3, "Library", "--"),
                    (4, "Documents", "--"),
                    (5, "Pictures", "--"),
                ]);
                c
            });
            let columns = vec![
                TableColumn::text("name", "Name", |r: &(u32, &'static str, &'static str)| {
                    r.1.into()
                })
                .width(160.)
                .sort_by_text(),
                TableColumn::text("size", "Size", |r: &(u32, &'static str, &'static str)| {
                    r.2.into()
                })
                .width(70.)
                .align(ColumnAlign::Trailing),
            ];
            let mut table = TableView::new(controller, columns, cx);
            table.set_row_height(19., cx);
            table.set_header_height(16., cx);
            table.set_sort("name", SortDirection::Ascending, cx);
            table
        });
        let list = cx.new(|cx| SourceList::new(vec![root], cx).expect("source list"));
        list.update(cx, |list, cx| {
            list.set_expanded("home", true, cx);
            list.select("documents", cx);
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

        let panels = if on_panel_page(page) {
            let provider: std::rc::Rc<dyn silvestris::file_panel::FileProvider> =
                std::rc::Rc::new(capture_file_provider());
            let specs = [
                (
                    PanelMode::Open,
                    true,
                    FileViewMode::Columns,
                    "Untitled.txt",
                    false,
                ),
                (
                    PanelMode::Open,
                    true,
                    FileViewMode::List,
                    "Untitled.txt",
                    false,
                ),
                (
                    PanelMode::Save,
                    false,
                    FileViewMode::Columns,
                    "Untitled.txt",
                    false,
                ),
                (
                    PanelMode::Save,
                    true,
                    FileViewMode::List,
                    "Untitled.txt",
                    true,
                ),
                (
                    PanelMode::Save,
                    true,
                    FileViewMode::Columns,
                    "Untitled.txt",
                    false,
                ),
                (
                    PanelMode::Open,
                    true,
                    FileViewMode::Icons,
                    "Untitled.txt",
                    false,
                ),
                (
                    PanelMode::Save,
                    true,
                    FileViewMode::Icons,
                    "Untitled.txt",
                    false,
                ),
                (
                    PanelMode::Save,
                    false,
                    FileViewMode::Columns,
                    "Untitled.txt",
                    false,
                ),
            ];
            specs
                .into_iter()
                .enumerate()
                .map(|(index, (mode, expanded, view, name, extras))| {
                    let mut options = PanelOptions::new(mode, "/".into());
                    options.expanded = expanded;
                    options.expanded_set = true;
                    options.name = name.into();
                    options.allowed_types = vec!["txt".into()];
                    if extras {
                        options.message = Some("Choose a destination for this document.".into());
                        options.accessory = Some(std::rc::Rc::new(|_, _| {
                            Checkbox::new("capture-save-backup", "Keep a backup copy")
                                .size(ControlSize::Small)
                                .checked(false)
                                .into_any_element()
                        }));
                    }
                    let panel = FilePanel::build(options, provider.clone(), window, cx);
                    panel.update(cx, |panel, cx| {
                        let model = panel.model();
                        model.update(cx, |model, cx| model.set_view(view, cx));
                        if index == 0 {
                            panel.select_entry(&SharedString::from("photo"), cx);
                        }
                        if index == 5 {
                            // One selected item in the Open icon grid, on the
                            // first row so its label pill is in frame; the
                            // dimmed PDF sits beside it on the next row.
                            panel.select_entry(&SharedString::from("desktop"), cx);
                        }
                        cx.notify();
                    });
                    panel
                })
                .collect()
        } else {
            Vec::new()
        };

        // Let async provider loads and the panel's Loaded rebuild settle
        // before readback; a single frame captures "Loading…" in the browser.
        let out_a = out.clone();
        window.on_next_frame(move |window, _cx| {
            let out_b = out_a.clone();
            window.on_next_frame(move |window, _cx| {
                let out_c = out_b.clone();
                window.on_next_frame(move |window, cx| {
                    capture(window, cx, &out_c, scale, page);
                    cx.quit();
                });
            });
        });

        Self {
            field,
            list,
            secondary,
            scroll: gpui::ScrollHandle::new(),
            table,
            browser,
            panels,
            menus,
            toolbar_commands,
            content: cx.theme().colors.content,
            row_alternate: cx.theme().colors.row_alternate,
            page,
        }
    }

    fn sample(&self, id: &str, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
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
                    Segment::new("grid", "Grid")
                        .icon(TemplateImage::IconView.role())
                        .show_label(false),
                    Segment::new("list", "List")
                        .icon(TemplateImage::ListView.role())
                        .show_label(false),
                    Segment::new("columns", "Columns")
                        .icon(TemplateImage::ColumnView.role())
                        .show_label(false)
                        .disabled(true),
                ],
            )
            .selected("grid")
            .icon_renderer(silvestris::icons::role_renderer(
                TemplateImageSlot::Segmented,
            ))
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
            "table-view" => self.table.clone().into_any_element(),
            "column-browser" => self.browser.clone().into_any_element(),
            "artwork-file-states" => div()
                .flex()
                .items_center()
                .gap(px(8.))
                .children([
                    FileArtwork::Folder.file_element(16.0, FileIconState::normal()),
                    FileArtwork::Folder.file_element(32.0, FileIconState::normal()),
                    FileArtwork::Folder.file_element(16.0, FileIconState::selected()),
                    FileArtwork::Folder.file_element(32.0, FileIconState::selected()),
                    FileArtwork::Folder.file_element(
                        16.0,
                        FileIconState {
                            inactive: true,
                            ..FileIconState::normal()
                        },
                    ),
                    FileArtwork::Folder.file_element(
                        32.0,
                        FileIconState {
                            inactive: true,
                            ..FileIconState::normal()
                        },
                    ),
                    FileArtwork::Folder.file_element(
                        16.0,
                        FileIconState {
                            disabled: true,
                            ..FileIconState::normal()
                        },
                    ),
                    FileArtwork::Folder.file_element(
                        32.0,
                        FileIconState {
                            disabled: true,
                            ..FileIconState::normal()
                        },
                    ),
                ])
                .into_any_element(),
            "artwork-template-states" => {
                let template_images = [
                    TemplateImage::Back,
                    TemplateImage::Forward,
                    TemplateImage::IconView,
                    TemplateImage::ListView,
                    TemplateImage::ColumnView,
                    TemplateImage::Search,
                    TemplateImage::Stop,
                    TemplateImage::Refresh,
                    TemplateImage::Add,
                    TemplateImage::Remove,
                    TemplateImage::Action,
                    TemplateImage::Info,
                    TemplateImage::Sidebar,
                    TemplateImage::Inspector,
                    TemplateImage::Eject,
                ];
                let theme = cx.theme().clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                Button::new("golden-template-action", "Action")
                                    .style(ButtonStyle::Textured)
                                    .icon(
                                        TemplateImage::Action
                                            .button_icon_renderer(TemplateImageSlot::Toolbar),
                                    ),
                            )
                            .child(
                                Button::new("golden-template-disabled", "Disabled")
                                    .style(ButtonStyle::Textured)
                                    .disabled(true)
                                    .icon(
                                        TemplateImage::Action
                                            .button_icon_renderer(TemplateImageSlot::Toolbar),
                                    ),
                            ),
                    )
                    .children(
                        [
                            theme.colors.text,
                            theme.colors.text_secondary,
                            theme.colors.text_disabled,
                        ]
                        .into_iter()
                        .map(|ink| {
                            div().flex().flex_wrap().gap(px(4.)).children(
                                template_images
                                    .iter()
                                    .map(|image| image.element(TemplateImageSlot::Toolbar, ink)),
                            )
                        }),
                    )
                    .into_any_element()
            }
            "artwork-consumer" => ConsumerArtwork::new("Example application artwork", |_, _| {
                FileArtwork::Application.file_element(32.0, FileIconState::normal())
            })
            .toolbar_element(window, cx),
            "toolbar-rectangular" => Toolbar::new(
                "golden-toolbar-rectangular",
                "Document",
                self.toolbar_commands.clone(),
                vec![
                    ToolbarItem::toggle("golden-toolbar-sidebar", "Sidebar", "golden.sidebar")
                        .image(TemplateImage::Sidebar),
                    ToolbarItem::command("golden-toolbar-action", "Action", "golden.action")
                        .image(TemplateImage::Action),
                    ToolbarItem::command("golden-toolbar-info", "Info", "golden.info")
                        .image(TemplateImage::Info),
                ],
            )
            .display_mode(ToolbarDisplayMode::IconAndLabel)
            .into_any_element(),
            "toolbar-capsule" => Toolbar::new(
                "golden-toolbar-capsule",
                "Document",
                self.toolbar_commands.clone(),
                vec![
                    ToolbarItem::toggle(
                        "golden-toolbar-sidebar-capsule",
                        "Sidebar",
                        "golden.sidebar",
                    )
                    .image(TemplateImage::Sidebar)
                    .style(ToolbarItemStyle::Capsule),
                    ToolbarItem::command(
                        "golden-toolbar-action-capsule",
                        "Action",
                        "golden.action",
                    )
                    .image(TemplateImage::Action)
                    .style(ToolbarItemStyle::Capsule),
                ],
            )
            .display_mode(ToolbarDisplayMode::IconAndLabel)
            .into_any_element(),
            "toolbar-disabled" => Toolbar::new(
                "golden-toolbar-disabled",
                "Document",
                self.toolbar_commands.clone(),
                vec![ToolbarItem::command(
                    "golden-toolbar-disabled-item",
                    "Unavailable",
                    "golden.disabled",
                )
                .image(TemplateImage::Info)],
            )
            .display_mode(ToolbarDisplayMode::IconAndLabel)
            .into_any_element(),
            "toolbar-icon-only" => Toolbar::new(
                "golden-toolbar-icon-only",
                "Document",
                self.toolbar_commands.clone(),
                vec![
                    ToolbarItem::toggle("golden-icon-only-sidebar", "Sidebar", "golden.sidebar")
                        .image(TemplateImage::Sidebar),
                    ToolbarItem::command("golden-icon-only-action", "Action", "golden.action")
                        .image(TemplateImage::Action),
                    ToolbarItem::command("golden-icon-only-info", "Info", "golden.info")
                        .image(TemplateImage::Info),
                ],
            )
            .display_mode(ToolbarDisplayMode::IconOnly)
            .into_any_element(),
            "toolbar-text-only" => Toolbar::new(
                "golden-toolbar-text-only",
                "Document",
                self.toolbar_commands.clone(),
                vec![
                    ToolbarItem::toggle("golden-text-only-sidebar", "Sidebar", "golden.sidebar")
                        .image(TemplateImage::Sidebar),
                    ToolbarItem::command("golden-text-only-action", "Action", "golden.action")
                        .image(TemplateImage::Action),
                    ToolbarItem::command("golden-text-only-info", "Info", "golden.info")
                        .image(TemplateImage::Info),
                ],
            )
            .display_mode(ToolbarDisplayMode::TextOnly)
            .into_any_element(),
            "bottom-bar-regular" => WindowFrame::new("Documents")
                .bottom_bar_component(
                    BottomBar::new(
                        "golden-bottom-bar-regular",
                        "Regular bottom bar",
                        vec![
                            BottomBarItem::status(BottomBarStatus::new(
                                "golden-bottom-status-regular",
                                "12 items, 3 selected",
                            )),
                            BottomBarItem::flexible_space("golden-bottom-gap-regular"),
                            BottomBarItem::command("golden-bottom-add", "Add", "golden.add")
                                .image(TemplateImage::Add)
                                .style(BottomBarControlStyle::Gradient),
                            BottomBarItem::command(
                                "golden-bottom-remove",
                                "Remove",
                                "golden.remove",
                            )
                            .image(TemplateImage::Remove)
                            .style(BottomBarControlStyle::Gradient),
                        ],
                    )
                    .registry(self.toolbar_commands.clone()),
                    cx,
                )
                .child(div().size_full())
                .into_any_element(),
            "bottom-bar-small" => WindowFrame::new("Documents")
                .bottom_bar_component(
                    BottomBar::new(
                        "golden-bottom-bar-small",
                        "Small bottom bar",
                        vec![
                            BottomBarItem::status(BottomBarStatus::new(
                                "golden-bottom-status-small",
                                "12 items",
                            )),
                            BottomBarItem::flexible_space("golden-bottom-gap-small"),
                            BottomBarItem::command("golden-bottom-add-small", "Add", "golden.add")
                                .image(TemplateImage::Add)
                                .style(BottomBarControlStyle::Gradient),
                        ],
                    )
                    .registry(self.toolbar_commands.clone())
                    .size(BottomBarSize::Small),
                    cx,
                )
                .child(div().size_full())
                .into_any_element(),
            "open-dialog-columns" => WindowFrame::new("Open")
                .role(WindowRole::OpenPanel)
                .child(self.panels[0].clone())
                .into_any_element(),
            "open-dialog-list" => WindowFrame::new("Open")
                .role(WindowRole::OpenPanel)
                .child(self.panels[1].clone())
                .into_any_element(),
            "open-dialog-icons" => WindowFrame::new("Open")
                .role(WindowRole::OpenPanel)
                .child(self.panels[5].clone())
                .into_any_element(),
            "save-sheet-minimal" => {
                save_sheet_sample(&self.panels[2], false, cx).into_any_element()
            }
            "save-sheet-expanded" => {
                save_sheet_sample(&self.panels[3], true, cx).into_any_element()
            }
            "save-sheet-expanded-icons" => {
                save_sheet_sample(&self.panels[6], true, cx).into_any_element()
            }
            "save-sheet-grow" => save_sheet_sample(&self.panels[4], true, cx).into_any_element(),
            "save-sheet-minimal-crop" => {
                save_sheet_crop_sample(&self.panels[7], cx).into_any_element()
            }
            "preferences-general" => preferences_sample("General", 4, false, cx).into_any_element(),
            "preferences-scroll" => preferences_sample("Privacy", 30, true, cx).into_any_element(),
            "split-view-thin" => {
                split_sample(SplitOrientation::Vertical, SplitDividerStyle::Thin, cx)
                    .into_any_element()
            }
            "split-view-thick" => {
                split_sample(SplitOrientation::Horizontal, SplitDividerStyle::Thick, cx)
                    .into_any_element()
            }
            "action-menu-scope-bar" => {
                action_scope_sample(&self.toolbar_commands, cx).into_any_element()
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

/// A small document window with the Save sheet attached under its toolbar.
/// The parent is not dimmed; the sheet uses the same chrome as `sheet.rs`.
fn save_sheet_sample(panel: &Entity<FilePanel>, _expanded: bool, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let height = panel.read(cx).preferred_height();
    WindowFrame::new("Untitled")
        .role(WindowRole::Document)
        .toolbar(
            div()
                .flex()
                .items_center()
                .px(px(10.))
                .size_full()
                .text_size(px(theme.type_.size_small))
                .text_color(silvestris::convert::hsla(theme.colors.text_secondary))
                .child("File"),
            32.0,
        )
        .child(
            div()
                .relative()
                .size_full()
                .bg(silvestris::convert::hsla(theme.colors.content))
                .child(
                    div()
                        .p(px(18.))
                        .w(px(360.))
                        .text_size(px(13.))
                        .text_color(silvestris::convert::hsla(theme.colors.text))
                        .child("The quick brown fox jumps over the lazy dog."),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .justify_center()
                        .child(
                            silvestris::sheet::card_shell(
                                silvestris::file_panel::chrome::PANEL_WIDTH,
                                Some(height),
                                cx,
                            )
                            .child(panel.clone()),
                        ),
                ),
        )
}

/// The minimal Save sheet alone, cropped to its own card: no document behind
/// it, so dead vertical space inside the sheet is obvious in review.
fn save_sheet_crop_sample(panel: &Entity<FilePanel>, cx: &mut App) -> impl IntoElement {
    let height = panel.read(cx).preferred_height();
    div()
        .debug_selector(|| "save-sheet-crop".into())
        .flex()
        .justify_center()
        .child(
            silvestris::sheet::card_shell(
                silvestris::file_panel::chrome::PANEL_WIDTH,
                Some(height),
                cx,
            )
            .child(panel.clone()),
        )
}

fn split_sample(
    orientation: SplitOrientation,
    style: SplitDividerStyle,
    cx: &mut App,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let pane = |id: &'static str, label: &'static str, background: gpui::Hsla| {
        div()
            .debug_selector(move || id.into())
            .size_full()
            .bg(background)
            .p(px(8.))
            .text_size(px(theme.type_.size_small))
            .text_color(silvestris::convert::hsla(theme.colors.text_secondary))
            .child(label)
    };
    let source = pane(
        "split-source",
        "Source list",
        silvestris::convert::hsla(theme.colors.sidebar),
    );
    let content = pane(
        "split-content",
        "Content",
        silvestris::convert::hsla(theme.colors.content),
    );
    let inspector = pane(
        "split-inspector",
        "Inspector",
        silvestris::convert::hsla(theme.colors.window),
    );
    let title = pane(
        "split-title",
        "Split view",
        silvestris::convert::hsla(theme.colors.content),
    );
    let log = pane(
        "split-log",
        "Log",
        silvestris::convert::hsla(theme.colors.window),
    );

    if orientation.is_side_by_side() {
        SplitView::new("split-sample", orientation)
            .divider_style(style)
            .state(SplitState::with_dividers([
                ("split-source", 96.0),
                ("split-content", 270.0),
            ]))
            .pane("split-source", SplitPane::new(72.0, 160.0), source)
            .pane(
                "split-content",
                SplitPane::new(120.0, f32::INFINITY),
                content,
            )
            .pane("split-inspector", SplitPane::new(48.0, 160.0), inspector)
    } else {
        SplitView::new("split-sample", orientation)
            .divider_style(style)
            .state(SplitState::with_dividers([("split-title", 112.0)]))
            .pane("split-title", SplitPane::new(64.0, 180.0), title)
            .pane("split-log", SplitPane::new(48.0, f32::INFINITY), log)
    }
}

fn action_scope_sample(registry: &Entity<CommandRegistry>, cx: &App) -> impl IntoElement {
    let toolbar_height = cx.theme().metrics.toolbar_labelled_height;
    let menu = MenuModel {
        label: "Action".into(),
        entries: vec![
            MenuEntry::Command("golden.action".into()),
            MenuEntry::Command("golden.info".into()),
            MenuEntry::Separator,
            MenuEntry::Command("golden.add".into()),
        ],
    };
    div()
        .size_full()
        .p(px(12.))
        .flex()
        .flex_col()
        .items_start()
        .gap(px(12.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ActionMenu::new(
                    "golden-action-menu",
                    "Action",
                    menu.clone(),
                    registry.clone(),
                ))
                .child(CommandPopDown::new(
                    "golden-command-pop-down",
                    "More",
                    menu,
                    registry.clone(),
                )),
        )
        .child(
            ScopeBar::new(
                "golden-scope",
                "Search scope",
                vec![
                    Scope::new("all", "All"),
                    Scope::new("documents", "Documents"),
                    Scope::new("images", "Images"),
                ],
            )
            .caption("Search:")
            .selected("all"),
        )
        .child({
            let items = vec![
                ToolbarItem::command("golden-tb-add", "Add", "golden.add")
                    .image(TemplateImage::Add),
                ToolbarItem::command("golden-tb-remove", "Remove", "golden.remove")
                    .image(TemplateImage::Remove)
                    .optional(true),
                ToolbarItem::command("golden-tb-info", "Info", "golden.info")
                    .image(TemplateImage::Info)
                    .optional(true),
            ];
            let layout = ToolbarLayout::default_for(&items);
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div().w_full().h(px(toolbar_height)).child(
                        Toolbar::new(
                            "golden-custom-toolbar",
                            "Custom",
                            registry.clone(),
                            items.clone(),
                        )
                        .layout(layout.clone())
                        .display_mode(ToolbarDisplayMode::IconAndLabel),
                    ),
                )
                .child(ToolbarPalette::new(
                    "golden-palette",
                    "Customize toolbar",
                    items,
                    layout,
                ))
        })
}

fn preferences_sample(
    title: &'static str,
    rows: usize,
    scrolls: bool,
    cx: &mut App,
) -> impl IntoElement {
    let theme = cx.theme().clone();
    let nav = PreferencesNavigator::new(Some(title.into()));
    let mut form = Form::new(120.0);
    for index in 0..rows {
        form = form.row(FormRow::new(
            format!("Option {index}"),
            Checkbox::new(format!("preferences-option-{index}"), "Enabled")
                .checked(index % 2 == 0)
                .on_change(|_, _, _| {}),
        ));
    }
    if !scrolls {
        form = form
            .row(
                FormRow::new(
                    "Font",
                    Checkbox::new("preferences-font", "Monospaced").checked(true),
                )
                .id("preferences-font-row")
                .error("Unknown font family"),
            )
            .row(
                FormRow::new(
                    "Size",
                    Checkbox::new("preferences-size", "Fit to window").checked(false),
                )
                .id("preferences-size-row")
                .warning("Larger sizes may not fit"),
            );
    }
    WindowFrame::new(title)
        .role(WindowRole::Preferences)
        .fixed_size()
        .toolbar(PreferencesToolbar::new(&nav), theme.metrics.toolbar_height)
        .child(pane_viewport(
            pane_body().child(div().pt(px(34.0)).child(form)),
            scrolls,
        ))
}

impl Render for CaptureView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                    .child(self.sample(id, window, cx)),
            );
        }
        for menu in &self.menus {
            root = root.child(deferred(menu.clone()).priority(1));
        }
        root
    }
}

/// Deterministic provider for the panel captures.
fn capture_file_provider() -> MemoryProvider {
    let provider = MemoryProvider::new();
    provider.add_location(
        "/",
        "Macintosh HD",
        Icon::Volume,
        None,
        vec![
            FileEntry::folder("desktop", "Desktop").modified(1_735_689_600),
            FileEntry::folder("documents", "Documents").modified(1_735_689_600),
            FileEntry::document("readme", "Read Me.txt")
                .size(4096)
                .modified(1_735_603_200),
            FileEntry::document("stacks", "About Stacks.pdf")
                .size(1_048_576)
                .modified(1_735_516_800),
            FileEntry::document("photo", "Photo.png")
                .size(2_097_152)
                .modified(1_735_430_400),
            // Deliberately long so the icon view's label wrapping and its
            // selection pill can be reviewed.
            FileEntry::document("long", "A Deliberately Long Filename That Has To Wrap.txt")
                .size(5_120)
                .modified(1_735_344_000),
        ],
    );
    provider.add_location(
        "/documents",
        "Documents",
        Icon::Documents,
        Some("/".into()),
        vec![
            FileEntry::document("notes", "Notes.txt")
                .size(12_288)
                .modified(1_735_603_200),
            FileEntry::document("report", "Report.pdf")
                .size(892_928)
                .modified(1_735_516_800),
        ],
    );
    for (id, label, icon) in [
        ("/desktop", "Desktop", Icon::Desktop),
        ("/home", "Home", Icon::Home),
        ("/applications", "Applications", Icon::Application),
        ("/downloads", "Downloads", Icon::Documents),
    ] {
        provider.add_location(id, label, icon, Some("/".into()), Vec::new());
    }
    provider.set_places(vec![
        FilePlace::new("/", "Macintosh HD", Icon::Volume, PlaceSection::Devices).eject(true),
        FilePlace::new("/desktop", "Desktop", Icon::Desktop, PlaceSection::Places),
        FilePlace::new("/home", "Home", Icon::Home, PlaceSection::Places),
        FilePlace::new(
            "/documents",
            "Documents",
            Icon::Documents,
            PlaceSection::Places,
        ),
        FilePlace::new(
            "/applications",
            "Applications",
            Icon::Application,
            PlaceSection::Places,
        ),
    ]);
    provider
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
    // The platform may have reset a chosen scale after a DPI change; force
    // the requested scale and redraw before readback.
    window.set_scale_factor(scale);
    window.draw(cx).clear(cx);
    let image = window.render_to_image().expect("frame readback");
    eprintln!(
        "capture page {page}: requested scale {scale}, window scale {}, image {}x{}",
        window.scale_factor(),
        image.width(),
        image.height()
    );
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
