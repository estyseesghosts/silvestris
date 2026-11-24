# Window roles

A Silvestris window declares a role. The role fixes the policy that differs per
window type in 10.6: resizability, which title-bar buttons are live, whether the
window centres on its parent or hides when the app deactivates, and whether its
frame is saved. Consumers pick a role; they do not assemble `WindowOptions` by
hand or hand-roll a second title bar.

Evidence: HIG p. 191 (types of windows), p. 197-198 (title bar and buttons),
p. 220-221 (resizing, zoom, close), p. 227-230 (panels), p. 235-237 (dialogs and
sheets), p. 244 (preferences).

## Policy

| Role | Resize | Close | Min | Zoom | Centres on parent | Hides on deactivate | Autosave |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `Document` | yes | yes | yes | yes | no | no | `document` |
| `Utility` | yes | yes | yes | yes | no | no | `utility` |
| `Panel` | yes | yes | no | no | no | yes | `panel` |
| `Preferences` | no | yes | yes | no | yes | no | `preferences` |
| `Dialog` | no | yes | no | no | yes | no | none |
| `OpenPanel` | yes | yes | yes | yes | yes | no | `open-panel` |
| `Assistant` | no | yes | no | no | yes | no | none |
| `About` | no | yes | no | no | no (screen-centred) | no | none |

`RoleSpec` is the table as data; `WindowRole::spec()` returns it and the tests
assert the rows that matter (document and preferences, dialog and about, panel
and utility, OpenPanel). `WindowRole::ALL` lists every role so a gallery or a test can
iterate.

`Utility` and `Document` differ in the title bar: a utility window uses a
smaller band. The band height is an estimate recorded in
`docs/exceptions.md`; the 10.6 captures in the corpus have no utility window at
a known scale.

