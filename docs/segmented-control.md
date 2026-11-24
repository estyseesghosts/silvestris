# Segmented control

SegmentedControl implements the text, icon, size and mode family from #128 and
#11. HIG pp. 286-288 (hig-09/03-aqua-interface/controls.md) requires equal
segment widths, short noun labels, uniform text or icons, and use for related
choices or view changes. It does not claim tab-view attachment and must not
implement source-list add/remove actions.

Each Segment has a stable ID, label, optional icon role, show_label and disabled
flag. The consumer supplies selected(id) for SelectOne, selected_many(..) for
SelectAny, and handles on_select; no app state or persistence lives in the
control. IDs must be unique within the group. SelectOne does not emit for the
current ID; SelectAny and Momentary dispatch every click, and Momentary never
latches. Empty and all-disabled groups do not take focus or dispatch.

`SegmentMode` is SelectOne, SelectAny or Momentary. `SegmentStyle` is Rounded
(capsule, window body) or Textured (toolbar). `ControlSize` selects regular,
small or mini height and type. Icon-only segments are square, with icons
resolved by `icon_renderer(role, ink)` so no font glyph is baked into geometry.

The longest shaped label plus regular push-padding determines every segment's
width; min_segment_width can make them wider. Theme tokens provide type,
height, normal/selected/pressed/disabled bevel and focus. Aqua Light tokens
cite measurements segmented.normal/selected from the 10.6 Desktop & Screen
Saver capture. The capsule radius (half the height), reuse of push padding
and border-colour seam are estimates pending pixel review.

The painter reconstructs the whole group bounds within each segment's clip.
This keeps outer endcaps rounded and interior joins square with one device
pixel seams. Pressed material and label colour are scoped to each segment.

Tab reaches the group once. Arrow keys select the next enabled choice,
wrapping at the ends; Home/End select first/last enabled. Enter/Space selects
the first enabled choice when no valid selection exists. Modified keys are
left to the application. Mouse selection focuses the same group handle.
The disabled group removes its focus registration and click handlers.

Accessibility uses Group and Button roles. The pinned GPUI lacks aria_pressed
and aria_checked, so the selected button's accessible name includes
", selected"; this is a fallback, not a verified screen-reader result.

The gallery exercises text, icon-only, textured, small and both multi/momentary
modes. GPUI tests in segmented.rs cover equal geometry and contiguous joins,
click selection, disabled click suppression, arrow skipping/wrapping, Home/End,
modified-key pass-through, suppression of duplicate SelectOne callbacks and
every-click dispatch for SelectAny/Momentary in all four themes. Separate cases
cover absent/invalid selection and empty/all-disabled lists. The mock text
system verifies geometry and event routing, not font accuracy.

Still open: native keyboard delivery, screen-reader verification and visual
snapshots at multiple DPI for every mode; the committed `segmented*` goldens
cover the regular, textured, icon and small shapes at all five scales.
