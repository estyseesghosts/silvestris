# Shared command dispatch

The GPUI command adapter implements #129's shared registry/dispatch portion.
MenuModel/MenuEntry and CommandDescriptor remain GPUI-free in core::command.
Popup menu rendering is documented in menus.md; native menu integration remains open.

Create an Entity<CommandRegistry> and register one callback per stable
CommandId. Duplicate registration returns an error. Publish app-owned
descriptor snapshots through publish; unknown IDs return an error.
Observe the registry from each composing view and call cx.notify so labels,
enabled/checked states and menus update when the model changes.

CommandRegistry::invoke is the single entry for toolbar, menu, context and
shortcut actions. It reads the current descriptor and rejects disabled or
removed commands before cloning the callback. The callback runs after the
entity read ends, so it can update the registry itself. Invocation returns
Dispatched, Disabled or Missing. It does not toggle state speculatively;
the application publishes the actual resulting state.

CommandButton reads its label (including ellipsis), enabled and checked
state from the descriptor and invokes the ID. Missing commands render as
disabled. Buttons default to textured style. The consumer can choose another
ButtonStyle/ControlSize. Tooltip/icon rendering remains separate work.

CommandScope provides an explicit responder boundary for InvokeCommand
actions. It consumes disabled and removed commands too, so those invocations
do not fall through to a guest-input handler. It does not own a focus handle;
focused descendants route actions through it. Transient menus and text fields
must take precedence in the binding context.

## Responder chain

`CommandRegistry` answers "what does this command mean and what is its
metadata". It does not answer "which object should handle it right now". That
is `responder::ResponderChain`, one per window.

Build the chain from the outside in, then attach it to the surfaces that
invoke:

```rust
use silvestris::responder::{Responder, ResponderChain, ResponderEntry, ResponderScope};

let chain = cx.new(|_| ResponderChain::new());
chain.update(cx, |chain, _| {
    chain.push(ResponderEntry::new(ResponderScope::Application, "app", &app_delegate));
    chain.push(ResponderEntry::new(ResponderScope::Document, "document", &document));
    chain.push(
        ResponderEntry::new(ResponderScope::Control, "field", &field)
            .when_focused(&field_focus),
    );
});
```

Resolution walks back from the most recently pushed layer. The first layer
that is active and claims the command wins; a claiming layer that reports
`enabled: false` answers `Disabled` instead of falling through. Layers whose
entity was dropped stop claiming, so a callback that outlives its window is a
no-op rather than a panic.

`Responder` has three methods: `handles` (ownership, which gives `stop`/`pass`
semantics), `state` (enabled, checked and an optional title such as "Undo
Typing") and `perform`. Validation and dispatch call the same `state`, so a
menu row and a shortcut cannot disagree. `CommandScope`, `CommandButton` and
`PopupMenu` accept `.chain(...)`; without one they use the registry alone, and
invocation falls back to the registered callback when no layer claims the id.

`ResponderEntry::custom` builds a layer from closures for an app delegate or a
validation-only layer. `capture_keys(allow)` builds the canvas role: the layer
swallows every command except an allow-list, which is how an emulator or editor
canvas receives no shell shortcut while the shell still receives the keys it
owns. That replaces hand-written `!SilvestrisTextField`-style exclusions.

A menu or sheet does not change which control commands act on. Call
`ResponderChain::begin_transient` before moving focus into the transient and
`end_transient` when it closes; while it is open the chain resolves against the
focus it captured. `FocusReturn` records and restores the focus handle for the
same lifetime.

`docs/guide.md` shows the composition; `docs/testing.md` covers what the
scenario tests prove. Tests cover focus-selected layers, disabled layers not
falling through, dropped entities, capture allow-lists, presentation titles,
two document windows with separate chains, and one command reached through a
button, a shortcut, a menu row and a direct call with the same result.

bindings(context, cx) builds startup key bindings through GPUI KeyBinding::load,
which resolves secondary- to Command on macOS and Ctrl elsewhere. Empty
contexts and shortcuts are errors. Normalized collisions (for example
secondary-r versus ctrl-r on Windows) are errors; no partial bindings are
installed. The caller installs the returned vector with cx.bind_keys.
bindings_where(context, include, cx) does the same for the descriptors
`include` accepts, so a consumer can leave out runtime-scoped IDs or keys
that another responder (guest input) must see first.
These bindings are startup snapshots: changing descriptor shortcut strings
does not rewrite GPUI's global keymap. Runtime rebind ownership is not yet
implemented. Enabled-state changes remain safe through invocation rechecking.

The gallery registers view.sidebar once. Its toolbar, footer and Ctrl+B /
Cmd+B route to that handler. Divider changes publish the resulting checked
state. The Gallery && !SilvestrisTextField predicate keeps the accelerator
out of text fields. No Lotus guest input is present in the gallery.

Callbacks should capture weak entity handles (Context::listener does this),
especially if they can reach their own registry, to avoid ownership cycles.
Lotus must additionally revalidate worker identity/epoch and operation
availability inside its command callbacks; generic enabled checks cannot
prove an emulator target remains valid.

Tests cover rendered button/shortcut/direct dispatch through one handler,
disabled and removed IDs, reentrant publication, duplicate registration,
normalized shortcut conflicts and no-op descriptor invalidation. They use
GPUI's mock platform, not native keyboard delivery.

Source: aqua-gpui handbook 12, "Menu hierarchy and command ownership" and
"Guest HID safety rules"; HIG menu behavior pp. 165-173; pinned GPUI
action.rs (parameterized actions), keymap/binding.rs (dynamic key loading),
and div.rs (action responders). Pending #129 work beyond the popup in menus.md includes menubar surfaces,
submenus, checkmarks, hover/type selection, dismissal/focus restoration,
native macOS integration and the Windows/Linux menu-bar equivalent.
