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
//! The Save sheet rows and their heights are measured from the 10.6 captures
//! (`M: panel.save.*`); the constants below state each one.

use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use silvestris_core::Rgba8;

use super::super::controls::text_field::TextField;
use super::super::controls::{
    path_control::{PathEntry, PathPopUp},
    ControlSize, Segment, SegmentMode, SegmentStyle, SegmentedControl,
};
use super::super::icons::{self, Icon, TemplateImage, TemplateImageSlot};
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

// Save sheet (M: panel.save.*). Measured from the three 10.6 captures in
// `hig-09/vm-screenshots/sl-archive/dialogs/` (`dialogs--save-sheet--*`), which
// are 1x window captures of a 1440 x 900 desktop.
//
// The collapsed sheet is 431 x 185 (x 385-815, y 88-274) and the expanded sheet
// 541 x 399 (x 330-870, y 88-488). The Save As field keeps the same absolute
// screen x in both states; the wider sheet only adds a centred gutter, so the
// label column, the field, the disclosure and the Where pop-up never move
// between states.

/// The Save sheet keeps the Open window's 527 px outer width. 10.6 Save sheets
/// are narrower (431 collapsed, 541 expanded) because AppKit sizes them to
/// their content; Silvestris presents them in the same modal card as Open, so
/// the rows below are laid out in one fixed inner column centred in that card.
/// `docs/file-panel-comparison.md` records the difference.
pub const SAVE_SHEET_WIDTH: f32 = PANEL_WIDTH;
/// The shared label/field column. 10.6: the collapsed sheet's 431 px minus
/// both 20 px insets.
pub const SAVE_COLUMN_WIDTH: f32 = 391.0;
/// Sheet side inset for the name, Where and bottom rows (10.6: the sheet starts
/// at x 330 and the bottom row at 350).
pub const SAVE_INSET: f32 = 20.0;
/// Sheet content top to the Save As field (10.6: sheet 88, field element 104).
pub const SAVE_TOP_PAD: f32 = 16.0;
/// Right-aligned label column, colon included. 10.6: `Save As:` 419-467 and
/// `Where:` 427-467 in a sheet at 385, so both end 82 px in; 20 px inset plus
/// this column.
pub const SAVE_LABEL_WIDTH: f32 = 63.0;
/// Gap between a label's colon and its control (10.6: colon 467, field element
/// 474; HIG p. 322-325 asks for 8 px, the capture shows 6-7).
pub const SAVE_LABEL_GAP: f32 = 6.0;
/// One regular control row (M: field_height). 10.6: 104-125.
pub const SAVE_NAME_HEIGHT: f32 = 22.0;
/// Save As field width. 10.6: element 474-725, its focus ring 472-727.
pub const SAVE_NAME_WIDTH: f32 = 252.0;
/// Gap between the Save As field and its disclosure button. 10.6: field ring
/// ends at 727, the button bezel starts at 733.
pub const SAVE_DISCLOSURE_GAP: f32 = 6.0;
/// Gap between the field row and the Where row (10.6: element ends 125, Where
/// element starts 141).
pub const SAVE_ROW_GAP: f32 = 15.0;
/// Where pop-up width. 10.6: element 474-725, the same left edge as the field
/// and the same right edge as the disclosure button.
pub const SAVE_WHERE_WIDTH: f32 = 252.0;
/// Gap between the Where row and the Format group box (10.6: element ends 162,
/// box border 174).
pub const SAVE_FORMAT_TOP_GAP: f32 = 11.0;
/// File Format group box height, border included (10.6: 174-227 collapsed and
/// 381-434 expanded, 54 px in both).
pub const SAVE_FORMAT_BOX_HEIGHT: f32 = 54.0;
/// Horizontal inset of the Format group box (10.6: box 400-801 in a sheet at
/// 385-815, so 15 px each side).
pub const SAVE_BOX_INSET: f32 = 15.0;
/// Format label column inside the box, right-aligned on the colon (10.6: ink
/// 422-490 inside a box starting at 401).
pub const SAVE_FORMAT_LABEL_WIDTH: f32 = 70.0;
/// Gap between the Format label and its pop-up (10.6: colon 490, pop-up 504).
pub const SAVE_FORMAT_GAP: f32 = 14.0;
/// Format pop-up width (10.6: bezel 504-781).
pub const SAVE_FORMAT_WIDTH: f32 = 278.0;
/// Gap from the Hide extension checkbox to New Folder (10.6: ink ends 461,
/// New Folder bezel 481).
pub const SAVE_NEW_FOLDER_GAP: f32 = 20.0;
/// Gap between Cancel and Save (HIG, Push Buttons p. 265: at least 12 px;
/// 10.6: Cancel ends 762, Save starts 775).
pub const SAVE_BUTTON_GAP: f32 = 12.0;
/// Gap between the Format box and the bottom row (10.6: box 227, buttons at
/// 241).
pub const SAVE_BOTTOM_GAP: f32 = 13.0;
/// Sheet bottom pad under the buttons (10.6: buttons end 262, content ends
/// 273).
pub const SAVE_BOTTOM_PAD: f32 = 11.0;
/// Save-specific bottom row, border box: its top gap, one regular button, its
/// bottom pad (10.6: 227 to 273).
pub const SAVE_BOTTOM_ROW_HEIGHT: f32 = SAVE_BOTTOM_GAP + SAVE_NAME_HEIGHT + SAVE_BOTTOM_PAD;
/// Gap between the field row and the chooser in expanded Save (10.6: field
/// element ends 125, toolbar separator 141).
pub const SAVE_NAME_TO_CHOOSER: f32 = 16.0;
/// Browser height inside the chooser (10.6: 185-370).
pub const SAVE_BROWSER_HEIGHT: f32 = 186.0;
/// Grip band below the buttons in expanded Save, where the resize corner
/// lives (10.6: buttons end 468, content ends 487, 13 px of pad against 12 in
/// the minimal sheet); the minimal sheet has no grip.
pub const SAVE_GRIP_PAD: f32 = 6.0;

