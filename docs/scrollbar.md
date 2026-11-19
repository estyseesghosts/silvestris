# Scrollbar

`controls::Scrollbar` is the vertical Aqua scroller for a GPUI scroll
container. Give the container and the bar the same `ScrollHandle`:

```rust
let scroll = ScrollHandle::new();
div().flex()
    .child(div().id("content").flex_1().overflow_y_scroll().track_scroll(&scroll).children(rows))
    .child(Scrollbar::new("content-scrollbar", &scroll))
```

Behaviour (HIG pp. 225-226):

- The bar takes no width until the content is taller than the container.
- The thumb length is proportional to the visible fraction of the content.
- Dragging the thumb scrolls live.
- Clicking the track pages towards the pointer. Holding the button repeats until the thumb reaches the pointer.
- Both arrows sit at the bottom, which is the 10.6 default. A click scrolls one line, and holding the button repeats.
- In an inactive window the thumb uses `thumb_inactive`.

Geometry comes from M: `scroller.track.row`, `scroller.thumb.row`,
`scroller.arrows`, plus the Console capture for the glyph:

- The bar is 15 px wide.
- The track ends 34 px above the bottom.
- A 9 px divider sits 17 px above the bottom.
- The arrow glyphs are 7 x 6 px, centred 7.5 px across.

The thumb is painted with `paint::thumb`, which runs the bevel across the bar.

Estimates (no capture covers them): the 16 px line step, the 18 px minimum
thumb, and the 350 ms / 50 ms hold repeat. Horizontal bars are not
implemented.
