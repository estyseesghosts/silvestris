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
  corpus crop. No row is `measured` yet; the M2 fidelity issues are open.

Five rows are `provisional` only because no test covers them although the
gallery shows them: radio button, combo box, progress indicator, disclosure
triangle and the search field. The tracking issue is that test. The other
`provisional` rows lack a gallery sample or wait on an API replacement.

Appearances `four` means the control is built for all four (Aqua Light, Aqua Dark,
Geist Light, Geist Dark). It is a design statement, not evidence: rendering
evidence is the golden id or a test that iterates appearances.
A golden id names the sample stem under `goldens/renders/<appearance>/<scale>/`.
The matching 10.6 reference is `<id>.<state>` in `reference/crops.json`; see
[docs/goldens.md](goldens.md). `—` means no committed golden.

## Window types

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Document window | `chrome::open_window` | functional | four | — | #20 |
| Application window | `chrome::open_window` | functional | four | — | #20 |
| Panel, inspector | `Shell` inspector, `Sidebar` | functional | four | — | #20, #27 |
| Dialog, alert | `ConfirmationSheet` | provisional | four | — | #21, #23 |
| Sheet | none | missing | — | — | #22 |
| Source list, sidebar | `SourceList`, `SourceListEvent`, `Sidebar` | functional | four | `source-list` | #3, #4 |
| Find window | none | missing | — | — | #77 |
| Preferences window | `PreferencesNavigator`, `PreferencesToolbar`, `PreferencesOverview`, `PreferencePane`, `PreferenceCategory` | provisional | four | — | #14 |

## Menu bar

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Menu bar composition | `menu::native_menus` (macOS only) | missing | — | — | #24 |
| Menu items, state, submenus | `core::command::MenuModel`, `MenuEntry`, `PopupMenu` | functional | four | — | #16, #25 |
| Contextual menu | `PopupMenu` | functional | four | — | #16 |
| Transient surfaces | `TransientLayer`, `TransientHandle`, `CancelToken` | functional | four | — | #60 |

## Controls

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Push button | `Button`, `ButtonStyle`, `ControlSize` | functional | four | `button` | #10 |
| Default button throb | `ButtonStyle::Default` | provisional | four | — | #9 |
| Bevel, icon, round, gradient buttons | none | missing | — | — | #26, #48, #75 |
| Segmented control | `SegmentedControl`, `Segment` | functional | four | `segmented` | #11 |
| Pop-up button | `PopUpButton` | functional | four | `popup-button` | #1, #2 |
| Scope buttons | none | missing | — | — | #46 |
| Slider | `Slider` | functional | four | `slider` | #7 |
| Stepper | `Stepper` | provisional | four | — | unfiled |
| Combo box | `ComboBox` | provisional | four | — | unfiled |
| Action menu | none | missing | — | — | #46 |
| Command pop-down | none | missing | — | — | #46 |
| Date picker | none | missing | — | — | #42 |
| Path control | none | missing | — | — | #41 |
| Colour well, image well | none | missing | — | — | #44 |
| Radio button | `RadioGroup`, `RadioOption` | provisional | four | `radio` | #6 |
| Checkbox, mixed state | `Checkbox`, `CheckState` | functional | four | `checkbox` | #19 |
| Placard | none | missing | — | — | #13 |

## Indicators and feedback

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Progress indicator | `ProgressIndicator` | provisional | four | `progress` | #8 |
| Capacity, rating, relevance | none | missing | — | — | #45 |
| Notice, status line | `Notice` | provisional | four | — | unfiled |

## Text and input

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Static text | styled text in composition | missing | — | — | unfiled |
| Text input field | `TextField`, `TextFieldEvent` | functional | four | `text-field` | #58 |
| Token field | none | missing | — | — | #43 |
| Search field | `TextField` search variant | provisional | four | — | #77 |
| Scrolling list | `controls::table::virtual_table` | provisional | four | — | #32 |
| Scroller | `Scrollbar` | provisional | four | — | #40 |

## View controls

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Disclosure triangle | `Disclosure` | provisional | four | `disclosure` | #5 |
| Disclosure button | none | missing | — | — | #5 |
| List view | `virtual_table` | provisional | four | — | #32 |
| Outline view | none | missing | — | — | #33 |
| Column view | none | missing | — | — | #34 |
| Split view | `Shell` divider only | provisional | four | — | #53 |
| Tab view | `TabView` | provisional | four | — | #13 |
| Separator | painted in layout, no type | provisional | four | — | #13 |
| Group box | `TabView`, `Form` | provisional | four | — | #13 |

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
| Alert | none | missing | — | — | #23 |
| Open, Save, Choose | none | missing | — | — | #29 |
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
| Preference storage | `silvestris_core::prefs::{Store, Versioned, Migration, Storage}`, `silvestris::prefs::Preferences` | functional | — | — | #62 |
| Modal sessions | `modal::ModalSession`, `ModalScope`, `ModalResult`; `ConfirmationFocus` | functional | four | — | #21 |
| Drag and drop | none | missing | — | — | #36 |
| Undo | none | missing | — | — | #39 |
| System images and icons | `sprites` | provisional | four | — | #48 |
| Cursors | GPUI cursor styles | provisional | four | — | #48 |

## Keeping the matrix current

`crates/silvestris/tests/coverage.rs` fails when a public type in
`crates/silvestris/src/controls/` has no row here. Add the row with the same
commit that adds the control. A row is `functional` only when the gallery
shows the control and a test covers its behaviour. A control that exists but
is absent from the gallery, or has no test, stays `provisional`, which is a
reminder rather than a claim of completeness.
