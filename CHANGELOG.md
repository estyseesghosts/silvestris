# Changelog

Silvestris has no released crate version. Consumers pin commits, so entries are
keyed by commit range. A range lists public API changes a consumer must handle
when moving its pin. Behaviour-only changes are listed because consumers can
depend on focus, key routing and layout.

## After 216de9b

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
  `tooltip::attach_in`; `PopupMenu::is_open` now takes `&App`.

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
