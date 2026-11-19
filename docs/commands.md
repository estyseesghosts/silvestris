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

bindings(context, cx) builds startup key bindings through GPUI KeyBinding::load,
which resolves secondary- to Command on macOS and Ctrl elsewhere. Empty
contexts and shortcuts are errors. Normalized collisions (for example
secondary-r versus ctrl-r on Windows) are errors; no partial bindings are
installed. The caller installs the returned vector with cx.bind_keys.
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
