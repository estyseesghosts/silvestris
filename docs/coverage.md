# Coverage matrix

Every HIG component the corpus names, the Silvestris type that provides it,
its status, the appearances it is built for, its golden id and the issue that
tracks the remaining work. The component list follows
`hig-09/notes/component-inventory.md`.

Status:

- `missing`: no Silvestris type; consumers paint it themselves.
- `provisional`: the type exists, but a gallery sample or a test is missing, or
  an open issue replaces the API.
- `functional`: a gallery sample and a test exist.
- `measured`: functional, with goldens and a fidelity issue closed against a
  corpus crop. M2 control rows are `measured` once their goldens are accepted;
  the crop distance is recorded in the golden report.

Rows stay `provisional` when a gallery sample or a scenario test is missing.
The combo box, stepper, notice, scroller and search field appear in the
gallery but still need a scenario test; the list, split, tab and form rows wait
on their tracked APIs. A row is never `measured` without both a golden and a
test.

Appearances `four` means the control is built for all four (Aqua Light, Aqua Dark,
Geist Light, Geist Dark). It is a design statement, not evidence: rendering
evidence is the golden id or a test that iterates appearances.
A golden id names the sample stem under `goldens/renders/<appearance>/<scale>/`.
The matching 10.6 reference is `<id>.<state>` in `reference/crops.json`; see
[docs/goldens.md](goldens.md). `—` means no committed golden.

## Window types

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Document window | `chrome::open_window`, `document::DocumentController` | functional | four | `window-frame` | #20, #37 |
| Application window | `chrome::open_window` | functional | four | — | #20 |
| Panel, inspector | `Shell` inspector, `Sidebar` | functional | four | — | #20, #27 |
| Dialog, alert | `Alert`, `ConfirmationSheet` | functional | four | `save-sheet-minimal` | #21, #23 |
| Sheet | `sheet::Sheet` | functional | four | `save-sheet-minimal`, `save-sheet-expanded` | #22 |
| Open dialog | `WindowRole::OpenPanel`, `OpenPanel::run_modal` | measured | four | `open-dialog-columns`, `open-dialog-list` | #29 |
| Source list, sidebar | `SourceList`, `SourceListEvent`, `SourceListStyle`, `Sidebar` | measured | four | `source-list`, `source-list-secondary` | #3, #4 |
| Source-list row accessories | `RowAccessory`, `RowStatus` | measured | four | `source-list-secondary` | #4 |
| Find window | none | missing | — | — | #77 |
| Preferences window | `PreferencesNavigator`, `PreferencesToolbar`, `PreferencesOverview`, `PreferencePane`, `PreferenceCategory` | provisional | four | — | #14 |

## Menu bar

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Menu bar composition | `menu::native_menus` (macOS only) | missing | — | — | #24 |
| Menu items, state, submenus | `core::command::MenuModel`, `MenuEntry`, `PopupMenu` | measured | four | `menu-arrange-by`, `menu-marks` | #25 |
| Contextual menu | `PopupMenu::open`, `open_pop_down` | measured | four | `menu-arrange-by`, `menu-marks` | #16 |
| Transient surfaces | `TransientLayer`, `TransientHandle`, `CancelToken` | functional | four | — | #60 |

## Controls

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Push button | `Button`, `ButtonStyle`, `ControlSize` | measured | four | `button-default` | #10 |
| Default button throb | `ButtonStyle::Default` | measured | four | `button-default` | #9 |
| Bevel, icon, round, gradient buttons | `ButtonStyle::{Bevel, Round, Recessed, Help}` | measured | four | `button-round`, `button-recessed`, `button-help`, `button-bevel` | #10 |
| Segmented control | `SegmentedControl`, `Segment`, `SegmentMode`, `SegmentStyle` | measured | four | `segmented`, `segmented-textured`, `segmented-icon`, `segmented-small` | #11 |
| Pop-up button | `PopUpButton`, `PopupStyle` | measured | four | `popup-button`, `popup-textured`, `popup-disabled` | #1, #2 |
| Scope buttons | none | missing | — | — | #46 |
| Slider | `Slider`, `Orientation`, `ThumbStyle` | measured | four | `slider`, `slider-pointed`, `slider-vertical`, `slider-disabled` | #7 |
| Stepper | `Stepper` | provisional | four | — | unfiled |
| Combo box | `ComboBox` | provisional | four | — | unfiled |
| Action menu | none | missing | — | — | #46 |
| Command pop-down | none | missing | — | — | #46 |
| Date picker | none | missing | — | — | #42 |
| Path control | `PathControl`, `PathPopUp`, `PathEntry`, `PathEvent`, `LocationId` | functional | four | — | #41 |
| Colour well, image well | none | missing | — | — | #44 |
| Radio button | `RadioGroup`, `RadioOption` | measured | four | `radio-on`, `radio-off`, `radio-small` | #6 |
| Checkbox, mixed state | `Checkbox`, `CheckState` | measured | four | `checkbox-on`, `checkbox-off`, `checkbox-mixed`, `checkbox-small`, `checkbox-mini` | #19 |
| Placard | none | missing | — | — | #107 |

