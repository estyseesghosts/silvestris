# Command popup menus

PopupMenu is the reusable menu surface for #129. It consumes core MenuModel
entries and an Entity<CommandRegistry>; it contains no application callbacks
or emulator types. The gallery View popup uses the same view.sidebar command
as the toolbar, footer and accelerator, plus an Appearance submenu.

Construct the entity with the model, registry, window and context. Mount it
once as a deferred overlay at priority 1 above normal content. open(position)
takes focus. Outside click, scroll, Escape, Tab and window deactivation dismiss
through the window's `TransientLayer` (#60), which restores the responder that
had focus before the first surface opened. An open submenu closes on the first
Escape and the menu on the second. Consumers must invalidate the return target
themselves if their view or session is removed while a menu is open.
`set_layer` joins a shared layer; without it the menu owns a private one.
set_model replaces the command tree; if the model differs while open, the
menu dismisses first so a highlighted index never moves to another command.

- Command rows read live labels, checked state, availability and accelerators.
  Missing commands are disabled. Activation revalidates availability and then
  goes through CommandRegistry::invoke outside the popup entity borrow.
- Headings and separators are noninteractive. Checked toggle/radio commands
  use the HIG checkmark. Disabled commands retain visible rows.
- Hover opens submenus immediately. Right enters and Left leaves a submenu;
  Up/Down cycle through enabled rows, Home/End select boundaries, Enter/Space
  activates, and a letter selects the next matching initial.
- Mouse release activates a command; holding the pointer over rows can scan
  without invoking. The full-window shield consumes background pointer,
  scrolling and keyboard events, including dismissal clicks. Registered
  InvokeCommand shortcuts are intercepted, dismiss and use the same registry.
- Panels measure label and shortcut text for width, clamp to the viewport,
  and use GPUI anchored placement. Nested deferred panels attach to the
  parent row and use GPUI's opposite-side placement when needed.
- Long menus scroll; keyboard selection calls ScrollHandle::scroll_to_item.
  The text width can truncate at narrow viewport sizes. Popup size is bounded
  by the current window viewport with a designed 4 logical px margin.

Materials and geometry come from the active theme's menu tokens. Aqua Light
cites the contextual captures (M: menu.context.*, #16):

- `open` draws a contextual menu with four rounded corners;
  `open_pop_down` hangs the menu from a control with square top corners, as
  under the Finder toolbar's Action button. A submenu's top corner on the
  parent's side is square.
- The frame is a 1 px edge around a translucent body. The edge and the drop
  shadow are painted outside the body only, because 10.6 casts no shadow
  under the menu.
- Rows are 18 px, separators 12 px with the line on the 6th row and 1 px
  side insets, and the label sits 21 px from each body edge. Menus are as
  wide as their widest row; there is no minimum.
- The submenu arrow is a drawn 7.5 x 10 px triangle 19 px in from the body
  edge. A submenu's edge overlaps the parent's by 2 px and its first row
  lines up with the parent row; it flips left by GPUI's rule.
- Key equivalents are split by `split_shortcut`: the key is left-aligned in a
  shared column and the modifiers end 4 px before it (HIG p. 167).
- Checked items show a checkmark and mixed items a dash (HIG pp. 170-171).
  GPUI's native menu items cannot show the dash.

`highlight_path` highlights rows and opens submenus as pointer scanning
would, without invoking. The capture tool uses it for the `menu-arrange-by`
and `menu-marks` goldens. The 4 px window margin and immediate submenu
opening are designed. Remaining estimates are listed in
[exceptions](exceptions.md#menus).

GPUI interaction tests cover pointer activation and live disabled-state
rejection in all four themes, outside-click shielding, keyboard submenu
dispatch and focus restoration, viewport bounds, End-key scrolling of 100
commands and deactivation dismissal. The mock text/platform verifies routing
and geometry rather than native fonts, OS event delivery or pixel accuracy.

Remaining #129 work: timed multi-character search, submenu hover delay and
pointer corridor, scroll arrows/edge autoscroll, activation blink, native
macOS app menus, Windows/Linux menubar and title-drag interaction, real
screen-reader behavior, focus target invalidation and screenshot review.
The pinned GPUI lacks aria_disabled/checked; checked state is included in the
row's accessible name as a fallback. No native menu integration is claimed.

References: HIG 2009 pp. 165-173 (hig-09/03-aqua-interface/menus.md);
aqua-gpui handbook 12; pinned GPUI anchored.rs (attach and local positioning),
examples/popover.rs (nested deferred layers), and div.rs (scroll tracking).
