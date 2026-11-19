# Command popup menus

PopupMenu is the reusable menu surface for #129. It consumes core MenuModel
entries and an Entity<CommandRegistry>; it contains no application callbacks
or emulator types. The gallery View popup uses the same view.sidebar command
as the toolbar, footer and accelerator, plus an Appearance submenu.

Construct the entity with the model, registry, window and context. Mount it
once as a deferred overlay at priority 1 above normal content. open(position)
takes focus and records the previous responder. Escape/Tab, outside click or
activation dismisses and restores that responder. Window deactivation closes
the menu and restores logical focus without activating the OS window.
Consumers must invalidate the return target themselves if their view/session
is removed while a menu is open.
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

Materials come from the active theme's menu/selection tokens and metrics.
Aqua Light's tokens cite measurements.json menu.body and menu.selection;
the checkmark/submenu glyphs are font glyphs, not copied Apple artwork.
The 160 px minimum width, 20 px check gutter, 16 px submenu gutter, 8 px gap,
4 px window margin, shadow_lg and immediate submenu opening are estimates or
modern extensions pending visual/native review.

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
