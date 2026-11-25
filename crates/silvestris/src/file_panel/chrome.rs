//! The Open and Save panel toolbar: history, view mode, path pop-up and
//! search field (HIG p. 245; the 4.15.47 AM Open capture).
//!
//! Measurements (M: panel.outer, panel.geometry), all at 1x:
//!
//! - outer window 527 x 398, of which the title band is 22 px; the panel
//!   content is 376 px.
//! - toolbar 42 px; sidebar 160 px to the divider; button row 46 px.
//! - history pair 43 px, view selector 69 px with three 22-23 px segments,
//!   path pop-up 219 px, search field 108 px; controls 22 rows.
//!
//! The collapsed Save height is still an estimate until 10.6 Save-sheet
//! captures exist; it is logged in `docs/exceptions.md`.

use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use silvestris_core::Rgba8;

use super::super::controls::text_field::TextField;
use super::super::controls::{
    path_control::{PathEntry, PathPopUp},
    ControlSize, Segment, SegmentMode, SegmentStyle, SegmentedControl,
};
use super::super::icons::{self, Icon};
use crate::file_panel::FileViewMode;

/// Outer width, including the title band (M: panel.outer).
pub const PANEL_WIDTH: f32 = 527.0;
/// Outer height, including the 22 px title band (M: panel.outer).
pub const PANEL_HEIGHT: f32 = 398.0;
/// Title band inside the outer height (M: panel.outer, y 37-56 at 1x).
pub const TITLEBAR_HEIGHT: f32 = 22.0;
/// Content height: outer minus the title band. The panel fills this in an
/// Open window and the sheet uses it as its content height.
pub const PANEL_CONTENT_HEIGHT: f32 = PANEL_HEIGHT - TITLEBAR_HEIGHT;
/// Minimal Save height. Estimate: no 10.6 Save-sheet capture exists
/// (`docs/exceptions.md`). This is the sheet's content height.
pub const COLLAPSED_HEIGHT: f32 = 160.0;
/// Optional message line above Save As, measured from the NSSavePanel
/// references (`docs/file-panel-comparison.md`).
pub const MESSAGE_HEIGHT: f32 = 30.0;
/// Inset accessory box plus its margins, measured from the NSSavePanel
/// references.
pub const ACCESSORY_HEIGHT: f32 = 88.0;
/// Toolbar row (M: panel.outer, y 57-98).
pub const TOOLBAR_HEIGHT: f32 = 42.0;
/// Sidebar plus its scroller, to the divider (M: panel.geometry).
pub const SIDEBAR_WIDTH: f32 = 160.0;
/// Button row (M: panel.outer, y 389-434).
pub const BUTTON_ROW_HEIGHT: f32 = 46.0;
/// Search field width (M: panel.geometry).
pub const SEARCH_WIDTH: f32 = 108.0;
/// History pair width (M: panel.geometry).
pub const HISTORY_WIDTH: f32 = 43.0;
/// View selector width, three segments (M: panel.geometry).
pub const VIEW_WIDTH: f32 = 69.0;
/// Path pop-up width (M: panel.geometry).
pub const PATH_WIDTH: f32 = 219.0;

type Action = Rc<dyn Fn(&mut Window, &mut App)>;
type ViewAction = Rc<dyn Fn(FileViewMode, &mut Window, &mut App)>;
type NavigateAction = Rc<dyn Fn(&SharedString, &mut Window, &mut App)>;

/// One toolbar row. The panel owns the model; this element only draws the
/// controls and reports the three actions the toolbar can take.
#[derive(gpui::IntoElement)]
pub struct PanelToolbar {
    pub view: FileViewMode,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub current: PathEntry,
    pub ancestors: Vec<PathEntry>,
    pub recent: Vec<PathEntry>,
    pub search: Entity<TextField>,
    pub on_back: Action,
    pub on_forward: Action,
    pub on_view: ViewAction,
    pub on_navigate: NavigateAction,
}

impl gpui::RenderOnce for PanelToolbar {
    fn render(self, _: &mut Window, _: &mut App) -> impl gpui::IntoElement {
        let back = self.on_back;
        let forward = self.on_forward;
        let view = self.on_view;
        let navigate = self.on_navigate;
        let selected_view = match self.view {
            FileViewMode::Icons => "icons",
            FileViewMode::List => "list",
            FileViewMode::Columns => "columns",
        };
        let history = SegmentedControl::new(
            "panel-history",
            "History",
            vec![
                Segment::new("back", "Back")
                    .icon(Icon::Back.role())
                    .show_label(false)
                    .disabled(!self.can_go_back),
                Segment::new("forward", "Forward")
                    .icon(Icon::Forward.role())
                    .show_label(false)
                    .disabled(!self.can_go_forward),
            ],
        )
        .mode(SegmentMode::Momentary)
        .style(SegmentStyle::Textured)
        .size(ControlSize::Regular)
        .min_segment_width(21.0)
        .icon_renderer(icons::role_renderer(13.0))
        .on_select(move |id, window, cx| match id.as_str() {
            "back" => back(window, cx),
            "forward" => forward(window, cx),
            _ => {}
        });
        let modes = SegmentedControl::new(
            "panel-views",
            "View",
            vec![
                Segment::new("icons", "Icons")
                    .icon(Icon::IconView.role())
                    .show_label(false),
                Segment::new("list", "List")
                    .icon(Icon::List.role())
                    .show_label(false),
                Segment::new("columns", "Columns")
                    .icon(Icon::Columns.role())
                    .show_label(false),
            ],
        )
        .selected(selected_view)
        .mode(SegmentMode::SelectOne)
        .style(SegmentStyle::Textured)
        .size(ControlSize::Regular)
        .min_segment_width(23.0)
        .icon_renderer(icons::role_renderer(13.0))
        .on_select(move |id, window, cx| {
            let mode = match id.as_str() {
                "icons" => FileViewMode::Icons,
                "columns" => FileViewMode::Columns,
                _ => FileViewMode::List,
            };
            view(mode, window, cx);
        });
        div()
            .id("panel-toolbar")
            .debug_selector(|| "panel-toolbar".into())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(20.))
            .h(px(TOOLBAR_HEIGHT))
            .pl(px(15.))
            .pr(px(10.))
            .child(history)
            .child(modes)
            .child(
                div().flex_none().w(px(PATH_WIDTH)).child(
                    PathPopUp::new("panel-path", self.current)
                        .ancestors(self.ancestors)
                        .recent(self.recent)
                        .width(PATH_WIDTH)
                        .on_navigate(move |id, window, cx| navigate(id, window, cx)),
                ),
            )
            .child(div().flex_none().w(px(SEARCH_WIDTH)).child(self.search))
            .into_any_element()
    }
}

/// The full-colour icon element helper used by table cells and the sidebar.
pub fn icon_element(icon: Icon, state: icons::FileIconState) -> AnyElement {
    icon.file_element(16.0, state)
}

/// Kept for callers that explicitly want a monochrome glyph.
pub fn glyph_element(icon: Icon, color: Rgba8) -> AnyElement {
    icon.element(16.0, color)
}
