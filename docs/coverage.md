# Coverage matrix

Every HIG component the corpus names, the Silvestris type that provides it,
its status, the appearances it renders in, its golden id and the issue that
tracks the remaining work. The component list follows
`hig-09/notes/component-inventory.md`.

Status:

- `missing`: no Silvestris type; consumers paint it themselves.
- `provisional`: implemented and tested, but no gallery sample, or an open
  issue replaces the API.
- `functional`: a gallery sample and a test exist.
- `measured`: functional, with goldens and a fidelity issue closed against a
  corpus crop. Nothing reaches this status before #18 lands.

Appearances `four` means Aqua Light, Aqua Dark, Geist Light and Geist Dark.
`—` in the golden column means no committed golden yet; #18 owns the gate.

## Window types

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Document window | `chrome::open_window`, `WindowFrame` | functional | four | — | #20 |
| Application window | `chrome::open_window`, `WindowFrame` | functional | four | — | #20 |
| Panel, inspector | `Shell` inspector, `Sidebar` | functional | four | — | #20, #27 |
| Dialog, alert | `ConfirmationSheet` | provisional | four | — | #21, #23 |
| Sheet | none | missing | — | — | #22 |
| Source list, sidebar | `SourceList`, `SourceListEvent`, `Sidebar` | functional | four | — | #3, #4 |
| Find window | none | missing | — | — | #77 |
| Preferences window | `PreferencesNavigator`, `PreferencesToolbar`, `PreferencesOverview`, `PreferencePane`, `PreferenceCategory` | provisional | four | — | #14 |

## Menu bar

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Menu bar composition | `menu::native_menus` (macOS only) | missing | — | — | #24 |
| Menu items, state, submenus | `core::command::MenuModel`, `MenuEntry`, `PopupMenu` | functional | four | — | #16, #25 |
| Contextual menu | `PopupMenu` | functional | four | — | #16 |

## Controls

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Push button | `Button`, `ButtonStyle`, `ControlSize` | functional | four | — | #10 |
| Default button throb | `ButtonStyle::Default` | provisional | four | — | #9 |
| Bevel, icon, round, gradient buttons | none | missing | — | — | #26, #48, #75 |
| Segmented control | `SegmentedControl`, `Segment` | functional | four | — | #11 |
| Pop-up button | `PopUpButton` | provisional | four | — | #1, #2 |
| Scope buttons | none | missing | — | — | #46 |
| Slider | `Slider` | functional | four | — | #7 |
| Stepper | `Stepper` | provisional | four | — | unfiled |
| Combo box | `ComboBox` | functional | four | — | unfiled |
| Action menu | none | missing | — | — | #46 |
| Command pop-down | none | missing | — | — | #46 |
| Date picker | none | missing | — | — | #42 |
| Path control | none | missing | — | — | #41 |
| Colour well, image well | none | missing | — | — | #44 |
| Radio button | `RadioGroup`, `RadioOption` | functional | four | — | #6 |
| Checkbox, mixed state | `Checkbox`, `CheckState` | functional | four | — | #19 |
| Placard | none | missing | — | — | #13 |

## Indicators and feedback

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Progress indicator | `ProgressIndicator` | functional | four | — | #8 |
| Capacity, rating, relevance | none | missing | — | — | #45 |
| Notice, status line | `Notice` | provisional | four | — | unfiled |

## Text and input

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Static text | styled text in composition | missing | — | — | unfiled |
| Text input field | `TextField`, `TextFieldEvent` | functional | four | — | #58 |
| Token field | none | missing | — | — | #43 |
| Search field | `TextField` search variant | functional | four | — | #77 |
| Scrolling list | `controls::table::virtual_table` | provisional | four | — | #32 |
| Scroller | `Scrollbar` | provisional | four | — | #40 |

## View controls

| Component | Silvestris | Status | Appearances | Golden | Issue |
| --- | --- | --- | --- | --- | --- |
| Disclosure triangle | `Disclosure` | functional | four | — | #5 |
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
| Window frame | `WindowFrame`, `FrameCapabilities` | functional | four | — | #20 |
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
| Focus and responder routing | `CommandScope`, GPUI focus handles | provisional | four | — | #57 |
| Keyboard shortcuts | `CommandRegistry::bindings`, `bindings_where` | functional | four | — | #57, #66 |
| Modal sessions | `ConfirmationFocus` only | provisional | four | — | #21 |
| Drag and drop | none | missing | — | — | #36 |
| Undo | none | missing | — | — | #39 |
| System images and icons | `sprites` | provisional | four | — | #48 |
| Cursors | GPUI cursor styles | provisional | four | — | #48 |

## Keeping the matrix current

`crates/silvestris/tests/coverage.rs` fails when a public type in
`crates/silvestris/src/controls/` has no row here. Add the row with the same
commit that adds the control. A row is `functional` only when the gallery
shows the control and a test covers its behaviour. A control that exists but
is absent from the gallery stays `provisional`, which is a reminder rather than
a claim of completeness.
