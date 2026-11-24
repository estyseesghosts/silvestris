# Documents and the close decision

Silvestris owns the document protocol, not the file format or IO. An
application implements `document::Document` for its model; the framework
supplies identity, dirty state, untitled numbering, the edited close-button
dot and the close decision.

Evidence: HIG p. 197 and 221 (title, dirty dot, closing), p. 199 (proxy icon),
p. 246 (save/close/quit dialogs). The save sheet and Save panel are #22/#23 and
#29; this module is the policy they call into.

## Model

```rust
impl Document for MyDocument {
    fn display_name(&self) -> SharedString { self.name.clone() }
    fn url(&self) -> Option<String> { self.path.clone() }
    fn is_dirty(&self) -> bool { self.dirty }
    fn save(&mut self, cx: &mut Context<Self>) -> SaveResult { /* app IO */ }
    fn save_as(&mut self, cx: &mut Context<Self>) -> SaveResult { /* #29 hook */ }
    fn revert(&mut self, cx: &mut Context<Self>) { /* app IO */ }
}
```

`SaveResult` is `Saved`, `Failed(message)` or `NeedsPanel`. The default
`save_as` calls `save`, so an app without Save As still type-checks.
`DocumentHandle` erases the concrete type for `DocumentController`.

## Controller

`DocumentController` runs one window's close decision:

- `request_close(cx)` → `Close` for a clean document, `Prompt` for a dirty one,
  `Saving` if a save is already in flight (a second close never prompts twice).
- `resolve_prompt(choice, cx)`:
  - `Cancel` → `Stay`, still dirty;
  - `Don't Save` → `Close`, still dirty;
  - `Save` → runs the document's save, then `Close`, `Saving` (needs a panel)
    or `Stay` (failed).
- `resolve_save(result)` completes a `NeedsPanel`: `Saved` closes, `Failed`
  stays.
- `revert(cx)` is the app's Revert command; `title(cx)` is the render input.

A failure returns to the idle state so the user can retry; a save in flight
does not. The app presents the prompt as its own content in a
`modal::ModalSession` (#21), and opens the Save panel (#29) when `Saving`
follows `NeedsPanel`.

## Title and dirty dot

`DocumentTitle { name, edited, url }` gives `window_title()` (the name only, as
10.6 shows), `edited` and `proxy_tooltip()`. `WindowFrame::edited(true)` paints
the dot in the close button and exposes `traffic-Close-edited`; the colour is
`theme.colors.text`, not new artwork. `goldens/renders/<appearance>/<scale>/
window-frame-edited.png` covers all four appearances at 100/125/150/175/200%.

The proxy icon itself is #48; `url()` is the data a proxy will use.

## Untitled numbering

`Documents` tracks open names. `next_untitled()` returns `Untitled`, then
`Untitled 2`, `Untitled 3`, skipping any name still open; `close(name)` releases
it. A closed `Untitled 2` may be reused, an open one never is.

## App-owned policies

Conflict and recovery prompts stay in the app. `DocumentController` only
decides what a close means; chaus keeps its resolve/save/close policy and can
call `resolve_save` after its own conflict flow. The controller does not read or
write files, does not autosave and has no versions (10.7).

## Tests

Six tests: clean close; dirty close with Cancel, Don't Save and Save; a save
that needs a panel stays saving and a second close returns `Saving`; a failed
save keeps the window open and returns to a promptable state; untitled numbers
skip open names and are released; Revert clears dirty and the title reflects it.
`chrome.rs` tests the edited close button selector and the clean frame.

The gallery has no separate document view: the edited window-frame golden and
the controller tests are the sample, because the protocol is headless and the
sheet/panel visuals belong to #22/#23/#29.
