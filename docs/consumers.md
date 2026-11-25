# Consuming Silvestris

Silvestris is the GUI layer for GPUI applications that want Mac OS X 10.6
Aqua (plus a designed Dark Aqua and Geist Light/Dark). It ships the controls,
windows, command routing, focus, modality, transient surfaces, text editing
and preference widgets. An application supplies its domain model, policy and
command callbacks, and composes the rest from Silvestris.

This page is the entry point. It covers how to depend on Silvestris, how to
start an application, what each module provides and where to read more. The
second half is the integration contract: pins, the gap register and how to
move a pin.

## Contents

- [Ownership rules](#ownership-rules)
- [Getting started](#getting-started)
- [What Silvestris provides](#what-silvestris-provides)
- [Testing a consumer](#testing-a-consumer)
- [Maturity and evidence](#maturity-and-evidence)
- [Integration contract](#integration-contract)

## Ownership rules

| Layer | Owns |
|---|---|
| Silvestris | Generic controls and their states, windows and chrome, themes, command routing, focus and key routing, modality, transient surfaces, the text-editing core, preference widgets and storage mechanics |
| Your application | Domain models, documents and sessions, file formats, persistence locations, command callbacks, validation, app-specific focus or input policy |
| GPUI | Platform paint and input. Silvestris builds on its focus handles, actions and input handlers and does not replace them |

Two rules follow from this:

- Do not paint a generic control yourself. If Silvestris lacks one, file a
  gap (see [Reporting a gap](#reporting-a-gap)) and record it in the
  [gap register](#gap-register).
- Do not patch or vendor Silvestris to fix a bug. Fix it upstream and move
  your pin.

Domain views (an emulator screen, a texture preview, a code editor) are
yours. Build them from Silvestris tables, splits, scrollers and paint
helpers so they still pick up the theme.

## Getting started

### Dependency

```toml
[dependencies]
silvestris = { git = "https://github.com/estyseesghosts/silvestris", rev = "<full commit>" }
gpui = { git = "https://github.com/zed-industries/zed.git", rev = "6b720973027999bae402df23d469026f4cced988" }

[dev-dependencies]
silvestris = { git = "https://github.com/estyseesghosts/silvestris", rev = "<same commit>", features = ["test-support"] }
```

Use the same GPUI revision as Silvestris (`config/pins.toml`) and the same
Rust toolchain (`rust-toolchain.toml`, currently 1.98.1). Two GPUI revisions
in one build produce incompatible types. Commit `Cargo.lock`. The forms
allowed for this dependency are under
[Supported dependency forms](#supported-dependency-forms).

`silvestris` re-exports `silvestris_core` as `silvestris::core`. Core holds
the toolkit-free parts: themes and tokens, the command model, layout
planning, the source-list tree and preference storage. You can unit-test
against core without a window.

### Start the application

Call `silvestris::init` once, before the first window opens. It registers the
bundled fonts, installs the theme, binds the text-editing keys and installs
modal-session handling.

```rust
use silvestris::{ThemeId, ThemePreference};

fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        // Load the saved preference yourself; Silvestris does not choose
        // where it lives.
        let preference = ThemePreference::explicit(ThemeId::AQUA_LIGHT);
        silvestris::init(preference, cx).expect("register fonts and theme");

        let registry = cx.new(|_| silvestris::command::CommandRegistry::default());
        // Register commands here, before the first window renders.

        let bounds = gpui::Bounds::centered(None, gpui::size(gpui::px(800.), gpui::px(600.)), cx);
        let options = silvestris::chrome::window_options(
            "Untitled",
            bounds,
            gpui::size(gpui::px(400.), gpui::px(300.)),
            cx,
        );
        silvestris::chrome::open_window(options, cx, |_, cx| {
            cx.new(|cx| MyRoot::new(registry, cx))
        })
        .expect("open window");
    });
}
```

The [composition guide](guide.md) walks through the full startup: window,
shell, commands, controllers and preferences. The
`crates/silvestris-demo/src/bin/explorer.rs` sample uses only the public
API. The gallery (`cargo run -p silvestris-demo --bin gallery --locked`)
shows every control in every state; 1–4 switch appearance.

### Read theme values

Every view can read the active theme through the `ActiveTheme` trait:

```rust
use silvestris::ActiveTheme;
let theme = cx.theme();
let fill = silvestris::convert::hsla(theme.colors.window);
let row = theme.metrics.table_row_height;
```

Use tokens rather than literal colours or sizes so your domain views follow
appearance changes. Tokens are defined in `silvestris_core::tokens` and set
per theme in `aqua.rs` and `geist.rs`. Aqua Light values cite
`reference/measurements.json`.

## What Silvestris provides

Each area names the module, the main types, what you get and the document
with the details. [coverage.md](coverage.md) has the per-component status,
appearances, goldens and open issues.

### Appearance and fonts

| | |
|---|---|
| Module | `silvestris::theme`, `silvestris::core::theme`, `silvestris::core::tokens` |
| Types | `ThemePreference`, `ThemeId` (`AQUA_LIGHT`, `AQUA_DARK`, `GEIST_LIGHT`, `GEIST_DARK`), `ThemeFamily`, `Appearance`, `Theme`, `FontOverrides` |
| Functions | `set_theme_preference`, `system_appearance_changed`, `set_font_overrides`, `set_global_font_overrides`, `clear_font_overrides`, `font_overrides` |

A preference is either explicit or follows the system's light or dark
appearance within a family. Changing it repaints every window live. Aqua
ships Lato and Source Code Pro in place of Lucida Grande and Monaco; Geist
ships Inter and Geist Mono. All fonts are bundled
(`crates/silvestris-fonts/fonts/manifest.json`), and nothing is fetched at
runtime. Font overrides let an application supply its own house face per
role without forking a theme.

### Windows and chrome

| | |
|---|---|
| Modules | `silvestris::chrome`, `silvestris::window_role`, `silvestris::native` |
| Types | `WindowFrame`, `WindowRole`, `RoleSpec`, `OwnedWindows`, `OwnedWindow`, `FrameStore`, `SavedFrame`, `PreferenceFrames` |
| Functions | `window_options`, `fixed_window_options`, `open_window`, `open_role`, `open_owned_role`, `role_options`, `restore_bounds`, `save_frame`, `cascade_from`, `centered_on` |
| Docs | [windows.md](windows.md), [snapping.md](snapping.md) |

`WindowFrame` draws the Aqua title bar, traffic lights, toolbar, bottom bar,
edited dot and resize grip. A `WindowRole` (document, utility panel,
preferences, dialog, about, and others) decides which of these appear,
whether the window zooms or resizes, and how it cascades. `FrameStore`
persists and restores window frames through your preference store.
`native` exposes the platform pieces: frame capabilities, zoom, the resize
grip hit test and the system caret blink rate.

### Shell layout

| | |
|---|---|
| Modules | `silvestris::shell`, `silvestris::sidebar`, `silvestris::core::layout` |
| Types | `Shell`, `ShellContent`, `Sidebar`, `ShellPlan`, `ShellGeometry`, `SidebarPreference`, `InspectorPreference`, `Presentation`, `Divider` |
| Docs | [shell.md](shell.md), [sidebar.md](sidebar.md), [guide.md](guide.md#shell) |

`Shell` lays out a sidebar, the workspace and an inspector that docks or
overlays depending on width. Layout is planned by the pure
`core::layout::plan`, so it can be unit-tested. Keep the preference structs
in your state and update them from the shell's change callbacks.
`Shell::measured` hands you the plan when a child needs the exact geometry.

### Controls

All controls are in `silvestris::controls`. Most are `RenderOnce` builders:
you hold the value and pass it in each render, and callbacks report user
changes. Setting a value from code never fires a callback. Controls follow
the window's active or inactive state, keyboard focus, disabled state and
all four appearances.

| Control | Type | Notes |
|---|---|---|
| Push, textured, gradient, bevel, round, recessed and help buttons | `Button`, `ButtonStyle`, `ControlSize` | Regular, small and mini. A default button pulses. A cancel button answers Escape |
| Checkbox | `Checkbox`, `CheckState` | On, off and mixed. Clicking the label toggles. [checkbox.md](checkbox.md) |
| Radio group | `RadioGroup`, `RadioOption` | Arrow keys move the selection |
| Pop-up button | `PopUpButton`, `PopupStyle` | The menu opens with the current item over the button. Textured style for toolbars |
| Segmented control | `SegmentedControl`, `Segment`, `SegmentMode`, `SegmentStyle` | Select one, select any, momentary. Text or icon segments. [segmented-control.md](segmented-control.md) |
| Slider | `Slider`, `Orientation`, `ThumbStyle` | Ticks, round or pointed thumb |
| Stepper | `Stepper` | Reports a direction; you own the value and bounds |
| Progress | `ProgressIndicator`, `ProgressStyle` | Bar (determinate, or the barber pole without a fraction), spinner and ring. Stops animating when the window is inactive or reduced motion is set |
| Disclosure | `Disclosure`, `DisclosureStyle` | Triangle and button styles. Keyboard: Space, Enter, Left, Right |
| Text field | `TextField`, `TextFieldEvent` | An entity. IME, clipboard, undo, masked (password) mode with `set_masked`. [text.md](text.md) |
| Combo box | `ComboBox` | Text entry with a suggestion list |
| Search field | `TextField::set_search` | Capsule bezel and clear button |
| Shortcut recorder | `ShortcutRecorder` | Records a chord in the spelling `CommandRegistry::rebind` accepts |
| Group box, separator, well | `GroupBox`, `Separator`, `SeparatorAxis`, `Well` | A group box title can be text, a checkbox or a pop-up |
| Tab view | `TabView` | Regular, small and mini tabs; scrolls overflowing content |
| Table view | `TableView`, `TableColumn` | Sortable, resizable, reorderable columns with autosave; multiple selection by id, type-select, in-place editing, virtualised rows. [table.md](table.md) |
| Scroll view | `ScrollView`, `Scrollbar`, `ScrollSource` | Content plus both scroll bars and the corner over a `ScrollHandle`, `UniformListScrollHandle` or `ListState`; regular, small and mini; Appearance preferences. [scrollbar.md](scrollbar.md) |
| Source list | `SourceList`, `SourceListEvent`, `SourceListStyle`, `RowAccessory`, `RowStatus` | Sidebar tree with sections, badges, eject and status accessories. [source-list.md](source-list.md) |
| Column browser | `ColumnBrowser`, `BrowserNode`, `BrowserEvent` | Hierarchical columns over provider-supplied children; late answers, Left/Right, resize grip, preview. [column-browser.md](column-browser.md) |
| Path controls | `PathPopUp`, `PathControl`, `PathEntry` | Ancestors plus Recent Places, and the Finder breadcrumb. Emit a location id; the app navigates |
| Open and Save panels | `SavePanel`, `OpenPanel`, `FilePanel`, `FileProvider`, `MemoryProvider`, `StdFsProvider`, `FileBrowserModel`, `PanelStateStore`, `ExtensionPolicy`, `SaveDisposition` | `SavePanel::begin_sheet` for Save/Save As, `OpenPanel::run_modal` for File > Open, `OpenPanel::begin_sheet` for Choose. Right-click menus expose only provider capabilities; Move to Trash is provider-owned, permanent delete is a confirmed opt-in. `extension_policy` is per panel. Returns paths/locations plus a Save disposition; never writes files. [file-panels.md](file-panels.md) |
| Sheet | `sheet::Sheet`, `sheet::card_shell` | Document-owned surface flush with the band; one per window; modal policy stays in `modal` |
| Alert | `Alert`, `AlertButton`, `AlertResult` | Icon, message, up to three buttons, suppression; replace-existing preset. `run_modal` for an application-modal alert, `begin_sheet` for a document confirmation |
| Table rows | `table::virtual_table`, `table::code_row` | Virtualized alternating rows; you own the cell contents |
| Help tags | `tooltip::attach`, `tooltip::attach_in` | Themed tooltips on the transient layer |
| Notices | `Notice`, `notice::error_banner` | Empty and error states for a workspace |
| Status values | `status::value` | Labelled values for status bars |
| Confirmation sheet | `ConfirmationSheet`, `ConfirmationFocus` | Thin wrapper over `Alert`; new code should compose `Alert` and a `sheet::Sheet` |
| Multiline text | `TextArea` | Prototype. Do not adopt until #58 lands the multiline API |

### Data binding

| | |
|---|---|
| Module | `silvestris::binding` |
| Types | `Binding`, `TextBinding`, `BoundCheckbox`, `BoundRadio`, `BoundPopUp`, `BoundSlider`, `ListController`, `ItemId` |
| Docs | [bindings.md](bindings.md) |

Bindings connect a control to a field of your model without hand-written
callbacks, and validate text input. `ListController` keeps selection and
scroll by item id across sorting, reloads and live updates.

### Commands, menus and the responder chain

| | |
|---|---|
| Modules | `silvestris::command`, `silvestris::responder`, `silvestris::menu`, `silvestris::core::command` |
| Types | `CommandRegistry`, `CommandDescriptor`, `CommandSet`, `CommandId`, `MenuModel`, `MenuEntry`, `CommandButton`, `CommandScope`, `InvokeCommand`, `ResponderChain`, `Responder`, `PopupMenu` |
| Docs | [commands.md](commands.md), [menus.md](menus.md) |

There is one command system. Register each command once with a stable id, a
label, an optional shortcut (`secondary-` means Command on macOS and Ctrl
elsewhere) and a callback. Its descriptor carries enabled, checked and mixed
state, and whether the command needs more input ("…"). Toolbar buttons,
menus and keyboard shortcuts all invoke through the registry, so their
labels and states stay in sync. Shortcuts can be rebound at runtime.
Command shortcuts never fire while a Silvestris text control has focus.

The responder chain routes a command to the focused view first and walks
out to the window and application. It also validates enabled state, so the
Edit menu follows the focused field.

`PopupMenu` draws contextual menus (`open`) and menus hanging from a control
(`open_pop_down`) from a `MenuModel`, with submenus, separators, checkmarks
and dashes, key equivalents and keyboard navigation. `menu::native_menus`
projects the same models onto the macOS menu bar.

### Transient surfaces and modality

| | |
|---|---|
| Modules | `silvestris::transient`, `silvestris::modal` |
| Types | `TransientLayer`, `SurfaceSpec`, `TransientKind`, `DismissPolicy`, `TransientHandle`, `CancelToken`, `ModalSession`, `ModalScope`, `ModalResult` |
| Docs | [transients.md](transients.md), [modal.md](modal.md) |

Menus, pop-ups, help tags and popovers share one transient layer per window.
It owns outside clicks, Escape, z-order and returning focus, so your
controls do not. Modal sessions block commands and input for a window or the
whole application while a sheet or dialog is up.

### Documents and the application lifecycle

| | |
|---|---|
| Modules | `silvestris::document`, `silvestris::lifecycle` |
| Types | `Document`, `DocumentController`, `Documents`, `DocumentTitle`, `SaveResult`, `AppDelegate`, `QuitCoordinator`, `Terminate` |
| Docs | [documents.md](documents.md), [lifecycle.md](lifecycle.md) |

Implement `Document` for your model. Silvestris then handles the edited
dot, untitled numbering, the save-before-close decision and coordinated
quit with a review of unsaved documents. You own the file format and IO.
`AppDelegate` receives launch, activation and reopen events.

### Preferences

| | |
|---|---|
| Modules | `silvestris::prefs`, `silvestris::core::prefs`, `silvestris::preferences` |
| Types | `Preferences`, `Store`, `Storage`, `FileStorage`, `Versioned`, `Migration`, `PreferencesNavigator`, `PreferencePane`, `PreferenceCategory`, `PreferencesToolbar`, `PreferencesOverview`, `Form`, `FormRow` |
| Docs | [preferences.md](preferences.md) |

`Preferences` is an observable, versioned preference document with
migrations and keyed change callbacks. You choose where it is stored
(`FileStorage` or your own `Storage`). The `preferences` module builds the
window: a toolbar or System Preferences–style overview, panes that resize
the window to fit, label/control form rows and `pane_tab_view`.

### Text editing and undo

| | |
|---|---|
| Modules | `silvestris::text`, `silvestris::text_client`, `silvestris::undo` |
| Types | `Buffer`, `StringBuffer`, `Selection`, `EditSession`, `CaretBlink`, `ClipboardPolicy`, `TextClient`, `UndoManager`, `TextHistory` |
| Docs | [text.md](text.md), [undo.md](undo.md) |

The text core handles grapheme-safe editing, IME composition, the platform
input-handler adapter and caret blinking. A custom editor (chaus's Ropey
buffer, for example) can implement `Buffer` and reuse the rest.
`UndoManager` groups edits and drives the Undo and Redo commands per window
or document.

### Painting your own domain views

`silvestris::paint` and `silvestris::convert` expose the painters controls
use: bevels, fields, wells, group boxes, selection gradients, focus rings,
hairlines and snapped bounds. `convert::hsla` turns a token into a GPUI
colour; `convert::hairline` gives one device row at any scale. Use them in
domain views so edges stay crisp at 125–200% and colours follow the theme.
See [snapping.md](snapping.md).

## Testing a consumer

Enable the `test-support` feature in dev-dependencies to get
`silvestris::test_support`:

- `Scenario` opens windows under GPUI's test platform with Silvestris
  installed.
- Fixtures (`TextFieldFixture`, `SourceListFixture`, `DocumentWindowFixture`,
  `PopupMenuFixture`, `KeyIsolationFixture` and others) give ready-made
  windows for common checks.
- `ScenarioScript` and `Recorder` replay and record input scripts.

Controls carry stable debug selectors (`menu-item-0-3`, `tab-view-box`,
`segment-<id>`) so tests can find bounds with `cx.debug_bounds`. Headless
tests prove GPUI dispatch and layout; they do not prove native input, OS
window behaviour or font rasterisation. [testing.md](testing.md) has the
details. [goldens.md](goldens.md) describes the render gate Silvestris uses for
its own pixels.

## Maturity and evidence

- Silvestris has no released version. Consumers pin commits;
  [CHANGELOG.md](../CHANGELOG.md) lists every API and behaviour change by
  commit range. Read it before moving a pin.
- [coverage.md](coverage.md) is the component status table. "Measured"
  means Aqua Light was compared with a 10.6 capture and has goldens;
  "functional" means the behaviour works but the visuals are not yet
  verified.
- [exceptions.md](exceptions.md) lists every place Silvestris deviates from
  the 2009 HIG or the captures, every estimate and every platform gap.
- Windows is the development platform. macOS and Linux validation is open
  (#50).
- The roadmap and milestones are in [ROADMAP.md](../ROADMAP.md) (epic #72).

## Integration contract

Silvestris is consumed as source. Consumers pin an exact revision, share its
GPUI revision and toolchain, and report gaps upstream instead of patching a
private copy.

### Supported dependency forms

Use a git revision with a committed lockfile:

```toml
silvestris = { git = "https://github.com/estyseesghosts/silvestris", rev = "<full commit>" }
```

Commit `Cargo.lock` and `rust-toolchain.toml` with the change. A consumer
build must resolve one GPUI: the revision in `config/pins.toml`, currently
`6b720973027999bae402df23d469026f4cced988`, crate version 0.2.2. Mixing two
GPUI revisions produces incompatible types even when both compile.

A path dependency is allowed only during local development against a clean,
committed Silvestris tree. Do not commit a path to a dirty checkout, a vendor
copy or a fork. Sandcat is the recorded exception, below.

Vendoring a private copy is not a supported form. Every local patch becomes
invisible to the other consumers and is lost on the next refresh. Upstream the
generic mechanism, then move the pin.

### Current consumer pins

Pins verified on 2026-11-23. Full commit ids, not abbreviations, belong in
`Cargo.toml` and `config/pins.toml`.

| Consumer | Pin | Form | Local patches |
| --- | --- | --- | --- |
| lotus | `d1db8f0c6cbe80e80fb1dfebbb5c78990d2b9265` | vendored `vendor/silvestris` (tree `a1b78ec`) | Inspector scroll, inspector focus deferral, printable keys, Settings bounds |
| chaus | `30173a5860ad263feacb3d417a73c3a354a21e78` | git revision | none |
| johjoh | `d1db8f0c6cbe80e80fb1dfebbb5c78990d2b9265` | git revision | none |
| sandcat | uncommitted checkout | path dependency | deferred |

Lotus's three generic patches are now upstream in `4df116b` (PR #81). Re-pin
lotus to that revision or later, regenerate `vendor/silvestris` from it and
delete the local patch list. The Settings bounds restore stays in lotus: it
decides which bounds to persist for which window role.

Sandcat builds against a path dependency into an uncommitted checkout. Its
adoption is deferred until Silvestris's own foundations land. It is not a gating
consumer, no provisional API exists for it, and this exception is recorded so
the pin table does not imply support.

### Gap register

Every consumer-side bespoke UI element found in the 2026-11-23 survey, with the
issue that replaces it. Adoption happens in the consumer repository after the
Silvestris issue lands. Sandcat rows are inventory only while its adoption is
deferred.

| Consumer | Bespoke today | Replaced by |
|---|---|---|
| lotus | `settings_window/shared.rs` `fit_size`/`fit_pending` pane resizing | [#14](https://github.com/estyseesghosts/silvestris/issues/14) |
| lotus | `settings_window/controllers.rs`, `field_text`/`parse_field` | [#61](https://github.com/estyseesghosts/silvestris/issues/61), [#54](https://github.com/estyseesghosts/silvestris/issues/54) |
| lotus | `settings_window/bindings.rs` shortcut editor | [#66](https://github.com/estyseesghosts/silvestris/issues/66) |
| lotus | `gauges.rs` stat cards, pills, sparklines | [#45](https://github.com/estyseesghosts/silvestris/issues/45) |
| lotus | `icons.rs` 16 px stroke icon set, `group_icon` | [#48](https://github.com/estyseesghosts/silvestris/issues/48) |
| lotus | `lotus_window/command_palette.rs` | [#76](https://github.com/estyseesghosts/silvestris/issues/76) |
| lotus | `tiling.rs` edge drop zones, drag previews | [#36](https://github.com/estyseesghosts/silvestris/issues/36) (primitives); tiling policy stays app-owned |
| lotus | Split stage dividers | [#53](https://github.com/estyseesghosts/silvestris/issues/53) |
| lotus | Memory, register, breakpoint, kernel, snapshot tables; event timeline | [#32](https://github.com/estyseesghosts/silvestris/issues/32), [#33](https://github.com/estyseesghosts/silvestris/issues/33), [#40](https://github.com/estyseesghosts/silvestris/issues/40) |
| lotus | `state_dot`, row accessories in the session list | [#4](https://github.com/estyseesghosts/silvestris/issues/4) |
| lotus | Status bar values (`status_value`), toolbar ([lotus#133](https://github.com/estyseesghosts/lotus/issues/133)) | [#75](https://github.com/estyseesghosts/silvestris/issues/75), [#26](https://github.com/estyseesghosts/silvestris/issues/26) |
| lotus | Secondary windows (investigation, regression, record/test) ([lotus#134](https://github.com/estyseesghosts/lotus/issues/134)) | [#20](https://github.com/estyseesghosts/silvestris/issues/20), [#27](https://github.com/estyseesghosts/silvestris/issues/27) |
| lotus | Flat legacy primary action buttons | [#10](https://github.com/estyseesghosts/silvestris/issues/10) |
| lotus | Vendored Silvestris patches ([lotus#141](https://github.com/estyseesghosts/lotus/issues/141) inspector, [lotus#74](https://github.com/estyseesghosts/lotus/issues/74) focus deferral, [lotus#58](https://github.com/estyseesghosts/lotus/issues/58) Settings bounds) | [#74](https://github.com/estyseesghosts/silvestris/issues/74) (upstreamed; lotus re-pins) |
| lotus | PICA texture/preview rendering, debugger domain panels | app-owned (domain views composed from Silvestris tables/splits) |
| lotus | Settings window bounds choice per role | app-owned (which bounds persist is application policy) |
| sandcat | `apps/desktop/src/scrollbar.rs` Aqua scroller over `ListState` | [#40](https://github.com/estyseesghosts/silvestris/issues/40) |
| sandcat | Composer on the discarded `TextArea` prototype | [#58](https://github.com/estyseesghosts/silvestris/issues/58) (new multiline API); sandcat re-adopts after the foundations land |
| sandcat | `body_segments` / code-block rendering in transcript | [#38](https://github.com/estyseesghosts/silvestris/issues/38) |
| sandcat | `status_label`/`status_color` session badges, `project_tone` | [#4](https://github.com/estyseesghosts/silvestris/issues/4), [#48](https://github.com/estyseesghosts/silvestris/issues/48) |
| sandcat | First-run / server pairing setup state | [#30](https://github.com/estyseesghosts/silvestris/issues/30) |
| sandcat | Permission prompts and forms | [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#23](https://github.com/estyseesghosts/silvestris/issues/23), [#54](https://github.com/estyseesghosts/silvestris/issues/54) |
| sandcat | Theme/model/agent pickers in toolbar | [#2](https://github.com/estyseesghosts/silvestris/issues/2), [#76](https://github.com/estyseesghosts/silvestris/issues/76) |
| sandcat | `!SilvestrisTextField` key-context exclusions | [#57](https://github.com/estyseesghosts/silvestris/issues/57) |
| sandcat | Shutdown draft flush | [#59](https://github.com/estyseesghosts/silvestris/issues/59) |
| sandcat | Composer submission, Ctrl+1–4 policy, transcript following | app-owned (session behaviour on top of [#38](https://github.com/estyseesghosts/silvestris/issues/38), [#57](https://github.com/estyseesghosts/silvestris/issues/57)) |
| chaus | Find bar (`editor_find.rs`) | [#77](https://github.com/estyseesghosts/silvestris/issues/77) |
| chaus | Close / conflict / recovery prompts (`editor_close.rs`, `editor_conflict.rs`, `editor_recovery.rs`) | [#37](https://github.com/estyseesghosts/silvestris/issues/37), [#22](https://github.com/estyseesghosts/silvestris/issues/22), [#23](https://github.com/estyseesghosts/silvestris/issues/23) (policies stay app-owned) |
| chaus | Menus assembled by hand (`tests/menus.rs`) | [#24](https://github.com/estyseesghosts/silvestris/issues/24) |
| chaus | Startup-only shortcut bindings | [#66](https://github.com/estyseesghosts/silvestris/issues/66) |
| chaus | Locally registered Geist Sans house font (`chaus-app/assets/fonts`) | [#71](https://github.com/estyseesghosts/silvestris/issues/71) (font overrides), then [#70](https://github.com/estyseesghosts/silvestris/issues/70) |
| chaus | Ropey editor, Vim engine, highlighting | app-owned by design (may reuse [#58](https://github.com/estyseesghosts/silvestris/issues/58) IME/selection traits) |
| johjoh | `heading`/`caption`/`group` helpers | `group` → `controls::GroupBox` (#13, adoption: estyseesghosts/johjoh#4); `heading`/`caption` need typography roles (unfiled) |
| johjoh | Hand-built ROM list rows | [#32](https://github.com/estyseesghosts/silvestris/issues/32) |
| johjoh | `prompt_for_paths` native dialogs | [#29](https://github.com/estyseesghosts/silvestris/issues/29), [#63](https://github.com/estyseesghosts/silvestris/issues/63) |
| johjoh | `render_bottom` progress/status strip | [#75](https://github.com/estyseesghosts/silvestris/issues/75), [#8](https://github.com/estyseesghosts/silvestris/issues/8) |
| johjoh | Error advice text | [#23](https://github.com/estyseesghosts/silvestris/issues/23) |
| johjoh | OS file drops (`ExternalPaths`) | [#36](https://github.com/estyseesghosts/silvestris/issues/36) |

Chaus adopted #29 at `30173a58`: File → Open is
`OpenPanel::run_modal` in a `WindowRole::OpenPanel` window, and Save As is
`SavePanel::begin_sheet` attached to the document. Chaus keeps one
`StdFsProvider` in `chaus-editor::FileDialogs` for the application session,
persists the separate Open and Save panel state plus the Open frame in a
UI-preferences document, and maps `SaveDisposition` onto its existing
create-new/overwrite save engine. The consumer tests open a real editor
window against a temporary directory and drive the real panel, so further
Open/Save changes should be validated through Chaus before they ship.

A new bespoke element goes in this table with an issue number or an app-owned
reason. "App-owned" means the element encodes policy the framework cannot
generalise; it does not excuse drawing a generic control by hand. The table is
the single copy: `ROADMAP.md` links here.

### Upstreamed lotus deltas

Each entry names the generic mechanism, its regression test and what stays in
lotus.

`TabView::scroll_handle` (`crates/silvestris/src/controls/tab_view.rs`) accepts a
caller-owned scroll position so an inspector pane can reveal content after a
layout change instead of depending on a keyed internal handle. Lotus keeps the
policy of which pane to reveal and when. Test: `tests/tab_view.rs`,
`tall_content_reports_overflow_and_scrolls`.

Shell inspector focus now transfers in `window.defer`, after the frame is
built, on open and on close. A focus change made by another view in the same
frame (for example a toolbar reporting to AccessKit) wins, because the deferred
closure checks the focused handle before moving it. Tests:
`shell::tests::inspector_focus_moves_after_the_frame_and_returns_on_close` and
`shell::tests::deferred_inspector_focus_does_not_override_a_toolbar`.

Shell's inspector key handler ignores printable keys while a
`SilvestrisTextField` is in the focus context, so GPUI's input handler receives
the character. Navigation, Escape and consumer shortcuts still run. Test:
`shell::tests::printable_keys_reach_a_focused_text_field_through_the_shell`.
Lotus's guest-input isolation rules and emulator focus policy remain in lotus.

### Reporting a gap

File an issue in `estyseesghosts/silvestris` with the standard anatomy: Why,
References, Deliverables, Avoid, Regression traps, Done when. Add the milestone,
layer label, priority and consumer label. Include a failing scenario using the
interaction harness where the behaviour is testable.

Do not add a consumer-specific type, policy or name to a Silvestris crate. Do
not keep a second implementation of a control that Silvestris already ships.
If the gap is app-owned domain behaviour, write it down as such in the gap
register.

### Moving a pin

1. Choose the Silvestris commit and read the matching `CHANGELOG.md` range.
2. Update `Cargo.toml` and `Cargo.lock` together; keep `config/pins.toml`
   aligned.
3. Run `tools/consumers/check.ps1` with the candidate checkout present.
4. Record any behavioural change to focus, keys, windows or persistence in the
   consumer's own release notes.

Changing the GPUI revision or the Rust toolchain is a breaking event. It needs
a note here, a passing consumer check and the same revision in every consumer.

### Build check

`tools/consumers/check.ps1` is opt-in and CI-able:

```powershell
pwsh tools/consumers/check.ps1 -Candidate . -Consumer lotus,chaus,johjoh
```

It skips a consumer whose checkout is absent, always skips sandcat, and fails
on a build error. Lotus is checked from a temporary worktree whose
`vendor/silvestris` is replaced by the candidate tree, so the build proves
lotus compiles against the candidate; it does not prove the old vendor copy was
byte-identical to a commit (compare trees for that). Git-pinned consumers are
checked from a temporary worktree with a Cargo `[patch]` entry pointing at the
candidate, so the consumer's own working tree and lockfile are not modified.

Last run, 2026-11-23, against `10747da` and later: lotus, chaus and johjoh all
reported `ok` and each compiled `silvestris` from the candidate path. Lotus ran
with `-SkipSubmodules` against a checkout whose `vendor/azahar` is empty, which
is enough for `cargo check -p lotus-desktop`; building the emulator binaries
needs that submodule. Lotus has not yet re-pinned to the upstreamed revision.
