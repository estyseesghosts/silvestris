# Changelog

Silvestris has no released crate version. Consumers pin commits, so entries are
keyed by commit range. A range lists public API changes a consumer must handle
when moving its pin. Behaviour-only changes are listed because consumers can
depend on focus, key routing and layout.

## After 6d3ceb1

M3 column browser, path controls and M4 panels (#34, #41, #22, #23, #29):

- Added `controls::ColumnBrowser`, `BrowserNode`, `BrowserEvent` and the
  `browser.*` measurements. `ColumnBrowser` gained `set_rename`,
  `begin_editing` and `is_editing` for New Folder in column view.
  `TableView` gained `set_header_visible`, `set_alternating`, `set_fill_width`,
  `repaint` and the matching getters.
- Added `controls::PathPopUp`, `PathControl`, `PathEntry`, `PathEvent` and
  `LocationId`. `PopUpButton` gained `leading_icon` and `unselectable`.
- Added `icons::Icon` (folder, home, desktop, documents, volume, document,
  application, back, forward, list, columns, eject, search) and
  `icons::{role_renderer, source_renderer}`.
- Added `chrome::WindowFrame` sheet attachment through `sheet::overlay`; a
  custom window root must call `sheet::overlay` to show sheets.
- Added `modal::ModalSession::set_focus_stops` and `set_scope`.
- Added `controls::Alert`, `AlertButton`, `AlertResult`, `AlertRole` and the
  `Alert::replace_existing` preset. `ConfirmationSheet` keeps its API and now
  renders an `Alert` body.
- Added `sheet::Sheet`, `sheet::present` and `sheet::active_sessions`.
- Added `file_panel` with `FileProvider`, `FileEntry`, `FilePlace`,
  `FileAncestor`, `FileError`, `FileKind`, `MemoryProvider`, the feature-gated
  `StdFsProvider`, `FileBrowserModel`, `FileBrowserEvent`, `FileViewMode`,
  `SavePanel`, `OpenPanel`, `FilePanel` and `PanelOutcome`.
- `SavePanel`/`OpenPanel` present a document sheet with an owner window and an
  application-modal dialog without one.
- Added the `std-fs` feature (off by default) for the local-disk provider.

## After 6bd0467

M3 table view (#32):

- Added `controls::TableView`, `TableColumn`, `TableEvent`, `CellContext`,
  `ColumnAlign`, `GridLines`, `SortDirection` and `icon_text`.
- Added `ColumnStore`, `MemoryColumns`, `PreferenceColumns`, `SavedColumns`
  and `SavedTables` for column autosave.
- `ListController` now holds a multiple selection: added `selected_ids`,
  `is_selected`, `anchor`, `set_selection`, `toggle`, `extend_to`,
  `select_all`, `selected_items`, `visible_item`, `visible_index_of` and
  `id_of`. `selection()` returns the lead id; `select` still replaces the
  selection with one id.
- Tokens: added `Controls::table_header_sorted`, `sort_arrow` and
  `table_grid`.

## After 0d5cc60

M3 scrollers (#40):

- Added `controls::ScrollSource` with adapters for `ScrollHandle`,
  `UniformListScrollHandle` and `ListState`, plus the
  `scroll_source::{scroll_by, scroll_to, page, reveal}` helpers.
- Added `controls::ScrollView` and `ScrollAxes`: content, both scroll bars and
  the corner in one element.
- `Scrollbar::new` now takes any `ScrollSource + Clone` (existing
  `&ScrollHandle` callers compile unchanged). Added `Scrollbar::horizontal`,
  `size`, `line` and `autohide`.
- Added `ScrollerPreferences`, `ScrollArrows` and `TrackClick`: an app-wide
  GPUI global for the Appearance scroll-arrow and track-click preferences.
- Token changes: `Metrics::scroller_width` is now `[f32; 3]` (regular, small,
  mini). `Scroller` gained `track_rows`, `arrow_rows` and `cap_shadow`.
  Themes built outside Silvestris must set them.
- Visual: the Aqua thumb now follows its measurement (4-row upper band, grey
  `#5e5e5e` trailing edge; was 5 rows and `#3e6d97`). Track and arrow boxes
  are painted from measured cross-sections, with cap and end shading and a
  full-width divider. Every scroller changes, including the tab view's.

## After e22e7bf

M2 menus (#16):

- Added `PopupMenu::open_pop_down` (square top corners, for menus hanging
  from a control) and `PopupMenu::highlight_path` (preselect rows and open
  submenus without invoking). `PopupMenu::open` is the contextual menu with
  four rounded corners.
- Added `CommandDescriptor::mixed`, the `mixed` builder and
  `CommandSet::set_mixed` for the HIG dash state (pp. 170-171). It is
  exclusive with `checked`. Struct literals of `CommandDescriptor` must set
  the new field. GPUI's native menu items have no mixed state, so
  `native_menus` shows a mixed item unchecked.
- Added `menu::split_shortcut`, which splits a displayed shortcut into
  modifiers and key for the key-equivalent column.
- Added tokens `Colors::menu_shadow`, `Metrics::menu_shadow_spread` and
  `Metrics::menu_label_dy`. Themes built outside Silvestris must set them.
  `Metrics::menu_shadow_blur` is now documented as GPUI's Gaussian sigma.
- Visual: Aqua menus use the measured contextual geometry: 18 px rows (was
  19, a menu-bar value), translucent white body, an edge hairline over the
  shadow, a shadow cast only outside the body, separators with 1 px insets,
  a drawn submenu arrow, a left-aligned key column and no 160 px minimum
  width. Submenus overlap the parent edge by 2 px and line their first row
  up with the parent row. The pop-up button menu shares the frame and body.

M2 tab views and group boxes (#13):

- Moved `TabView` to `controls::TabView`; `preferences::TabView` re-exports
  it. The control no longer adds the System Preferences top offset: its top
  edge is now the top of the tab control. Wrap it in
  `preferences::pane_tab_view(tab_view, cx)` to keep the old placement (box
  line 34 px below the toolbar separator).
- Added `TabView::size` (regular, small, mini tabs with 12 / 10 / 8 px label
  margins) and `GroupBox::title_control` for a checkbox or pop-up title.
- Behaviour: `GroupBox` titles use the small system font, no longer bold
  (HIG p. 344).
- Visual: group boxes and tab views paint the measured 10.6 edges (darker top
  over three shadow rows, lighter bottom, inner side highlight). Before, the
  top showed a doubled line and the bottom matched the sides. Segmented
  controls now show their measured emboss row, which the segment clip used to
  cut off.
- Added colour tokens `group_border_bottom`, `group_highlight`,
  `group_highlight_bottom` and `group_shadow`. Themes built outside Silvestris
  must set them.

## After 44ed694

M1 audit fixes. Behaviour changes a consumer must check:

- `CommandRegistry::bindings` and `bindings_where` now bind every command
  shortcut in `(<context>) && !SilvestrisText`. Drop hand-written
  `!SilvestrisTextField` clauses; they still parse, but they missed
  `TextArea`, where command shortcuts used to fire while typing (#57).
- Undo or redo during an IME composition now cancels the composition before
  applying the step. Before, the unrecorded marked text stayed in the buffer
  and shifted the recorded ranges, so the step edited the wrong bytes (#39).
- Added `text_client` with `TextClient`, `impl_text_input_handler!`,
  `restart_blink`, `stop_blink`, `observe_activation` and `select_range`.
  `TextField` and `TextArea` now share one `EntityInputHandler` (#58).
- Changed `text::CaretBlink`: `restart()` now returns the new epoch, `stop()`
  takes no argument, and `tick(epoch)` toggles. It was unused before.
- Behaviour: the caret no longer blinks in an inactive window; it is drawn
  steady and no timer runs until the window is active again (#58).
- Behaviour: Undo after an IME commit restores the text from before composing,
  not the raw composition (#58).
- Added `lifecycle::install`, `reopen`, `window_activation_changed`,
  `is_app_active`, `ACTIVATION_GRACE` and `SharedDelegate`; `Rc<RefCell<D>>`
  implements `AppDelegate`. Before, nothing called the delegate's launch,
  activation or reopen hooks (#59).
- Behaviour: `chrome::open_window` now observes each window's activation for
  the installed delegate. Without `install` it does nothing.
- Changed `window_role::FrameStore`: `load` takes `&App` and `save` takes
  `&mut App`; `restore_bounds` and `save_frame` take `cx`. Without the context a
  store could not update an entity, so frames could not go through #62's
  preference store as #20 asks. Added `PreferenceFrames`, `SavedFrame` and
  `SavedFrames`.

## After 216de9b

PR #59, application lifecycle. Additive:

- `lifecycle::AppDelegate` with `did_launch`, `should_open_untitled`, `reopen`,
  `did_become_active`, `did_resign_active` and `should_terminate`;
  `Terminate::{Now, Later(Task<()>), Cancel}`.
- `lifecycle::QuitCoordinator` with `request_quit`, `review_name`,
  `resolve_review`, `resolve_save`, `deferred_finished`, `take_deferred`,
  `timeout_elapsed`, `progress`, `needs_review_alert`; `QuitStep`,
  `ReviewChoice`, `QuitProgress`.
- `lifecycle::notify_activation`.

Behaviour: quit asks the delegate, waits for its deferred shutdown, then
reviews each dirty document; Cancel keeps the app and its dirty state; a needs-
panel save waits for #29; a failed save cancels; a timeout forces exit. No
wall-clock wait lives in the framework.

PR #37, documents. Additive:

- `document::Document`, `DocumentHandle`, `SaveResult`, `DocumentTitle`.
- `document::DocumentController` with `request_close`, `resolve_prompt`,
  `resolve_save`, `revert`, `title`, `is_saving`, `is_prompting`;
  `PromptChoice` and `CloseStep`.
- `document::Documents` for untitled numbering; `document::sync_title`.
- `WindowFrame::edited` and the close-button dirty dot.

Behaviour: a clean document closes; a dirty one prompts; Save may need a panel;
a failed save keeps the window open; a save in flight never prompts twice;
untitled numbers skip open windows. The prompt and Save panel are the app's
content in #21/#22/#29.

PR #39, undo. Additive:

- `undo::UndoManager` with `record`, `begin_group`/`end_group`, `undo`,
  `redo`, `clear`, `set_limit`, `undo_name`/`redo_name`, `can_undo`/`can_redo`;
  `UndoCallback`.
- `undo::TextHistory` and `undo::TextHost`, which record `text::Change` with
  typing coalescing; `undo::entry(manager, scope, name)`.
- `Responder for UndoManager`, claiming `edit.undo`/`edit.redo` with projected
  titles.
- `TextField::history`/`undo_entry` and `TextArea::history`/`undo_entry`.
- `text::Change` gained `removed` so undo can restore deleted text.

Behaviour: a focused control's Control-scope stack beats a Document-scope
stack; composing text is not recorded; contiguous and cumulative typing is one
entry; a change emits the client's Changed event when undone or redone.

PR #58, shared text core. Additive:

- `text::EditSession` with buffer, `Selection`, grapheme/word/line/paragraph
  boundaries, UTF-16 mapping, marked text, masked mode, `Change`/`EditKind`,
  `copy_text` and `floor_boundary`; `Buffer`/`StringBuffer`, `ClipboardPolicy`,
  `CaretBlink`.
- `controls::TextArea` and `TextAreaEvent`, built on the session with wrapped
  layout, visual-row movement, growth/scrolling and the composer contract.
- Shared `silvestris_text` actions and the `SilvestrisText` key context,
  including Up/Down/SelectUp/SelectDown/Newline; `cmd/ctrl-enter` submits.

Behaviour: `TextField` keeps `TextFieldEvent` and its silent setter but now
delegates editing to the session; the field's context stack also contains
`SilvestrisText` for shared bindings. `TextArea` submits on Return, inserts a
newline on Shift-Return, and never emits `Changed` from `set_text`.

PR #61, bindings. Additive:

- `binding::Binding<T>` with `new`, `field`, `value`, `set`; `checkbox`,
  `radio`, `popup` and `slider` adapters (`BoundCheckbox`, `BoundRadio`,
  `BoundPopUp`, `BoundSlider`).
- `binding::TextBinding<T>` with `sync`, `commit`, `field` and `subscribe`.
- `binding::ListController<T>` with `set_items`, `set_sort`, `set_filter`,
  `visible`, `visible_ids`, `select`, `selected`, `index_of`, `source_nodes`
  and `clear_selection`; `ItemId`.

Behaviour: a programmatic `set`/`select` changes the model without firing a
control callback; a control callback writes through the same setter; text
synchronisation is silent because `set_text` emits no `Changed`; selection is
by stable id and survives sort, filter and reload.

PR #66, runtime rebinding. Additive:

- `command::RESERVED_SHORTCUTS`, `RebindError`, `RebindWarning`,
  `contexts_overlap` and `apply_bindings`.
- `CommandRegistry::rebind`, `shortcut` and `set_binding_context`.
- `controls::ShortcutRecorder` with `shortcut`, `focus_handle`, `on_record` and
  `disabled`.

Behaviour: a rebind refuses a conflicting chord in an overlapping context and
keeps the old value; a reserved shortcut is accepted with a warning;
`apply_bindings` replaces the GPUI command bindings and preserves non-command
bindings, so a new chord dispatches immediately.

PR #12, device-pixel snapping. Additive:

- `paint::snap_edge`, `paint::snap_stroke_center` and `paint::snap_bounds`.
- `paint::hline` and `paint::vline` snap both ends; `bevel`, `thumb`, `field`,
  `focus_ring`, `selection`, `chrome` and `resize_grip` snap their bounds.

Behaviour: painted quads start and end on device edges; centred strokes sit on
device centres; a hairline is a whole number of device pixels
(`round(scale).max(1)`) at any scale. Existing goldens shift by up to one device
pixel and were re-accepted; 175% goldens are new.

PR #62, preference storage. Additive:

- `silvestris_core::prefs`: `Versioned`, `Migration`, `migrate`, `Storage`,
  `FileStorage`; `Store::open_versioned`, `Store::with_storage`,
  `Store::with_storage_versioned`, `Store::map_storage`, `Store::version`.
  `Store::save` and `writable` are unchanged.
- `silvestris::prefs::Preferences<T>` entity with `open`, `with_storage`,
  `value`, `writable`, `schema_version`, `set`, `on_change`, `subscribe` and
  `save`.

Behaviour: ordered migrations run on the raw JSON object and preserve unknown
fields; a migration gap or a malformed file disables saving; a document newer
than the code is kept as written; `set` writes through and notifies keyed and
global subscribers plus entity observers.

PR #21, modal sessions. Additive:

- `modal::ModalSession` with `scope`, `phase`, `is_active`, `allows`,
  `focus_stops`, `with_focus_stops`, `focus_first`, `on_end`, `owner`;
  `ModalScope`, `ModalResult` and `ModalPhase`.
- `modal::present`, `active`, `is_modal`, `finish`, `escape_session`,
  `cycle_focus`, `cancel_owned`, `command_allowed`, `install` and `reset`.
- `ConfirmationFocus::handles`, so a sheet can declare its Tab stops to a
  session.

Behaviour: a document session blocks its own window's command chain and leaves
siblings usable; an application session blocks every window and stacks over a
sheet; a second document session queues; closing the owner cancels active and
queued sessions with `Cancelled`; `ResponderChain::dispatch` and `present`
return disabled for blocked commands.

PR #60, transient surfaces. Additive:

- `transient::TransientLayer` with `push`, `close`, `close_top`, `escape`,
  `dismiss_top`, `dismiss_all`, `dismiss_surface`, `is_open`, `top_*`,
  `allows_tooltip`, `clear` and `set_chain`.
- `TransientKind`, `DismissPolicy`, `DismissTrigger`, `TransientHandle`,
  `SurfaceId` and `CancelToken`.
- `transient::shield`, `dismiss_on_pointer`, `dismiss_on_scroll`.
- `PopupMenu::set_layer`, `PopUpButton::layer`, `ComboBox::layer` and
  `tooltip::attach_in`. `PopupMenu::is_open` keeps its no-argument form;
  `is_open_in(&App)` is the authoritative check against a shared layer.

Behaviour: one stack per window; only the innermost surface dismisses; a closed
surface cancels its token; the responder chain freezes on the first surface and
restores focus when the last closes. An open submenu now takes the first Escape
instead of the menu closing whole.

PR #20, window roles. Additive:

- `window_role::WindowRole` (`Document`, `Utility`, `Panel`, `Preferences`,
  `Dialog`, `Assistant`, `About`) with `spec`, `titlebar_height`, `autosave_key`
  and `ALL`; `RoleSpec`.
- `window_role::role_options`, `centered_on`, `cascade_from`,
  `restore_bounds`, `save_frame`, `open_role`, `open_owned_role`;
  `OwnedWindow`, `FrameStore` and `OwnedWindows`.
- `WindowFrame::role`, which makes the drawn title bar agree with the role.

Behaviour: a role decides resizability, live title-bar buttons, title
visibility, parent centring, the frame-autosave key and, for utility windows, a
16 px title band. Fixed roles draw only the close button. `OwnedWindows` closes
its children when the application's close policy calls `close_all`; Silvestris
does not install an `on_window_should_close` handler of its own.

PR #57, responder chain. Additive:
 Additive:

- `responder::Responder` with `handles`, `state` and `perform`, plus
  `ResponderState` (`enabled`, `checked`, `title`).
- `responder::ResponderChain` with `push`, `replace`, `resolve`, `validate`,
  `dispatch`, `present`, `begin_transient` and `end_transient`.
- `responder::ResponderEntry` with `new`, `custom`, `active_when`,
  `when_focused` and `capture_keys`; `ResponderScope`.
- `responder::FocusReturn` for transient surfaces and sheets.
- `CommandButton::chain`, `CommandScope::chain`, `PopupMenu::set_chain`.

Behaviour: a resolved responder's `state` decides enabled and label for menu,
toolbar and shortcut presentation; a claiming but disabled layer answers
`Disabled` without falling through; a dropped responder entity passes outward.

## After 3ac1105

Pending #74 integration commit:

- Added `preferences::TabView::scroll_handle`, which accepts a caller-owned
  `gpui::ScrollHandle` instead of always using a keyed internal handle.
  Additive.
- Changed `Shell` inspector focus to transfer in `window.defer` after the
  frame, on open and close. A focus change made by another view in the same
  frame wins. Consumers that relied on focus being set synchronously inside
  `Shell::render` should read `Window::focused` after the frame.
- Changed `Shell`'s inspector key handler to ignore printable keys while a
  `SilvestrisTextField` is in the focus context. Consumer key callbacks no
  longer see those keys when a text field has focus.
- Added regression tests in `crates/silvestris/src/shell.rs`. No removals.

## 06a88af..3ac1105

PR #71, “Allow consumers to override theme font families”. Additive:

- `silvestris_core::FontOverrides` with `new`, `ui`, `mono`, `is_empty`.
- `Theme::apply_font_overrides` and
  `silvestris_core::theme_with_font_overrides`.
- `silvestris::set_global_font_overrides`, `set_font_overrides`,
  `clear_font_overrides` and `font_overrides`.

A per-theme override wins over the global one. Sizes, colours and metrics are
unchanged.

## bf52d64..06a88af

PR #78, roadmap and issue template. Documentation only; no code change.

## d1db8f0..bf52d64

PRs #79 and #80.

- PR #79 reformatted the workspace with `rustfmt.toml` (`max_width = 100`, LF
  line endings). No API change.
- PR #80 added `silvestris::test_support` with `scenario`, `fixtures` and
  `script`. `test_support` is now available under `cfg(test)` as well as the
  `test-support` feature; the existing `test_support::initialize` helper moved
  into the module. Additive.

## Before d1db8f0

No stable public API. The source is a work-in-progress shell for Lotus issues
#118–#138. Treat every type as movable and pin an exact commit.
