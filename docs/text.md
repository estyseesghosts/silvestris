# Text editing core

`text::EditSession` is the one editing engine behind `TextField` and
`TextArea`. Token fields, editable table cells and the find bar use it the same
way; a consumer with its own editor can implement `text::Buffer` for a rope and
reuse the session without adopting a Silvestris widget.

## What the core owns

- `Buffer` (string-backed `StringBuffer` by default) and byte offsets.
- `Selection` with an anchor and a head, so reversed selections, Shift-click
  and mouse drags are the same state.
- Grapheme, word, physical-line and paragraph boundaries.
  `floor_boundary` keeps a caret from landing inside a grapheme, including a
  multi-codepoint emoji.
- UTF-8↔UTF-16 mapping for the platform input handler and accessibility.
- Marked text for IME composition (`marked_range`, `replace_marked`, `unmark`).
- Masked mode: one bullet per grapheme for display, no copy text, and word
  movement treats the whole value as one word.
- `Change { range, inserted, composing }` for change notification and undo
  grouping (#39); `EditKind` classifies an edit for a menu title.
- `ClipboardPolicy`: a field flattens newlines on paste, an area keeps them.
- `CaretBlink` state; each view owns the timer and stops it when focus leaves.

The view owns focus, painting, horizontal or vertical scrolling, wrapping and
the `EntityInputHandler` trait. GPUI implements that trait per entity, so each
control has a thin forwarding impl; all of its decisions call the same session
methods. The alternative, a generic adapter entity owning the session, would
put the entity boundary in the wrong place and force every view to reach
through it for normal editing.

## TextField

Rebuilt on the session. Its boundary, word, UTF-16, selection, clipboard and
IME methods delegate to it; the renderer keeps the measured single-line bezel,
horizontal caret scrolling and the placeholder handling. `TextFieldEvent` is
unchanged: `Changed` only for user edits, `Submitted` on Return, `EndedEditing`
on focus loss. `set_text` stays silent.

Existing predicates keep working because the field's context stack is
`SilvestrisTextField SilvestrisText`; new bindings use the shared
`SilvestrisText`.

## TextArea

New, defined from scratch from the #73 inventory; the discarded prototype's API
and namespace do not return.

- Wrapped layout via the platform text shaper; caret and selection use
  `WrappedLine::position_for_index` and `index_for_position`.
- Up/Down move by visual row and keep the x position; Home/End use the physical
  line.
- Grows from `min_rows` to `max_rows` logical rows, then scrolls vertically to
  keep the caret visible. Mouse wheel scrolls; horizontal scrolling is not
  offered.
- Composer contract: Return and Cmd/Ctrl-Return emit `Submitted`; Shift-Return
  inserts a newline. A failed submit does not touch the buffer, so the app's
  draft survives.
- No masked or search mode: those are single-line features in 10.6.

## One keymap

`actions!(silvestris_text, [...])` holds the shared actions, including Up, Down,
SelectUp, SelectDown and Newline. `text_field::bind_keys` binds them once for
every `SilvestrisText` client; the field and area each handle the actions they
support, and the field ignores the multiline-only ones. There is no
`silvestris_text_area` namespace.

## Masking and password fields

`EditSession::set_masked(true)` makes `display_text` bullets, `copy_text` return
`None`, and IME `text_for_range` return bullets from the field. `copy` and `cut`
bell instead of writing the clipboard. The area has no masked mode.

## Tests

- `text.rs`: grapheme step over a family emoji; reversed selection; surrogate
  pairs through UTF-16; replacement `Change`; backspace over a line break;
  marked-text composition; masked display/copy/word boundaries; clipboard
  policy; physical line and paragraph bounds.
- `text_area.rs`: Return submits and Shift-Return inserts; exact whitespace,
  tabs and newlines round-trip; Up/Down across physical lines and Home/End on
  the physical line; backspace at the end; paste keeps newlines.
- The existing text-field interaction suite still passes against the rebuilt
  engine (typing, IME, masking, key isolation, focus per window).

The shared engine means the conformance cases above are the same code path for
every client; a new client gets them by using `EditSession`.
