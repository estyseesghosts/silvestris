# Checkbox

`controls::Checkbox` is a controlled independent selection. Supply `checked(bool)`
or `state(CheckState::{Off, On, Mixed})` and handle `on_change`. Activation requests
On from Off/Mixed and Off from On; the owner updates its model and repaints.
The framework never persists or applies an application setting itself.

The label shares the pointer target. `show_label(false)` supports an adjacent
form label without removing the accessible name. Tab/Shift+Tab use the host's
focus routing; Space and Enter activate on release through GPUI's click path.
Modified keys do not activate. Pointer activation takes focus and stops event
propagation, so a containing row does not also activate. Disabled controls have
no click handler or focus node, and use the disabled material and label colour.

The role is CheckBox and `aria_toggled` exposes False/True/Mixed at the pinned
GPUI revision. The pin has no `aria_disabled`; native screen-reader delivery is
unverified. See [exceptions](exceptions.md).

Sources: hig-09/specs/checkbox.md (CMP-03), HIG pp. 283–286; handbook 09/15;
core/aqua.rs `checkbox_*` and reference/measurements.json `checkbox.off`,
`checkbox.on`, `checkbox.off.disabled`. Regular Aqua bounds/materials use those
measurements; the regular tick and dash silhouettes are calibrated to
`checkbox.on` and `checkbox.mixed` at 2x (left arm from 10%/52%, elbow at
40%/84%, long arm overshooting the top-right at 94%/6%). Small/mini bounds
(12/10 logical px), the 6 px label gap and their scaled glyphs are estimates.
Geist sizes (16/14/12) and Dark Aqua artwork are designed. Mixed→On, label
clicks and Enter activation are recreation choices. No theme branches exist in
the interaction or painter.

The gallery includes independently selectable regular/small/mini examples and
a disabled checked example. Mock-platform tests cover label clicks, Space/Enter,
modified keys, mixed→On, independent focus, reverse focus and disabled activation
in all four themes. These tests send KeyDown and KeyUp: GPUI's test helper
`simulate_keystrokes` alone sends only KeyDown and cannot prove release activation.
Native optical/DPI/assistive-technology acceptance remains open.
