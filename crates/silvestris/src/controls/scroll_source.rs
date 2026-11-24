//! Scroll sources (#40): one contract for every GPUI scroll state, so one
//! scroller painter serves divs, uniform lists and variable-height lists.
//!
//! Positions are positive distances scrolled from the top-left, in logical
//! pixels. GPUI's own handles store the negated offset; the adapters convert.
use gpui::{
    point, px, Axis, ListState, Pixels, Point, ScrollHandle, Size, UniformListScrollHandle,
};

/// Anything a scroller can read and drive.
///
/// Implementations must be cheap to clone handles (GPUI's are `Rc`s): the
/// scroller keeps one for its paint and repeat closures.
pub trait ScrollSource: 'static {
    /// Size of the visible area.
    fn viewport(&self) -> Size<Pixels>;
    /// Furthest position on each axis; zero on an axis that cannot scroll.
    fn max_position(&self) -> Point<Pixels>;
    /// Current position, between zero and [`max_position`](Self::max_position).
    fn position(&self) -> Point<Pixels>;
    /// Move to `position`. Callers clamp; implementations may clamp again.
    fn set_position(&self, position: Point<Pixels>);
    /// The thumb was grabbed. Sources whose content size is an estimate
    /// freeze it here so the thumb does not jump while rows measure.
    fn drag_started(&self) {}
    fn drag_ended(&self) {}

    /// Viewport plus the scrollable range.
    fn content_size(&self) -> Size<Pixels> {
        let viewport = self.viewport();
        let max = self.max_position();
        Size {
            width: viewport.width + max.x,
            height: viewport.height + max.y,
        }
    }
}

impl ScrollSource for ScrollHandle {
    fn viewport(&self) -> Size<Pixels> {
        self.bounds().size
    }
    fn max_position(&self) -> Point<Pixels> {
        let max = self.max_offset();
        point(max.x.max(px(0.)), max.y.max(px(0.)))
    }
    fn position(&self) -> Point<Pixels> {
        let offset = self.offset();
        point(-offset.x, -offset.y)
    }
    fn set_position(&self, position: Point<Pixels>) {
        self.set_offset(point(-position.x, -position.y));
    }
}

/// `uniform_list` keeps its offset and measured extent in a base
/// `ScrollHandle`, so this adapter delegates to it.
impl ScrollSource for UniformListScrollHandle {
    fn viewport(&self) -> Size<Pixels> {
        self.0.borrow().base_handle.viewport()
    }
    fn max_position(&self) -> Point<Pixels> {
        self.0.borrow().base_handle.max_position()
    }
    fn position(&self) -> Point<Pixels> {
        self.0.borrow().base_handle.position()
    }
    fn set_position(&self, position: Point<Pixels>) {
        let base = self.0.borrow().base_handle.clone();
        base.set_position(position);
    }
}

/// Variable-height `list`. Vertical only. The content height is the sum of
/// measured rows; [`drag_started`](ScrollSource::drag_started) freezes it
/// through GPUI's scrollbar contract, and dragging to the end resumes
/// bottom-following for a `ListAlignment::Bottom` list.
impl ScrollSource for ListState {
    fn viewport(&self) -> Size<Pixels> {
        self.viewport_bounds().size
    }
    fn max_position(&self) -> Point<Pixels> {
        point(px(0.), self.max_offset_for_scrollbar().y.max(px(0.)))
    }
    fn position(&self) -> Point<Pixels> {
        point(px(0.), -self.scroll_px_offset_for_scrollbar().y)
    }
    fn set_position(&self, position: Point<Pixels>) {
        self.set_offset_from_scrollbar(point(px(0.), -position.y));
    }
    fn drag_started(&self) {
        self.scrollbar_drag_started();
    }
    fn drag_ended(&self) {
        self.scrollbar_drag_ended();
    }
}

/// The component of `p` along `axis`.
pub fn along(p: Point<Pixels>, axis: Axis) -> Pixels {
    match axis {
        Axis::Vertical => p.y,
        Axis::Horizontal => p.x,
    }
}

/// The extent of `s` along `axis`.
pub fn extent(s: Size<Pixels>, axis: Axis) -> Pixels {
    match axis {
        Axis::Vertical => s.height,
        Axis::Horizontal => s.width,
    }
}

/// Move `source` by `delta` along `axis`, clamped to its range. Returns
/// whether the position changed.
pub fn scroll_by(source: &dyn ScrollSource, axis: Axis, delta: Pixels) -> bool {
    let current = source.position();
    let max = along(source.max_position(), axis);
    let target = (along(current, axis) + delta).clamp(px(0.), max.max(px(0.)));
    scroll_to(source, axis, target)
}

/// Move `source` to `position` along `axis`, clamped. Returns whether the
/// position changed.
pub fn scroll_to(source: &dyn ScrollSource, axis: Axis, position: Pixels) -> bool {
    let current = source.position();
    let max = along(source.max_position(), axis).max(px(0.));
    let target = position.clamp(px(0.), max);
    if target == along(current, axis) {
        return false;
    }
    let next = match axis {
        Axis::Vertical => point(current.x, target),
        Axis::Horizontal => point(target, current.y),
    };
    source.set_position(next);
    true
}

/// One "windowful" along `axis`: the viewport minus one `line` of overlap
/// to keep the reader's context (HIG p. 226), never less than one line.
pub fn page(source: &dyn ScrollSource, axis: Axis, line: Pixels) -> Pixels {
    (extent(source.viewport(), axis) - line).max(line)
}

/// Scroll the least distance that shows `start..end` (content coordinates
/// along `axis`). If the range is larger than the viewport, its start is
/// shown. Does nothing when it is already fully visible (HIG p. 227: "move
/// the document only as much as is necessary").
pub fn reveal(source: &dyn ScrollSource, axis: Axis, start: Pixels, end: Pixels) -> bool {
    let position = along(source.position(), axis);
    let viewport = extent(source.viewport(), axis);
    if start >= position && end <= position + viewport {
        return false;
    }
    if start < position || end - start > viewport {
        scroll_to(source, axis, start)
    } else {
        scroll_to(source, axis, end - viewport)
    }
}
