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

## Remaining issues

- Native Windows 11 corner captures over light and dark desktops at all five
  scales were not possible in this headless environment. DWM corner behaviour
  needs interactive verification.
- `DWMWCP_ROUNDSMALL` acceptance is recorded as a capability flag, not as
  proof of an exact compositor radius.
- Lotus, Pain and Chaus smoke tests require those repositories and builds.
- Final golden acceptance is deliberately deferred to the user.
