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

Build a toolbar from the typed item model and mount it through
`WindowFrame::toolbar_component`; that host chooses the height from the active
theme and display mode. Do not stretch controls or assemble arbitrary children
into the toolbar.

```rust
let toolbar = Toolbar::new(
    "document-toolbar",
    "Document",
    commands.clone(),
    vec![
        ToolbarItem::toggle("sidebar", "Sidebar", "view.sidebar")
            .image(TemplateImage::Sidebar)
            .toggle_help("Hide the sidebar", "Show the sidebar"),
        ToolbarItem::segmented("view", "View", view_segments, None::<&str>, on_view),
        ToolbarItem::flexible_space("search-gap"),
        ToolbarItem::search("search", "Search", search_field),
    ],
)
.chain(responders.clone())
.display_mode(ToolbarDisplayMode::IconAndLabel);
let frame = WindowFrame::new("Document").toolbar_component(toolbar, cx);
```

Items have stable IDs and external descriptive labels. Use command-backed
buttons/toggles, homogeneous icon-or-text segmented controls, pop-down menus,
real search fields, groups, and fixed/flexible spaces. `priority` determines
which measured natural-width items move to overflow first; overflow retains
command reachability and returns focus to an overflowed search field. Rectangular
textured items are the default; `ToolbarItemStyle::Capsule` opts into the
capsule treatment. Prefer a consistent bezel style within one toolbar. The
display mode supports Icon Only, Icon & Label, and Text Only. Toolbar control
spacing is 8 px.

Build a bottom bar from status values, fixed/flexible spaces, and small
rectangular or square gradient controls, then mount it through
`WindowFrame::bottom_bar_component`. A bottom bar is not a second toolbar:
it holds controls that affect the window body's contents or organization, not
the most frequent commands (HIG pp. 212–214).

```rust
let bar = BottomBar::new(
    "document-bottom-bar",
    "Document status",
    vec![
        BottomBarItem::status("count", BottomBarStatus::new("count", "12 items")),
        BottomBarItem::flexible_space("gap"),
        BottomBarItem::command("add", "Add", "list.add").image(TemplateImage::Add)
            .style(BottomBarControlStyle::Gradient),
        BottomBarItem::command("remove", "Remove", "list.remove").image(TemplateImage::Remove)
            .style(BottomBarControlStyle::Gradient),
    ],
)
.registry(commands.clone())
.chain(responders.clone());
let frame = WindowFrame::new("Document").bottom_bar_component(bar, cx);
```

`BottomBarSize::Regular` uses regular controls and `BottomBarSize::Small` uses
small controls; the host takes the height from the active theme. Status values
carry their own text, optional help tag and optional activation callback; they
truncate before controls shrink. The application owns status computation and
omits zero-value entries. The bar reserves the resize-grip hit region, and its
material follows the active/inactive window appearance.

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

## Split views

`SplitView` is the reusable multi-pane splitter inside a workspace. It supports
horizontal or vertical orientation, two or more panes, thin (1 px, 2 px hot
zone) and thick (9 px, dimple) Aqua dividers, per-pane minimums and maximums,
explicit collapse, pointer and keyboard resizing, and exact measured pane
bounds through `on_bounds`. It is controlled: pass a `SplitState`, persist the
new state from `on_resize`, and pass it back. Persist only divider IDs and
positions. Do not turn it into a tiling tree or use it to replace Shell's
sidebar/workspace/inspector policy. See [split-view.md](split-view.md).

## File browser core

`file_browser` owns the provider protocol, entry data, one `FileBrowserModel`
for location/history/filter/view/sort, generation-cancelled async loading and
navigation events. Open/Save/Choose panels consume it; a file-manager app can
instantiate it directly. `file_panel::{model, provider}` are transitional
re-exports and the panel keeps extension, sheet and modal policy. See
[file-browser.md](file-browser.md).

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

## Drag and drop

`drag::DragSession` is the shared session mechanics, not an application policy.
Begin it on mouse-down with the payload and source id; it stays pending until
the pointer moves three pixels (HIG p. 123). `pointer_moved` records modifiers
but `operation`/`drop` sample Option only at drop time, so Option never latches.
`DropTargets::register` takes a predicate over the complete payload; `negotiate`
reports accepted or rejected feedback and a target highlights only when every
dragged item is acceptable. `Insertion` carries a gap, an item or a text offset.
`DragImage::transparent` is the Aqua drag image, and `autoscroll` returns a
delta only for the frontmost source window. Cancellation releases capture.

The consumer decides what a drop means: Finder provider policy, Lotus retile,
Sandcat attachment or Chaus document open. Do not put container, volume or
filesystem rules in the session.

## Preferences

Use `PreferencesNavigator` for pane selection and history,
`PreferencesToolbar` for back/forward and Show All, `PreferencesOverview` for
category bands, and `TabView` plus `Form` for pane content. The application owns
the pane list, the values and persistence.

`PreferencesController` owns the window-level behavior: it projects the current
pane's title, restores and stores the last selection, retargets a pane resize
instead of letting an older one finish, and reports whether only the pane body
must scroll. Fit the pane after layout:

```rust
let mut controller = PreferencesController::new("Preferences", panes, sizing)
    .with_persistence(
        |cx| store.read(cx).last_pane.clone(),
        |pane, cx| store.update(cx, |store, cx| store.set_last_pane(pane, cx)),
    );
controller.restore(cx);
// Measure the natural-height pane body, not a size_full() one.
let body = measure_content(pane_body_natural().child(form), move |size, window, cx| {
    controller.measure(size, window, cx);
});
// In render, advance an in-flight resize:
controller.tick(window);
```

The window stays fixed beside the pane: it takes the measured natural height,
clamped by the display's usable region. When the pane is taller than that
region, `pane_viewport(content, controller.pane_scrolls())` scrolls only the
pane. `pane_body_natural()` is the measuring variant of `pane_body()`. Focus
returns to the handle passed to `PreferencesController::focus_return` after a
pane change.

A simple one-pane consumer does not need an overview or categories: pass one
`PreferencePane`, render a `Form` in a `WindowRole::Preferences` window, and
keep `PreferencesToolbar` out if it has no pane history.

`TabView::on_overflow` reports how much wider and taller the pane needs; resize
the window to that. `Form` handles right-aligned labels and label-less rows.
`FormRow` states distinguish normal, disabled/dependent, informational note,
warning and validation error; warnings and errors share one message slot below
the control so the control does not move sideways. Use
`FormRow::dependent_on(Binding<bool>)` for a row enabled by another setting, and
keep the last valid model value when a `TextBinding` parse fails.
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
