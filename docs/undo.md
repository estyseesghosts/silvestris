# Undo and redo

`undo::UndoManager` is the one history mechanism. A text control owns a local
stack, a document owns one, and an application can own one; the responder chain
(#57) picks the innermost active stack and the menu title comes from the same
`Responder::state`.

## Manager

```rust
let history = cx.new(|_| UndoManager::new());
history.update(cx, |manager, _| {
    manager.record(
        "Rename",
        None,
        move |cx| { /* undo */ },
        move |cx| { /* redo */ },
    );
});
```

- `record(name, coalesce, undo, redo)` pushes one step. `coalesce` re-amends the
  top instead of adding a step when the previous entry has the same key.
- `begin_group(name)` / `end_group()` make several steps undo and redo as one;
  the group's undo runs the children in reverse and its redo in order.
- `limit` (default 100, minimum 1) drops the oldest undo entries. `clear()`
  empties both stacks; call it after save or revert as the app asks.
- `undo`/`redo` run the closures with `&mut App` outside the manager borrow, so
  a callback can update entities and the manager itself.
- `history().undo_name()` / `redo_name()` and `can_undo`/`can_redo` drive menus
  and toolbars.

`Responder for UndoManager` claims `edit.undo` and `edit.redo`, disables an
empty stack and projects `Undo Typing` / `Redo Typing`. `undo::entry(manager,
scope, name)` builds the chain entry: Control for a focused text field, Document
for a document, Application for an app-wide history. The focused field wins
because Control is pushed innermost.

## Text history

`TextHistory` is the local stack a `TextField` or `TextArea` owns. It records a
`text::Change` (replaced range, removed text, inserted text, composing flag):

- undo replaces the inserted text with the removed text;
- redo does the reverse;
- composing changes are skipped, so IME updates do not create steps;
- contiguous or cumulative typing amends one `Typing` entry. Platform input
  may send `"a"` then `"ab"` over `0..1`; both shapes coalesce.
- a backspace or delete is its own `Delete` step.

`TextField::history()`/`undo_entry()` and `TextArea::history()`/`undo_entry()`
expose it. `after_history_change` emits the client's `Changed` event and asks
for a repaint.

## Apps with their own history

chaus keeps a Ropey editor and its own undo. It does not need `TextHistory`: it
pushes entries with `record` from its document model at Document scope, or
implements an undo provider in the app. The manager only needs reversible
closures, so a document can also group a drag or a multi-field form edit with
`begin_group`.

## Tests

`undo.rs`: reverse/forward order; grouped undo as one step; coalescing and the
limit; titles and clear; responder claims and disabled state; limit floor 1.
`TextField`: typing `ab` is one `Typing` entry, undo empties the field, redo
restores it; a chain with a Document stack and a focused field's Control stack
presents `Undo Typing` and leaves the document stack untouched.
`TextArea`: typing coalesces, undo/redo restore the text, a backspace is a
separate step and undoing it restores the character.
