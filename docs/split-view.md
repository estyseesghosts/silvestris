# Split view

`split::{SplitView}` is a reusable two-or-more-pane splitter (#53). It is not a
tiling tree, docking system, tab manager or IDE layout manager, and it does not
replace [`crate::shell::Shell`]. Put a `SplitView` inside Shell's workspace when
a domain view needs its own panes.

## Geometry (HIG pp. 336-338)

- A **horizontal** splitter stacks panes; a **vertical** splitter places them
  side by side. One `SplitView` never mixes the two.
- **Thin** is the 10.5+ standard: a 1 logical px seam with a 2 px hot zone on
  each side. The hot zone overlays the neighbouring panes and does not consume
  layout space.
- **Thick** is 9 logical px with a 4 px circular dimple at its midpoint. The
  dimple size is measured from the 10.6 Automator capture.
- A pane may declare a minimum and maximum. The layout reconciles both without
  ever growing the container past its measured extent.
- A collapsed pane gets zero extent, but the consumer, not `SplitView`, owns
  that decision. A thick divider remains visible when a pane is collapsed; a
  thin divider disappears, so a consumer that hides a pane with a thin divider
  must provide a restore path (HIG p. 337).

`split::layout` is the pure one-dimensional rule. `SplitView` measures its own
allocation with `container_query` before building children, so pane spans and
divider positions are exact on the first frame. `on_bounds` reports the measured
pane rectangles for domain views that derive geometry from their allocation.

## Controlled API

```rust
use silvestris::core::split::{
    SplitDividerStyle, SplitOrientation, SplitPane, SplitState,
};
use silvestris::split::SplitView;

let view = SplitView::new("workspace-split", SplitOrientation::Vertical)
    .divider_style(SplitDividerStyle::Thin)
    .state(saved_state.clone())
    .pane("sidebar", SplitPane::new(180.0, 320.0), sidebar)
    .pane("content", SplitPane::new(320.0, f32::INFINITY), content)
    .pane("inspector", SplitPane::new(200.0, 420.0), inspector)
    .on_resize(cx.listener(|this, state: &SplitState, _, cx| {
        this.saved_state = state.clone();
        // Persist saved_state with the rest of the application's settings.
        cx.notify();
    }));
```

`SplitView` is controlled: it never mutates the supplied state. Resizing and
keyboard adjustment produce a new `SplitState` through `on_resize`; the consumer
persists it and passes it back. Persist only divider IDs and positions. Do not
serialize pane contents or application state with it.

A divider is focusable with Tab. Left/Right (vertical) or Up/Down (horizontal)
move it by 10 logical px; Home and End move it to the adjacent minimum or
maximum. The whole hot zone changes the pointer to the appropriate resize
cursor.

## What stays with the consumer

`SplitView` does not decide whether a drop means move or copy, which pane
collapses, whether a pane is a terminal, emulator, debugger or inspector, or
which provider location a pane shows. Lotus keeps tile/session policy and
emulator screens; Pain keeps PTY ownership, terminal resize calculations and
Ghostty state; Sandcat continues to use Shell for sidebar/workspace/inspector.