## Indicators and feedback

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Progress indicator | `ProgressIndicator`, `ProgressStyle` | measured | four | `progress`, `progress-indeterminate`, `spinner`, `progress-ring` | #8 |
| Capacity, rating, relevance | none | missing | — | — | #45 |
| Notice, status line | `Notice` | provisional | four | — | unfiled |

## Text and input

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Static text | styled text in composition | missing | — | — | unfiled |
| Text input field | `TextField`, `TextFieldEvent`, `text::EditSession` | functional | four | `text-field` | #58 |
| Multiline text area | `TextArea`, `TextAreaEvent` | functional | four | — | #58 |
| Token field | none | missing | — | — | #43 |
| Search field | `TextField` search variant | provisional | four | — | #77 |
| Table view | `TableView`, `TableColumn`, `TableEvent`, `CellContext`, `ColumnAlign`, `GridLines`, `SortDirection`, `ColumnStore`, `MemoryColumns`, `PreferenceColumns`, `SavedColumns`, `SavedTables` | measured | four | `table-view` | #32 |
| Scrolling list | `controls::table::virtual_table` | provisional | four | — | #32 |
| Scroller and scroll view | `ScrollView`, `ScrollAxes`, `Scrollbar`, `ScrollSource`, `ScrollerPreferences`, `ScrollArrows`, `TrackClick` | measured | four | `scroll-view` | #40 |

## View controls

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Disclosure triangle | `Disclosure`, `DisclosureStyle` | measured | four | `disclosure`, `disclosure-expanded`, `disclosure-button` | #5 |
| Disclosure button | `DisclosureStyle::Button` | measured | four | `disclosure-button` | #5 |
| List view | `virtual_table` | provisional | four | — | #32 |
| Outline view | none | missing | — | — | #33 |
| Column view | `ColumnBrowser`, `BrowserNode`, `BrowserEvent` | functional | four | `column-browser` | #34 |
| Icon view | `IconGrid`, `IconGridEvent` | functional | four | `open-dialog-icons`, `save-sheet-expanded-icons` | #35 |
| Split view | `Shell` divider only | provisional | four | — | #53 |
| Tab view | `TabView`, `preferences::pane_tab_view` | measured | four | `tab-view` | #13 |
| Separator | `Separator`, `SeparatorAxis` | measured | four | `separator` | #13 |
| Group box | `GroupBox` (text, checkbox or pop-up title), `Form` | measured | four | `groupbox` | #13 |
| Recessed well | `Well` | measured | four | `well` | #13 |

## Window frame and layout

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Window frame | `WindowFrame`, `WindowRole`, `RoleSpec`, `FrameCapabilities` | functional | four | `window-frame` | #20 |
| Toolbar | none | missing | — | — | #26 |
| Bottom bar | none | missing | — | — | #75 |
| Shell layout | `Shell`, `ShellContent` | functional | four | — | #13 |
| Forms | `Form`, `FormRow` | provisional | four | — | #54 |

## Dialogs and sheets

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Alert | `Alert`, `AlertButton`, `AlertResult`, `AlertRole` | functional | four | `save-sheet-minimal` (replace alert in modal window is behavioural) | #23 |
| Sheet | `sheet::Sheet`, `sheet::present`, `sheet::dismiss`, `sheet::overlay`, `sheet::card_shell` | functional | four | `save-sheet-minimal`, `save-sheet-expanded`, `save-sheet-grow` | #22 |
| Open, Save, Choose | `SavePanel`, `OpenPanel`, `FilePanel`, `FileProvider`, `MemoryProvider`, `StdFsProvider`, `FileBrowserModel`, `FileEntry`, `FileSort`, `FileSortKey`, `FileMetadata`, `FileListing`, `FileProviderCapabilities`, `SaveDisposition`, `PanelStateStore` | functional | four | `open-dialog-columns`, `open-dialog-list`, `save-sheet-minimal`, `save-sheet-expanded`, `save-sheet-grow` | #29 |
| Print, Page Setup | none | missing | — | — | #47 |
| About | none | missing | — | — | #47 |
| Progress copy window | none | missing | — | — | #47 |

