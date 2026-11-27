# Geist refinement final report

This report covers `docs/geist.txt` against commits `d575f34..HEAD` on branch
`devel/sil-cocoa-fixture`.

## Completed tasks

1. Baseline commit, GPUI pin, four-theme configuration and allowable change
   surface recorded in `docs/geist-refinement.md`.
2. Aqua goldens and pre-refinement Geist captures preserved under
   `docs/geist-before`; pre-existing host drift recorded before any change.
3. Aqua token fixtures, theme isolation, font override precedence and live
   switch sequence covered by `crates/silvestris-core/tests/aqua_isolation.rs`
   and `crates/silvestris/tests/theme_switch.rs`.
4. Geist Sans v1.9.2 static faces vendored, registered and checked; Inter kept
   as an auxiliary family.
5. Geist typography roles set; label measurements at five scales recorded in
   `docs/geist-typography/measurements.json`.
6. One private `GeistPalette` resolves both appearances through the existing
   public token structs.
7–9. Geist surfaces, Light parity and control-state colours follow the palette;
   hover, selection and text contrast have tests.
10–16. Theme-resolved corner rules, flat paint paths, button/popup, segment,
   field, small-control and preferences refinements use shared implementations.
17–19. Flat Geist scrollbar with shared pointer model; preferences opt-in
   scroll handles.
20–21. Ten pinned Lucide pictograms with prepared geometry and theme-resolved
   ink; gallery shows them in a 32 px slot grid.
23. Linux client decoration and Windows small-corner requests unchanged for
   Aqua and matched to the Geist root radius.
24. Candidate Geist goldens rendered for both appearances at 100, 125, 150,
   175 and 200%. An appended Geist-only page adds search field, button state,
   horizontal scroll, long-text wrapping and pictogram samples without moving
   any accepted Aqua golden. Final acceptance is pending user review.
25. Interaction tests for theme switching with focus, text, scroll state and an
   edited toolbar arrangement; contrast, font, scrollbar and hover tests pass.
26. Consumer integration requirements documented; no consumer repository was
   modified because none is present here.
27. This report, the refinement document and the changelog entry.

## Checks run

- `cargo test --workspace --locked`
- `cargo check --workspace --all-targets --locked`
- `cargo test -p silvestris-core --locked`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `pwsh tools/metrology/render.ps1 -Appearance geist-light -Out target/goldens-candidate`
- `pwsh tools/metrology/render.ps1 -Appearance geist-dark -Out target/goldens-candidate`
- `python tools/metrology/gate.py --candidate target/goldens-candidate`
- `python tools/fonts/manifest.py --check`
- `python tools/icons/geist_pictograms.py --check`

## Visual changes and acceptance

No Aqua golden was regenerated or accepted. The pre-existing 90 host-drift
failures in the baseline gate (six controls across both appearances and five
scales) are unchanged. Candidate Geist images reflect the palette, typography,
radii, flat scrollbar and pictogram changes; they are not accepted until the
user inspects them. The gate reports Aqua differences separately as forbidden
and Geist differences as awaiting review.

## Acceptance checklist (docs/geist.txt, 15 items)

This records the checklist in the specification without editing the supplied
specification file. "Verified" means evidence in this repository; "awaiting
review" means the evidence exists but the user has not approved it.

| # | Item | Status and evidence |
| - | ---- | ------------------- |
| 1 | Aqua Light accepted output unchanged | Verified by `aqua_isolation` fixtures and the candidate gate; no Aqua golden was regenerated or accepted. |
| 2 | Aqua Dark accepted output unchanged | Same as item 1. |
| 3 | Geist Sans used, Geist Mono retained | Verified by `tools/fonts/manifest.py --check`, `geist_fonts` tests and the resolved `Typography` tokens. |
| 4 | Geist Dark genuinely near-black | Verified by palette values (`#000000` content, `#0A0A0A` window) and the candidate window captures; awaiting user review. |
| 5 | Geist Light uses the same semantic roles and geometry | Verified by the single `GeistPalette` with dark/light constructors and shared component recipes. |
| 6 | Exact control radii, spacing and materials | Verified by palette/geometry tests and the corner-rule tests; awaiting user review of candidate images. |
| 7 | Menus, toolbars, sidebars, lists, tables, preferences consistent | Verified by shared token resolution and interaction tests; candidate screenshots await review. |
| 8 | Flat Geist scrollbars with all interactions preserved | Verified by `geist_scrollbar` tests (pointer, drag, page, jump, themes) and shared `ScrollSource` geometry. |
| 9 | Consistent pictograms, no Aqua artwork leakage | Verified by `geist_icons` tests, the pinned Lucide generator `--check`, and candidate pictogram captures. |
| 10 | Windows and Linux corners correct | Blocked: headless capture has no DWM compositor. Code paths and capability reporting are documented; interactive Windows 11 verification remains. |
| 11 | All five scale factors inspected | Candidate renders exist at 100/125/150/175/200% for all four themes; user inspection is pending. |
| 12 | Theme switching preserves focus, selection, scroll and state | Verified by five `theme_switch` tests covering focus, field contents, scroll position and toolbar arrangement. |
| 13 | Functional, font and visual gates pass | Functional and font gates pass; visual gate reports only the documented pre-existing host drift and new Geist differences awaiting review. |
| 14 | Consumer smoke tests show no toolkit regressions | Blocked: Lotus, Pain and Chaus are separate repositories not present here. Required consumer-side changes are documented. |
| 15 | User approves screenshots before golden acceptance | Blocked by design: candidate Geist images are rendered but deliberately not accepted. |

## Remaining issues

- Native Windows 11 corner captures over light and dark desktops at all five
  scales were not possible in this headless environment. DWM corner behaviour
  needs interactive verification.
- `DWMWCP_ROUNDSMALL` acceptance is recorded as a capability flag, not as
  proof of an exact compositor radius.
- Lotus, Pain and Chaus smoke tests require those repositories and builds.
- Final golden acceptance is deliberately deferred to the user.