/// Save As row: top pad plus the field.
pub const SAVE_NAME_ROW_HEIGHT: f32 = SAVE_TOP_PAD + SAVE_NAME_HEIGHT;
/// Where row: the gap above plus the pop-up.
pub const SAVE_WHERE_ROW_HEIGHT: f32 = SAVE_ROW_GAP + SAVE_NAME_HEIGHT;
/// The explorer block: toolbar plus browser (10.6: 142-370).
pub const SAVE_EXPLORER_HEIGHT: f32 = TOOLBAR_HEIGHT + SAVE_BROWSER_HEIGHT;
/// The Format row: the gap above the box plus the box itself.
pub const SAVE_FORMAT_ROW_HEIGHT: f32 = SAVE_FORMAT_TOP_GAP + SAVE_FORMAT_BOX_HEIGHT;
/// The folding chooser: its top gap, the explorer, and nothing else. It grows
/// from 0 while the Where row shrinks to 0 (10.6: field row 126 to the browser
/// bottom 370).
pub const SAVE_CHOOSER_HEIGHT: f32 = SAVE_NAME_TO_CHOOSER + SAVE_EXPLORER_HEIGHT;
/// Minimal Save: Save As, Where, Format and the buttons, all measured rows.
pub const SAVE_COLLAPSED_HEIGHT: f32 =
    SAVE_NAME_ROW_HEIGHT + SAVE_WHERE_ROW_HEIGHT + SAVE_FORMAT_ROW_HEIGHT + SAVE_BOTTOM_ROW_HEIGHT;
/// Expanded Save: Save As, the chooser (which replaces the Where row), Format,
/// the buttons and the grip band.
pub const SAVE_EXPANDED_HEIGHT: f32 = SAVE_NAME_ROW_HEIGHT
    + SAVE_CHOOSER_HEIGHT
    + SAVE_FORMAT_ROW_HEIGHT
    + SAVE_BOTTOM_ROW_HEIGHT
    + SAVE_GRIP_PAD;

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
                    .icon(TemplateImage::Back.role())
                    .show_label(false)
                    .disabled(!self.can_go_back),
                Segment::new("forward", "Forward")
                    .icon(TemplateImage::Forward.role())
                    .show_label(false)
                    .disabled(!self.can_go_forward),
            ],
        )
        .mode(SegmentMode::Momentary)
        .style(SegmentStyle::Textured)
        .size(ControlSize::Regular)
        .min_segment_width(21.0)
        .icon_renderer(icons::role_renderer(TemplateImageSlot::Segmented))
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
                    .icon(TemplateImage::IconView.role())
                    .show_label(false),
                Segment::new("list", "List")
                    .icon(TemplateImage::ListView.role())
                    .show_label(false),
                Segment::new("columns", "Columns")
                    .icon(TemplateImage::ColumnView.role())
                    .show_label(false),
            ],
        )
        .selected(selected_view)
        .mode(SegmentMode::SelectOne)
        .style(SegmentStyle::Textured)
        .size(ControlSize::Regular)
        .min_segment_width(23.0)
        .icon_renderer(icons::role_renderer(TemplateImageSlot::Segmented))
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

/// Kept for callers that explicitly want a monochrome template glyph.
pub fn glyph_element(image: TemplateImage, color: Rgba8) -> AnyElement {
    image.element(TemplateImageSlot::Segmented, color)
}
