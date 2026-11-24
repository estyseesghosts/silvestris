//! The Open and Save panel toolbar: history, view mode, path pop-up and
//! search field (HIG p. 245; the 4.15.47 AM Open capture).
//!
//! Geometry from that capture: the panel is 527 x 398 px, its toolbar 30 px,
//! the sidebar 157 px and the button row 37 px. The collapsed Save panel is
//! 527 x 160 (the browser and sidebar are hidden); the exact collapsed height
//! is an estimate because the Save captures do not include a window edge, and
//! is logged in `docs/exceptions.md`.

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

pub const PANEL_WIDTH: f32 = 527.0;
pub const PANEL_HEIGHT: f32 = 398.0;
pub const COLLAPSED_HEIGHT: f32 = 160.0;
pub const TOOLBAR_HEIGHT: f32 = 30.0;
pub const SIDEBAR_WIDTH: f32 = 157.0;
pub const BUTTON_ROW_HEIGHT: f32 = 37.0;
pub const SEARCH_WIDTH: f32 = 180.0;

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
        .size(ControlSize::Small)
        .min_segment_width(28.0)
        .icon_renderer(icons::role_renderer(12.0))
        .on_select(move |id, window, cx| match id.as_str() {
            "back" => back(window, cx),
            "forward" => forward(window, cx),
            _ => {}
        });
        let modes = SegmentedControl::new(
            "panel-views",
            "View",
            vec![
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
        .size(ControlSize::Small)
        .min_segment_width(28.0)
        .icon_renderer(icons::role_renderer(12.0))
        .on_select(move |id, window, cx| {
            view(
                if id.as_str() == "columns" {
                    FileViewMode::Columns
                } else {
                    FileViewMode::List
                },
                window,
                cx,
            );
        });
        div()
            .id("panel-toolbar")
            .debug_selector(|| "panel-toolbar".into())
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(TOOLBAR_HEIGHT))
            .px(px(8.))
            .child(history)
            .child(modes)
            .child(
                div().flex_1().min_w(px(120.)).child(
                    PathPopUp::new("panel-path", self.current)
                        .ancestors(self.ancestors)
                        .recent(self.recent)
                        .on_navigate(move |id, window, cx| navigate(id, window, cx)),
                ),
            )
            .child(div().flex_none().w(px(SEARCH_WIDTH)).child(self.search))
            .into_any_element()
    }
}

/// The colored icon element helper used by table cells and the sidebar. It
/// takes the text color from the caller so a selected row stays legible.
pub fn icon_element(icon: Icon, color: Rgba8) -> AnyElement {
    icon.element(16.0, color)
}
