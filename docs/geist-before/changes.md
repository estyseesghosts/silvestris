# Why the candidate Geist goldens differ

The images in this directory are the accepted pre-refinement Geist references.
The candidate renders in `target/goldens-candidate` differ for these documented
reasons. No candidate has been accepted; the gate must still be reviewed.

| Change | Authorized by | Affected samples |
| --- | --- | --- |
| Geist Sans replaces Inter; small/mini/label/heading sizes rise | Task 4, Task 5 | every text-bearing sample |
| Palette, black document surface, neutral popup cap and selection roles | Tasks 6–9 | buttons, popups, fields, menus, tables, source lists, preferences, windows |
| 4 px push/segment/field and 6 px search/menu/panel radii | Tasks 10, 12, 13, 16 | buttons, segments, fields, menus, group boxes, windows |
| Flat bevel and group-box paint paths | Task 11 | buttons, group boxes, tab views |
| 3 px checkbox radius, flat progress and small-control materials | Task 14 | checkboxes, progress, sliders |
| Geist preference typography and line boxes | Task 15 | preferences pages |
| Flat scrollbar with no arrows | Tasks 17–19 | scroll views, table/source-list scrolling where mounted |
| Ten standard pictograms | Tasks 20–21 | gallery artwork grid when captured |

Aqua Light and Aqua Dark references are not regenerated. Their only token
changes are the additive fields from Task 10, initialised with the existing
Aqua materials and corner policies.
