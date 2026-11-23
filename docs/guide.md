# Composition guide

How an application is put together from Silvestris: startup, window, shell,
commands, controllers and preferences. It also states the ownership boundary,
so a missing piece becomes an upstream issue rather than a private widget.

## Ownership boundary

Silvestris owns generic interaction: controls and their states, focus and key
routing, transient surfaces, modality, window chrome, layout planning, themes
and preference *widgets*.

The application owns domain state and policy: documents and sessions, file
formats, persistence locations, command callbacks, validation rules and
app-specific focus rules (Lotus guest input isolation, for example).

GPUI owns platform paint and input. Build on its focus handles, actions and
input handlers. Do not replace them.

## Startup

Call `silvestris::init` once before opening a window:

```rust
fn main() {
    gpui_platform::application().run(|cx: &mut gpui::App| {
        silvestris::init(silvestris::ThemePreference::default(), cx)
            .expect("register fonts and theme");
        // Register commands before the first window renders.
    });
}
```

`init` registers the bundled fonts, installs the theme global and binds the
text-editing keys. Loading a saved preference before this call is the
application's job; pass the result in.

## Window

`chrome::window_options` returns the platform options; `chrome::open_window`
opens a window whose root is a `WindowFrame`. Use
`chrome::fixed_window_options` and `WindowFrame::fixed_size` for preferences
and other fixed-size windows.

Choose the window role before choosing its content. A document window gets a
size control and a title; a utility panel gets neither; a dialog gets no zoom.
Window roles are #20; until that lands, encode the role in these options and
keep the policy in the application.

## Shell

`Shell` composes a sidebar, workspace, optional inspector rail and one
inspector subtree. Pass both preference structs and all content; keep the
preferences in application state and update them from
`on_sidebar_change`/`on_inspector_change`.

Use `Shell::measured` when a child must lay out to the planned geometry (a
preview that must match the workspace width, for example). The builder receives
the `ShellPlan` and allocated size; render `plan.workspace` and
`plan.dock.or(plan.overlay)`, never a second planner. Use `Shell::new` when the
children are size-agnostic.

Mount transient menus above the shell, not inside inspector content. Escape
dismisses an overlay inspector; `on_inspector_key_down` runs first and returns
`true` to handle an application shortcut or close an application popup.

## Commands

One `Entity<CommandRegistry>` per application. Register one callback per stable
`CommandId`; publish app-owned descriptor snapshots. Observe the registry from
each composing view and call `cx.notify` so labels, enabled and checked states
stay current.

```rust
let registry = cx.new(|_| CommandRegistry::default());
registry.update(cx, |registry, cx| {
    registry.register(
        CommandDescriptor::action("file.save", "Save").shortcut("secondary-s"),
        |_, window, cx| save(window, cx),
        cx,
    )?;
});
cx.bind_keys(registry.read(cx).bindings("AppShell && !SilvestrisTextField", cx)?);
```

Wrap the responder boundary in `CommandScope`. `CommandScope` consumes known
command ids, including disabled ones, before they reach a guest view. Text
fields and transient surfaces take priority through keymap predicates.
`CommandButton` and `PopupMenu` read the same descriptors; do not keep a second
label or enabled-state copy.

`CommandRegistry::invoke` is the single entry for toolbars, menus, context
menus and shortcuts. It rejects disabled or removed commands before cloning the
callback, and the callback runs after the registry read ends so it can update
the registry. The application publishes the resulting state; the registry does
not toggle speculatively.

## Controllers

A controller is a GPUI entity that owns one coherent piece of state and emits
events; a view subscribes and renders. Keep the model in the controller and the
widgets in the view. Callbacks mutate the controller, then notify.

Programmatic updates must not fire user callbacks. Distinguish a user action
from a model update in the controller API (for example `select` versus
`set_selection`), and test both.

Until #61 lands, type the controller API yourself and keep collection state in
the controller rather than in the view. Undo (#39), document identity and dirty
state (#37) are separate controllers because they outlive one view.

## Preferences

Use `PreferencesNavigator` for pane selection and history,
`PreferencesToolbar` for back/forward and Show All, `PreferencesOverview` for
category bands, and `TabView` plus `Form` for pane content. The application owns
the pane list, the values and persistence.

`TabView::on_overflow` reports how much wider and taller the pane needs; resize
the window to that. `Form` handles right-aligned labels and label-less rows.
`TextFieldEvent::EndedEditing` lets a form commit when focus leaves a field.

The preferences store with schema versions and migration is #62. Until then,
persist a versioned struct in the application and treat unknown fields as
forward-compatible.

## Testing

Enable the `test-support` feature and drive the composed view through
`test_support::scenario::Scenario`: open windows, focus handles, type, press
chords, click stable `debug_selector` names, advance the test clock and assert
focus, bounds and window order. `docs/testing.md` states what this proves and
what belongs to native platform testing.

## Common mistakes

- Rendering a control's own popup, Escape or outside-click handling. Use the
  transient layer (#60).
- Keeping a label or enabled state in a view instead of reading the command
  registry.
- Branching interaction on the theme. Themes differ in tokens and materials
  only.
- Adding a consumer name, policy or file format to a Silvestris crate.
- Copying a control into the application instead of filing the gap.
