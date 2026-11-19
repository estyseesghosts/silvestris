# Preferences windows

`silvestris::preferences` builds System Preferences-style windows (HIG
pp. 243-244 preferences windows, pp. 347-350 layout). Open the window with
`chrome::fixed_window_options` and render it in `WindowFrame::fixed_size()`:
no size control, zoom disabled, minimize active, matching the 10.6 System
Preferences capture in hig-09/images.

- `PreferencesNavigator`: current pane plus back/forward history. Show All is
  an entry like a pane. `title()` gives the window title, the selected pane's
  name per HIG p. 244. `retain()` drops panes that disappear.
- `PreferencesToolbar`: momentary back/forward segments, Show All, and a
  right-hand slot (System Preferences puts Spotlight search there). It is a
  fixed row: preferences toolbars are not customizable (HIG p. 244).
- `PreferencesOverview`: category bands of 32 px icon cells. Consumers supply
  their own artwork; no Apple icons are included.
- `TabView`: group box with a centred tab control straddling the top edge.
  `on_overflow` reports how much taller or shorter the content is than the box,
  so the window can resize to the pane as 10.6 does (HIG p. 244). Content that
  still does not fit scrolls, with a `Scrollbar` inside the box's right edge.
- `Form`/`FormRow`: right-aligned `Label:` column, controls in one column,
  label-less rows (checkboxes) aligned to the control column, optional note.
- `fit_content()` reports the laid-out height of the overview so the window can resize to it exactly; label wrapping makes a precomputed height unreliable.

Measured (reference/measurements.json): `prefs.overview.geometry`
(81 px cells, 32 px icons 32 px below the band top, labels from 68 px),
`prefs.overview.bands` (#ededed/#e6e6e6 alternation, #d8d8d8 rule; bands
are 99-103 px, 100 is used), `prefs.pane.geometry` (20 px side/bottom
margins, box top 34 px under the toolbar, 22 px centred tabs),
`groupbox.top`/`groupbox.left` (#ababab top edge, #c8c8c8 sides).
Estimates: 5 px grid inset, heading and label line offsets, box corner
radius, content padding, Aqua Dark and Geist band/edge colours.

Related controls: `controls::PopUpButton` (HIG pp. 279-281; current item
over the button, keyboard open/choose/dismiss; enabled cap colour is an
estimate since only the disabled pop-up is measured) and `controls::Stepper`
(HIG p. 301; sizes are estimates). `TextFieldEvent::EndedEditing` lets a
form commit a field's value when focus leaves it as well as on Return.

Not implemented: per-pane window height animation, search with spotlight
highlighting of matching icons, the round help button, and a captured
reference for the enabled pop-up, stepper and dark-mode panes.
