//! Reusable controls (#128). Each reads the active theme and cites its HIG
//! or handbook contract.
pub mod button;
pub mod checkbox;
pub mod container;
pub mod disclosure;
pub mod source_list;
pub mod text_area;
pub mod text_field;

pub use button::{Button, ButtonStyle, ControlSize};
pub use checkbox::{CheckState, Checkbox};
pub use container::{GroupBox, Separator, SeparatorAxis, Well};
pub use disclosure::{Disclosure, DisclosureStyle};
pub use source_list::{RowAccessory, RowStatus, SourceList, SourceListEvent, SourceListStyle};
pub use text_area::{TextArea, TextAreaEvent};
pub use text_field::{TextField, TextFieldEvent};

pub mod segmented;
pub use segmented::{Segment, SegmentMode, SegmentStyle, SegmentedControl};
pub mod tab_view;
pub use tab_view::TabView;

pub mod popup_button;
pub use popup_button::{PopUpButton, PopupStyle};
pub mod path_control;
pub use path_control::{PathControl, PathEntry, PathEvent, PathPopUp};
pub mod stepper;
pub use stepper::Stepper;
pub mod scroll_source;
pub mod scroll_view;
pub mod scrollbar;
pub use scroll_source::ScrollSource;
pub use scroll_view::{ScrollAxes, ScrollView};
pub use scrollbar::{ScrollArrows, Scrollbar, ScrollerPreferences, TrackClick};

pub mod tooltip;

pub mod progress;
pub use progress::{ProgressIndicator, ProgressStyle};
pub mod column_browser;
pub mod column_store;
pub mod table;
pub mod table_view;
pub use column_browser::{
    BrowserEvent, BrowserNode, ColumnBrowser, COLUMN_WIDTH as BROWSER_COLUMN_WIDTH,
    ROOT as BROWSER_ROOT,
};
pub use column_store::{ColumnStore, MemoryColumns, PreferenceColumns, SavedColumns, SavedTables};
pub use table_view::{
    icon_text, CellContext, ColumnAlign, GridLines, SortDirection, TableColumn, TableEvent,
    TableView,
};

pub mod radio;
pub use radio::{RadioGroup, RadioOption};
pub mod slider;
pub use slider::{Orientation, Slider, ThumbStyle};

pub mod notice;
pub use notice::Notice;
pub mod status;

pub mod combo_box;
pub use combo_box::ComboBox;

pub mod confirmation;
pub use confirmation::{ConfirmationFocus, ConfirmationSheet};
pub mod alert;
pub use alert::{Alert, AlertButton, AlertResult, AlertRole};

pub mod shortcut_recorder;
pub use shortcut_recorder::ShortcutRecorder;