## Interaction and artwork

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Focus and responder routing | `ResponderChain`, `Responder`, `ResponderEntry`, `FocusReturn`, `CommandScope` | functional | four | — | #57 |
| Model bindings and list state | `binding::Binding`, `TextBinding`, `ListController` | functional | four | — | #61 |
| Device-pixel snapping | `paint::snap_edge`, `paint::snap_stroke_center`, `paint::snap_bounds`, `convert::hairline` | functional | four | `segmented` | #12 |
| Keyboard shortcuts | `CommandRegistry::bindings`, `bindings_where`, `rebind`, `apply_bindings`, `ShortcutRecorder` | functional | four | — | #57, #66 |
| Application lifecycle | `lifecycle::AppDelegate`, `QuitCoordinator`, `Terminate` | functional | — | — | #59 |
| Preference storage | `silvestris_core::prefs::{Store, Versioned, Migration, Storage}`, `silvestris::prefs::Preferences` | functional | — | — | #62 |
| Modal sessions | `modal::ModalSession`, `ModalScope`, `ModalResult`; `ConfirmationFocus` | functional | four | — | #21 |
| Drag and drop | none | missing | — | — | #36 |
| Undo and redo | `undo::UndoManager`, `TextHistory`, `undo::entry` | functional | four | — | #39 |
| System images and icons | `sprites`; `icons::FileArtwork`, `TemplateImage`, `ConsumerArtwork` | functional | four | `artwork-file-states`, `artwork-template-states`, `artwork-consumer` | #48 |
| Cursors | GPUI cursor styles | provisional | four | — | #48 |

## Active, inactive and focus audit

Issue #17 checks each control in the key window with focus on the control, the
key window with focus elsewhere, and an inactive window. The control reads the
window state at paint time; interaction code never branches on theme.

| Control | Key window, focused | Key window, focus elsewhere | Inactive window | Evidence |
| --- | --- | --- | --- | --- |
| Source list selection | blue gradient, white label | muted blue gradient (`source_unfocused`) | grey gradient (`source_inactive`) | `SourceList` test + `source-list` golden |
| Table selection | blue flat | blue flat | grey flat | `Lists::table_*` tokens |
| Title bar and traffic lights | active chrome and coloured lights | active | inactive chrome and idle lights | window-frame goldens |
| Push/default button | normal/default | normal/default, throb stopped | dimmed material | `Button` throb guard + `button` goldens |
| Pop-up | normal | normal | dimmed body/cap | `popup-disabled` and popup tokens |
| Source-list disclosure | secondary ink or white on selection | same | disabled ink | `disclosure` goldens |
| Slider | normal track/thumb | normal | disabled track/thumb | `slider-disabled` golden |
| Progress | animates | animates | frozen | `ProgressIndicator` reduced-motion test |
| Segmented, radio, checkbox | normal | normal | normal material, disabled ink | per-control theme tests |

Panels (#27) keep their owning document active-looking; the audit treats a
panel's non-key state as inactive for its own chrome only.

## Appearance evidence

Every M2 sample is rendered in Aqua Light, Aqua Dark, Geist Light and Geist
Dark at 100, 125, 150, 175 and 200 percent. `goldens/manifest.json` is the
machine-readable matrix; `crates/silvestris/tests/m2_conformance.rs` fails
when an appearance or scale is missing. Dark Aqua is a designed appearance
(10.6 had no dark mode); Geist Light and Dark are designed on the Geist role
scale. Their deviations are recorded in [exceptions.md](exceptions.md).



`crates/silvestris/tests/coverage.rs` fails when a public type in
`crates/silvestris/src/controls/` has no row here. Add the row with the same
commit that adds the control. A row is `functional` only when the gallery
shows the control and a test covers its behaviour. A control that exists but
is absent from the gallery, or has no test, stays `provisional`, which is a
reminder rather than a claim of completeness.
