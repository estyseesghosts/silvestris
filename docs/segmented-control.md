# Single-choice segmented control

SegmentedControl implements the text-only, regular-size view-choice portion
of #128. HIG pp. 286-288 (hig-09/03-aqua-interface/controls.md) requires equal
segment widths, short noun labels, uniform text or icons, and use for related
choices or view changes. This widget accepts text only. It does not claim
tab-view attachment and must not implement source-list add/remove actions.

Each Segment has a stable ID, label and disabled flag. The consumer supplies
selected(id) and handles on_select; no app state or persistence lives in the
control. IDs must be unique within the group. Selecting the current ID does
not emit again. Empty and all-disabled groups do not take focus or dispatch.

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

The gallery exercises Grid/List with Columns disabled. GPUI tests in
segmented.rs cover equal geometry and contiguous joins, click selection,
disabled click suppression, arrow skipping/wrapping, Home/End, modified-key
pass-through and suppression of duplicate callbacks in all four themes.
Separate cases cover absent/invalid selection and empty/all-disabled lists.
The mock text system verifies geometry and event routing, not font accuracy.

Still open for the complete #128 family: multiple-selection and momentary
modes, icons, small/mini sizes, toolbar-specific variants, native keyboard
delivery, screen-reader verification and visual snapshots at multiple DPI.
