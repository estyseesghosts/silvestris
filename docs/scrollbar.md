# Scroll views and scroll bars

`controls::ScrollView` lays out content, its scroll bars and the corner
square between them (HIG pp. 225-227). The content element does the
scrolling; give it and the view the same scroll source:

```rust
let scroll = ScrollHandle::new();
ScrollView::new("log", &scroll)
    .axes(ScrollAxes::Both)
    .line(18.0)
    .child(
        div().id("log-content").size_full().overflow_scroll().track_scroll(&scroll).children(rows),
    )
```

## Scroll sources

`ScrollSource` is the one contract every scroller reads and drives: viewport,
maximum position, position and `set_position`, in positive logical pixels.
Adapters exist for:

| Source | Content element | Notes |
| --- | --- | --- |
| `ScrollHandle` | `div().overflow_scroll().track_scroll(&handle)` | Both axes. |
| `UniformListScrollHandle` | `uniform_list(..).track_scroll(&handle)` | Delegates to the list's base handle. Both axes. |
| `ListState` | `list(state)` | Vertical only. Grabbing the thumb calls `scrollbar_drag_started`, which freezes the measured height so the thumb does not jump while rows measure; releasing calls `scrollbar_drag_ended`. Dragging to the end of a bottom-aligned list resumes following. |

`scroll_source::scroll_by`, `scroll_to`, `page` and `reveal` give views
the same line, windowful and minimal-reveal arithmetic the bars use, so a
table's Page Down and the track click agree. `reveal` scrolls only as far
as needed and does nothing when the range is visible (HIG p. 227).

## Behaviour

- Thumb length is proportional to the visible fraction (HIG p. 226).
- Dragging the thumb scrolls live.
- An arrow click moves one line; holding repeats. The line is the view's
  unit (`ScrollView::line`, default 16 px).
- A track click follows `ScrollerPreferences::track_click`: `JumpToPage`
  (default) moves one windowful, the viewport minus one line; holding pages
  until the thumb reaches the pointer. `JumpToSpot` centres the thumb on the
  pointer and continues as a drag (HIG p. 226).
- `ScrollerPreferences::arrows` puts both arrows at the far end (default) or
  one at each end. Set the preferences once per app with
  `ScrollerPreferences { .. }.set(cx)`; every bar repaints.
- `autohide(true)` (default) collapses a bar while there is nothing to
  scroll. `autohide(false)` keeps an empty track, as document windows do
  (HIG p. 226).
- The corner shows when both bars show. `leave_corner(true)` keeps it for a
  lone bar so the bar stops above the window's size control, which
  `WindowFrame` draws over the window corner (HIG p. 225).
- `size(ControlSize::Small | Mini)` is for panels whose other controls are
  that size (HIG p. 227).
- In an inactive window the thumb uses `thumb_inactive`.

`Scrollbar::new` (vertical) and `Scrollbar::horizontal` remain for custom
layouts and accept any `ScrollSource`.

## Geometry and materials

From M: `scroller.track.row`, `scroller.thumb.row`, `scroller.arrows`,
`scroller.horizontal`, `scroller.cap`, `scroller.cap.vertical`,
`scroller.boxes`, `scroller.boxes.divider`, `scroller.corner`
(Desktop & Screen Saver, Console and Finder list-view captures). The
horizontal bar is the vertical bar transposed:

- 15 px bar. Track and arrow boxes are measured 15-row cross-sections
  (`Scroller::track_rows`, `arrow_rows`).
- Each arrow box is 16 px long. Together, the track ends 34 px before the
  far end; the divider between the boxes sits 17 px from it and darkens
  the boxes by up to a quarter.
- The 12 rows before the boxes darken towards `Scroller::cap_shadow`; the
  last 6 rows of the bar darken slightly.
- 7 x 6 px arrow glyphs, centred across the bar.
- Thumb inset 1 px each side: blue leading edge, 4-row upper band, 7-row
  lower band, grey trailing edge.
- Corner square: the content background (white in Aqua).

Estimates are in [exceptions.md](exceptions.md#scroll-bars).