`Preferences` is not resizable and keeps close and minimize, with zoom
disabled: the 10.6 System Preferences capture shows that, and HIG p. 244 only
removes the resize control and zoom. `Dialog`, `Assistant` and `About` are not
resizable and draw only the close button (HIG p. 235: most application-modal
dialogs omit close/minimize/zoom). `OpenPanel` is the File > Open dialog
(HIG p. 245): a normal resizable window with a remembered 527 x 398 frame and
a 560 x 360 minimum (estimate). `Panel` is resizable and closable but not minimizable or
zoomable (HIG p. 227: an inspector belongs to its window). It records that it
hides on deactivation; whether the platform can float it above other
applications is a capability question (#50), not a role question.

## Opening a window

```rust
use silvestris::window_role::{centered_on, open_role, WindowRole};

let bounds = centered_on(parent.bounds, size(px(640.), px(460.)));
let handle = open_role(
    WindowRole::Preferences,
    "Preferences",
    bounds,
    cx,
    |window, cx| cx.new(|cx| PreferencesView::new(window, cx)),
)?;
```

`open_role` calls `chrome::open_window` with `role_options(role, title, bounds,
cx)`: resizable roles get the standard minimum size, fixed roles get none.

Drawn chrome agrees with the platform options. `WindowFrame::role(role)` makes
the title bar hide or disable the buttons the role does not have and drops the
title when the role says so; without it the frame draws the default
close/minimize/zoom trio. Attach the same role to both — `open_role` for the
platform window, `.role(role)` for the frame inside it.

## Frame persistence

Silvestris does not write a preference file. `FrameStore` is the seam the
application implements:

```rust
pub trait FrameStore {
    fn load(&self, key: &str, cx: &App) -> Option<Bounds<Pixels>>;
    fn save(&self, key: &str, bounds: Bounds<Pixels>, cx: &mut App);
}
```

`PreferenceFrames` implements it over the app's #62 `Preferences` entity, so
frames are saved in the app's own preferences file, are versioned and migrated
with it, and notify observers under the `window.frame.<key>` key. Add a
`SavedFrames` field to the preferences struct and point the adapter at it:

```rust
#[derive(Clone, Default, PartialEq, Serialize, Deserialize)]
struct AppPrefs {
    schema_version: u32,
    #[serde(default)]
    frames: SavedFrames,
}

let frames = PreferenceFrames::new(prefs.clone(), |p| &p.frames, |p| &mut p.frames);
let bounds = restore_bounds(WindowRole::Preferences, default, None, Some(&frames), cx);
// later, when the window moves or closes:
save_frame(WindowRole::Preferences, window, &frames, cx);
```

A malformed preferences file is read-only (#62): the frame stays in memory and
the entity reports the failed write. lotus's saved Settings bounds map onto
this by moving them into a `SavedFrames` entry with a migration.

`restore_bounds(role, default, parent, store, cx)` returns the stored frame when the
role has an autosave key and the store has it, otherwise `default` centred on
`parent` for centring roles, otherwise `default`.

A new window with no saved frame can cascade from the previous one instead:
`cascade_from(previous, 20.0)` offsets the origin by HIG p. 218's 20 px step and
keeps the size. The application chooses cascade or restore; Silvestris has no
screen bounds to clamp against and does not guess. `save_frame(role, window,
store, cx)` writes `window.bounds()` under the role's key and does nothing for roles
with no key (dialogs do not persist). Which key, and where it lives, remains the
application's choice; #62's preference store is the intended backend.

## Migrating from `window_options`

`chrome::window_options` and `chrome::fixed_window_options` stay as the thin
layer underneath: `role_options` calls `window_options` and then applies the
role's resizability and minimum size. New code picks a role instead:

| Before | After |
| --- | --- |
| `open_window(window_options(title, bounds, min, cx), cx, build)` | `open_role(WindowRole::Document, title, bounds, cx, build)` (minimum 320x200) |
| `open_window(fixed_window_options(title, bounds, cx), cx, build)` for preferences | `open_role(WindowRole::Preferences, title, bounds, cx, build)` |
| a fixed About or assistant window | `WindowRole::About` / `WindowRole::Assistant` |
| a hand-centred dialog | `open_owned_role(OwnedWindow::new(WindowRole::Dialog, ...), ...)` |

Give the frame the same role, `WindowFrame::new(title).role(role)`, so the
drawn title-bar buttons match the platform options. Keep calling
`window_options` only for a window no role describes, for example a custom
minimum size; that is a reason to propose a role, not a permanent escape.

## Owned windows

A dialog, sheet or inspector usually belongs to a document. `OwnedWindows`
records child handles and closes them together:

```rust
let mut owned = OwnedWindows::new();
open_owned_role(
    OwnedWindow::new(WindowRole::Dialog, "Unsaved changes", default_bounds, *parent_handle),
    &mut owned,
    Some(&store),
    cx,
    |window, cx| cx.new(|cx| Prompt::new(window, cx)),
)?;
```

`open_owned_role` restores or centres the frame and registers the handle.
`close_all(cx)` closes every child; a child already closed by the user is
skipped. The parent's own close policy calls `close_all`, because GPUI keeps one
`on_window_should_close` callback per window and the application owns the close
decision: asking a dirty document, waiting for a sheet and flushing state is
#37/#59, not this module. Silvestris therefore does not install a close handler
of its own.

## Gallery

The gallery's window-roles row opens one window per role from the same builder,
so the title-bar buttons and resizability are visible rather than asserted. The
gallery's own window is a `Document` frame.

## Tests

`crates/silvestris/src/window_role.rs` covers the policy rows (including
preferences keeping minimize), `role_options` resizability and minimum size,
restored vs centred vs cascaded bounds, `save_frame` keying and skipping, and an
owned dialog that opens centred on its parent and closes with
`OwnedWindows::close_all`.

`crates/silvestris/src/chrome.rs` renders each role in a test window and asserts
the drawn title bar: a document frame draws three live buttons, preferences
draws close and minimize with zoom disabled, and dialog, assistant and about
draw only close. A utility frame's band is shorter than a document's.

`goldens/renders/<appearance>/<scale>/window-frame.png` and
`window-frame-fixed.png` gate the same buttons in every appearance at 100, 125,
150 and 200%.
