# Measured application shell

Shell connects core::layout to GPUI for #130-#132. It supplies a persistent
sidebar, workspace, optional inspector rail and one inspector implementation
that can be closed, overlaid or docked. Consumers own both preference structs
and domain state, and supply all source/rail/workspace/inspector content.

The pinned GPUI container_query receives the component's allocated size
before laying out children. Shell feeds that width to the pure planner,
retaining the previous plan in keyed state for breakpoint hysteresis.
It does not guess from window size or schedule a later measurement repaint.

Shell::measured accepts a one-shot content builder returning ShellContent.
The builder receives the effective ShellPlan and allocated logical size after
planning, before child layout. Fit previews using plan.workspace and inspector
content using plan.dock.or(plan.overlay); do not compute a second viewport plan.
Its rail_enabled argument participates in planning before content is built.
Shell::new remains available for content that does not need measured geometry.

Sidebar is rendered with the effective width/presentation and original saved
preference. Inspector overlay and dock share one absolute-positioned subtree:
overlay uses plan.overlay and never changes plan.workspace; dock uses plan.dock
and reserves horizontal width. The inspector rail depends on available width,
not on the consumer's workspace tile count.

Header Pin/Unpin and Close call on_inspector_change. Escape dismisses only an
overlay. A transparent shield consumes outside clicks while the overlay is
open; the rail remains above the shield so its actions stay usable.
Inspector children handle their own events first; remaining pointer/keyboard
events are consumed before reaching a workspace responder. Applications
still own guest-HID release, session/epoch validation and focus-scope policy.

The leading divider uses GPUI native drag routing, with keyed drag origin
surviving focus repaints. Left/Right keyboard input increases/decreases width
by a designed 10 logical px; Home/End choose min/max. Dragging under half the
minimum closes the inspector while retaining its pin preference and width.
Widths and hysteresis come from ShellGeometry (default: handbook 13's
Lotus-legacy geometry). Border/hit areas reuse theme divider/splitter tokens.
The 8 px header padding and 6 px header gap are estimates pending visual review.

Opening moves logical focus into the inspector. Closing restores the saved
responder if focus was still inside the inspector; otherwise the newer focus
is preserved. Applications must invalidate stale return targets if the
underlying view/session disappears. Pin/unpin preserves the same entity
content and focus state; transient popup menus should be mounted above Shell.
Clicking inspector content while focus is outside it reclaims inspector focus;
an already-focused child editor retains its responder.

Tests: measured parent width with padding, overlay/workspace identity,
pinned width reservation, Escape/focus return, narrow pinned demotion and
redocking, outside-click shielding, leading-edge drag and keyboard resizing.
Geometry tests run all four themes and simulated 125/150/200% scales using
GPUI's mock platform. Native DPI movement, screen reader behavior and pixel
comparisons are unverified.

The independent explorer binary composes a document source list, preview,
commands, persistent rail and inspector using public APIs only. It uses
in-memory sample documents and preferences. It is a buildable integration
example, not a file manager or persisted-settings implementation.

Remaining: native visual/input review, resize cancellation on deactivation,
split-worker target adapters and guest touch/HID tests, final icon artwork,
focus-return invalidation on target deletion, inspector header/status
refinement and app-owned preference migration/persistence. No Lotus worker
or MCP implementation is changed by Shell.

Primary references: aqua-gpui handbook 13; pinned GPUI container_query.rs
(measured child layout), div.rs (drag routing), window.rs (logical focus);
core/layout.rs and its source citations describe constraints and hysteresis.

Native first-frame check, 2026-11-19: the Windows GNU explorer executable
started with bundled fonts and rendered its closed-inspector Aqua Light shell
at 100% DPI (visible frame 1202x721). The local inspection capture is
target/explorer-first-frame.png and is not a committed golden baseline.
Startup stderr was empty. Native inspector interactions remain unverified.
