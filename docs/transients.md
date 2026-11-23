# Transient surfaces

One `TransientLayer` per window owns every surface that opens above the
content: menus, submenus, popovers, combo-box lists, tooltips and completion
lists. A surface does not keep its own open flag or its own outside-click rule;
it asks the layer.

Evidence: HIG p. 165 (menu behaviour), p. 230 (transparent panels), p. 163-165
(help tags). The corpus has no transient-specific capture for the stack rules,
so they are stated as behaviour rather than measured.

## Kinds and policy

| Kind | Default policy |
| --- | --- |
| `Menu`, `Submenu` | outside click, Escape, window deactivation, scroll |
| `Popover` (combo list, suggestions) | same as a menu |
| `Completion` | same as a menu |
| `Tooltip` | window deactivation and pointer/focus loss; not outside click or Escape |

`DismissPolicy` is a plain struct; `SurfaceSpec::policy` overrides the kind's
default. `DismissTrigger` names the event, and `DismissPolicy::allows` answers
whether that surface closes for it.

## Using it

```rust
let layer = cx.new(|cx| TransientLayer::new(window, cx));
layer.update(cx, |layer, _| layer.set_chain(chain.clone()));

// Open. The caller focuses its panel afterwards; the layer remembers the
// focus to return to.
let handle = layer.update(cx, |layer, cx| {
    layer.push(
        SurfaceSpec::new(TransientKind::Menu)
            .anchor(anchor_bounds)
            .owner(ResponderScope::Window)
            .title("View"),
        window,
        cx,
    )
});

// Render the surface while it is open.
if layer.read(cx).is_open(handle.id()) { /* panel */ }
```

The surface owner renders the panel and a full-window shield, because it owns
the geometry and z-order. It does not implement dismissal: attach
`transient::shield(layer.clone(), id, window, offset)` (full window) or
`dismiss_on_pointer` and `dismiss_on_scroll` to its own shield, and call
`TransientLayer::escape` from its key handler. The shield and panel are mounted
as `deferred` overlays; there are no per-control z-index constants.

`TransientLayer::dismiss_surface(id, trigger, ...)` only acts when `id` is the
innermost surface, so a stale shield cannot close a newer surface above it.
`dismiss_top` applies one trigger to the innermost surface. `dismiss_all`
applies one trigger to every surface that allows it; window deactivation uses
that so a menu and its submenu close together. Closing a surface that is not
open is a no-op, so a late async completion is harmless.

## Cancellation

Every `push` returns a `TransientHandle` with a `CancelToken`. Clone the token
into a spawned task, and check it before touching state or the UI:

```rust
let token = handle.cancel_token();
cx.spawn(move |this, cx| async move {
    let results = load(cx).await;
    if token.is_cancelled() {
        return;
    }
    this.update(cx, |this, cx| this.show(results, cx));
});
```

Closing the surface cancels the token. That is the answer to "the search or menu
population finishes into a surface that no longer exists".

## Escape order

Escape is resolved once:

1. the innermost transient surface, if its policy allows Escape;
2. the sheet or modal layer (#21), which is not a transient;
3. the application.

`TransientLayer::escape` implements step 1 and returns `false` when the stack is
empty or the top surface does not allow Escape, so the caller passes the key to
the sheet and then the app. A menu with an open submenu closes the submenu on
the first Escape and itself on the second; that is a surface-level rule inside
the menu (`path` pops before the menu closes), and the layer's stack still
decides when the last surface is gone.

Tab is not Escape: a menu closes on Tab because focus moves on, which is a menu
rule, not the dismissal order.

## Focus and the command target

Opening the first surface freezes the responder chain (#57), so commands still
target the control that had focus when the surface opened. Closing the last
surface restores the recorded focus. A stack of menus freezes once and unfreezes
when it empties; nested pushes do not re-record the focus.

## Tooltips

GPUI owns hover timing, long-press and the tooltip window; replacing that would
duplicate platform input. `tooltip::attach_in(element, text, layer)` therefore
asks the layer whether a tooltip may appear and returns an empty view when a
menu, popover or completion list is open, which is the regression trap "tooltips
must not appear over an open menu". `tooltip::attach` stays for elements that do
not share a layer.

## Migration

| Surface | State |
| --- | --- |
| `menu::PopupMenu` | on the layer; submenu Escape pops one level; outside click, scroll and deactivation go through it |
| `controls::PopUpButton` (combo list) | on the layer; `ComboBox` passes its shared layer through |
| `tooltip::attach_in` | gated by the layer; GPUI still owns lifetime |
| Preferences search results | no results surface exists in `preferences.rs`; the toolbar accepts a search field. When the dropdown is built (#76/#77) it belongs on this layer. |

`PopupMenu::set_layer` and `PopUpButton::layer` opt into a window's shared layer.
Without one, each control creates a private layer, so a single pop-up still
works but two controls in a window do not exclude each other.

## Tests

`transient.rs` covers: Escape closes the innermost surface first and then
returns false; closing cancels the token; a late close is a no-op; a tooltip
ignores outside click and Escape but closes on deactivation; tooltips are
suppressed while a menu is open; the chain freezes on the first surface and
unfreezes on the last; only the innermost surface can be dismissed by a shield.

`menu.rs` covers open menu → open submenu → Escape (submenu only) → Escape (menu,
focus restored). `popup_button.rs` covers Escape, outside click without leaking
the click to the background, row choice, and the surface kind on the shared
layer.
