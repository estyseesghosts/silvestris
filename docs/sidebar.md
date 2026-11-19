# Persistent sidebar container

Sidebar implements #130's reusable pane/divider portion using handbook 13
and core::layout::SidebarPreference. It is a fixed-width flex sibling of
the workspace, including when collapsed. It never becomes an overlay.

The consumer supplies expanded content (typically a SourceList entity),
meaningful rail content and optional footer. The consumer owns the saved
preference and updates it from on_change. The gallery shows Docs/Archive
rail actions plus a Hide/Show footer; these use the same SourceList entity,
so selection survives collapse.

Geometry defaults to ShellGeometry::LOTUS_LEGACY (52 px rail, 200..440 px
expanded range), values explicitly retained by handbook 13. geometry()
allows another consumer to supply its policy. effective(width, expanded)
accepts ShellPlan's temporary presentation without overwriting saved width
or expanded state. The final application shell must feed that plan; the
gallery currently demonstrates user resize/collapse only.

The divider has a one-logical-pixel visible seam. Its hit target is the
greater of the theme's splitter_hot_zone and a designed 4 logical px.
This minimum is an accessibility estimate pending visual/native review.

- Mouse: horizontal resize cursor; native GPUI on_drag/on_drag_move routes
  movement beyond the divider. No drag preview is drawn. Collapse below
  half the minimum preserves the width at drag start; expanding restores it.
- Keyboard: Tab reaches the labelled Splitter. Left/Right adjust by a designed
  10 logical px, Home/End choose min/max, Enter/Space toggle expansion.
  Toolbar/menu commands can update the same consumer-owned preference.
- Focus: keyboard-only ring; keyed focus and pointer-origin state survive
  the redraw caused by focusing the divider.
- Theme: sidebar colours/divider/focus tokens; shared interaction logic.
- Persistence: no filesystem access. Persist SidebarPreference through the
  application's existing preference path; no writes during temporary demotion.

GPUI interaction tests cover drag outside the hitbox, stopping on release,
snap-collapse with remembered width, keyboard collapse/reopen, reserved
workspace geometry and effective demotion across all four themes. The
drag test caught a pointer-origin reset across the focus repaint and fails
without the keyed drag state.

These tests use GPUI's mock platform/text system. Native OS capture, focus
loss/cancel during drag, screen reader value reporting, four-theme visual
review and mixed DPI remain unverified. This does not complete #130:
Lotus adaptation, persisted integration, footer command/menu wiring, final
icon artwork and classic scrollbars remain open.

Primary API references at pinned GPUI 6b720973:
elements/div.rs documents on_drag_move outside element bounds; app/test_context.rs
provides mouse event simulation. The size/snap policy is in core layout.rs,
with source citations and pure regression tests.
