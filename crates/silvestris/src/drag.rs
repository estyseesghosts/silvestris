//! Window-aware drag sessions (#36).
//!
//! GPUI owns pointer capture and delivers drag/drop events; this module owns
//! the semantics every Silvestris view shares: a payload and its source, the
//! operation indicated by the current modifiers, candidate-drop negotiation,
//! insertion feedback, cancellation and autoscroll. It never reads or mutates a
//! filesystem and never decides whether a drop means move, copy, retile or
//! detach; that stays with the consumer's provider/policy.
//!
//! HIG pp. 122-126: drag feedback begins at three pixels, Option is sampled at
//! drop time, a destination highlights only when it can accept every item, and
//! only the source window that is frontmost autoscrolls.

use std::rc::Rc;

use gpui::{px, size, Modifiers, Pixels, Point, Size};

/// Which operation a drop would perform.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DragOperation {
    /// Copy the payload (Option at drop time, or a different container).
    Copy,
    /// Move the payload (same container).
    #[default]
    Move,
    /// Create an alias or link where the consumer supports it.
    Link,
}

/// Whether the destination accepted and which operation it performed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropResult {
    pub accepted: bool,
    pub operation: DragOperation,
}

impl DropResult {
    pub fn accepted(operation: DragOperation) -> Self {
        Self {
            accepted: true,
            operation,
        }
    }

    pub fn rejected() -> Self {
        Self {
            accepted: false,
            operation: DragOperation::Move,
        }
    }
}

/// The pointer phase of a session.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DragPhase {
    /// Mouse-down, below the three-pixel threshold.
    #[default]
    Pending,
    Dragging,
    Dropped,
    Cancelled,
}

/// The transparent image the window draws under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DragImage {
    pub size: Size<Pixels>,
    /// Aqua drag images are transparent over the destination (HIG p. 123).
    pub transparent: bool,
}

impl DragImage {
    /// The standard transparent drag image.
    pub fn transparent(width: f32, height: f32) -> Self {
        Self {
            size: size(px(width.max(0.0)), px(height.max(0.0))),
            transparent: true,
        }
    }
}

/// A drop target's stable identity.
pub type DropTargetId = gpui::SharedString;

/// Where an ordered destination would insert the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Insertion {
    /// Between two rows: the index is the gap before the current row at that
    /// visible position.
    Gap(usize),
    /// On top of an item (icon views and folders).
    On(usize),
    /// A text insertion byte offset.
    Offset(usize),
}

/// Feedback the destination paints while a drag is over it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DropFeedback {
    #[default]
    None,
    Accepted(DragOperation),
    Rejected,
}

/// Autoscroll decision for one frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Autoscroll {
    pub delta: Point<f32>,
}

/// One drag, from mouse-down to drop or cancellation.
#[derive(Clone, Debug)]
pub struct DragSession<T> {
    payload: T,
    source: DropTargetId,
    start: Point<Pixels>,
    pointer: Point<Pixels>,
    modifiers: Modifiers,
    phase: DragPhase,
    default_operation: DragOperation,
    allowed: Vec<DragOperation>,
    image: DragImage,
    candidate: Option<DropTargetId>,
    insertion: Option<Insertion>,
    feedback: DropFeedback,
    result: Option<DropResult>,
    captured: bool,
}

impl<T> DragSession<T> {
    /// Begin on mouse-down. The session stays `Pending` until the pointer has
    /// moved three pixels (HIG p. 123).
    pub fn new(
        payload: T,
        source: impl Into<DropTargetId>,
        start: Point<Pixels>,
        image: DragImage,
    ) -> Self {
        Self {
            payload,
            source: source.into(),
            start,
            pointer: start,
            modifiers: Modifiers::default(),
            phase: DragPhase::Pending,
            default_operation: DragOperation::Move,
            allowed: vec![DragOperation::Copy, DragOperation::Move],
            image,
            candidate: None,
            insertion: None,
            feedback: DropFeedback::None,
            result: None,
            captured: true,
        }
    }

    /// The provider's default operation for this source/destination pair.
    pub fn default_operation(mut self, operation: DragOperation) -> Self {
        self.default_operation = operation;
        self
    }

    /// Restrict the operations this source advertises.
    pub fn allowed_operations(
        mut self,
        operations: impl IntoIterator<Item = DragOperation>,
    ) -> Self {
        self.allowed = operations.into_iter().collect();
        self
    }

    pub fn payload(&self) -> &T {
        &self.payload
    }

    pub fn source(&self) -> &DropTargetId {
        &self.source
    }

    pub fn image(&self) -> DragImage {
        self.image
    }

    pub fn phase(&self) -> DragPhase {
        self.phase
    }

    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    pub fn insertion(&self) -> Option<Insertion> {
        self.insertion
    }

    pub fn feedback(&self) -> DropFeedback {
        self.feedback
    }

    pub fn result(&self) -> Option<DropResult> {
        self.result
    }

    pub fn candidate(&self) -> Option<&DropTargetId> {
        self.candidate.as_ref()
    }

    /// Pointer capture begins on mouse-down and ends on drop/cancel.
    pub fn is_captured(&self) -> bool {
        self.captured
    }

    /// Update the pointer and current modifier state. Returns true on the frame
    /// the three-pixel threshold is crossed. Option is recorded but the
    /// operation is only decided at drop time.
    pub fn pointer_moved(&mut self, pointer: Point<Pixels>, modifiers: Modifiers) -> bool {
        self.pointer = pointer;
        self.modifiers = modifiers;
        if self.phase == DragPhase::Pending {
            let dx = (pointer.x - self.start.x).as_f32();
            let dy = (pointer.y - self.start.y).as_f32();
            if dx * dx + dy * dy >= 9.0 {
                self.phase = DragPhase::Dragging;
                return true;
            }
        }
        false
    }

    /// Set (or clear) the candidate destination and its feedback.
    pub fn set_candidate(
        &mut self,
        candidate: Option<DropTargetId>,
        feedback: DropFeedback,
        insertion: Option<Insertion>,
    ) {
        self.candidate = candidate;
        self.feedback = feedback;
        self.insertion = insertion;
    }

    /// The operation the drop would use with the current modifier state.
    ///
    /// Option at drop time means copy; it never latches and never turns a
    /// different-container copy into a move (HIG pp. 122-123).
    pub fn operation(&self) -> DragOperation {
        let copy = self.modifiers.alt;
        let requested = if copy {
            DragOperation::Copy
        } else {
            self.default_operation
        };
        if self.allowed.contains(&requested) {
            requested
        } else {
            self.allowed
                .first()
                .copied()
                .unwrap_or(self.default_operation)
        }
    }

    /// Finish the drag. A drop with no accepting candidate is rejected and
    /// zooms back (the consumer paints that from the result). Option is read
    /// here, not from drag start.
    pub fn drop(&mut self, modifiers: Modifiers) -> DropResult {
        self.modifiers = modifiers;
        let operation = self.operation();
        let accepted = self.phase == DragPhase::Dragging
            && self.candidate.is_some()
            && matches!(self.feedback, DropFeedback::Accepted(_));
        self.phase = DragPhase::Dropped;
        self.captured = false;
        let result = if accepted {
            DropResult::accepted(operation)
        } else {
            DropResult {
                accepted: false,
                operation,
            }
        };
        self.result = Some(result);
        result
    }

    /// Escape, a transient surface opening, or window teardown.
    pub fn cancel(&mut self) {
        self.phase = DragPhase::Cancelled;
        self.captured = false;
        self.candidate = None;
        self.insertion = None;
        self.feedback = DropFeedback::None;
    }
}

/// One registered destination. `accepts` evaluates the complete payload, so a
/// multi-item drag highlights only when every item is acceptable.
pub struct DropTarget<T> {
    id: DropTargetId,
    accepts: Rc<dyn Fn(&T) -> bool>,
}

impl<T> Clone for DropTarget<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            accepts: self.accepts.clone(),
        }
    }
}

impl<T> DropTarget<T> {
    pub fn new(id: impl Into<DropTargetId>, accepts: impl Fn(&T) -> bool + 'static) -> Self {
        Self {
            id: id.into(),
            accepts: Rc::new(accepts),
        }
    }

    pub fn id(&self) -> &DropTargetId {
        &self.id
    }

    pub fn accepts(&self, payload: &T) -> bool {
        (self.accepts)(payload)
    }
}

/// Destinations registered for one window.
pub struct DropTargets<T> {
    targets: Vec<DropTarget<T>>,
}

impl<T> Default for DropTargets<T> {
    fn default() -> Self {
        Self {
            targets: Vec::new(),
        }
    }
}

impl<T> DropTargets<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, target: DropTarget<T>) {
        self.targets.retain(|existing| existing.id != target.id);
        self.targets.push(target);
    }

    pub fn remove(&mut self, id: &DropTargetId) {
        self.targets.retain(|target| target.id != *id);
    }

    /// Resolve a session's candidate. The destination accepts only when one of
    /// its registered targets matches the id and its predicate accepts the
    /// complete payload.
    pub fn negotiate(&self, session: &mut DragSession<T>) -> Option<DropTargetId> {
        let candidate = session.candidate.clone()?;
        let target = self.targets.iter().find(|target| target.id == candidate);
        let accepted = target.is_some_and(|target| target.accepts(&session.payload));
        session.feedback = if accepted {
            DropFeedback::Accepted(session.operation())
        } else {
            DropFeedback::Rejected
        };
        accepted.then_some(candidate)
    }
}

/// Autoscroll is allowed only inside the source window, only when that window
/// is frontmost (HIG p. 125). `edge` is the distance from the viewport edge
/// that starts scrolling.
pub fn autoscroll(
    pointer: Point<Pixels>,
    viewport: Size<Pixels>,
    edge: f32,
    maximum: f32,
    source_window_is_frontmost: bool,
) -> Autoscroll {
    if !source_window_is_frontmost {
        return Autoscroll::default();
    }
    let edge = edge.max(0.0);
    let maximum = maximum.max(0.0);
    let x = pointer.x.as_f32();
    let y = pointer.y.as_f32();
    let width = viewport.width.as_f32();
    let height = viewport.height.as_f32();
    let ramp = |distance: f32| (1.0 - (distance / edge).clamp(0.0, 1.0)) * maximum;
    let dx = if x < edge {
        -ramp(x)
    } else if x > width - edge {
        ramp(width - x)
    } else {
        0.0
    };
    let dy = if y < edge {
        -ramp(y)
    } else if y > height - edge {
        ramp(height - y)
    } else {
        0.0
    };
    Autoscroll {
        delta: Point::new(dx, dy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{point, px, size};

    fn session() -> DragSession<Vec<&'static str>> {
        DragSession::new(
            vec!["a", "b"],
            "list",
            point(px(10.), px(10.)),
            DragImage::transparent(40., 40.),
        )
    }

    #[test]
    fn feedback_waits_for_three_pixels() {
        let mut drag = session();
        assert!(!drag.pointer_moved(point(px(12.), px(10.)), Modifiers::default()));
        assert_eq!(drag.phase(), DragPhase::Pending);
        assert!(drag.pointer_moved(point(px(13.), px(10.)), Modifiers::default()));
        assert_eq!(drag.phase(), DragPhase::Dragging);
    }

    #[test]
    fn option_is_read_at_drop_time_and_never_latches() {
        let mut drag = session().default_operation(DragOperation::Move);
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        drag.set_candidate(
            Some("folder".into()),
            DropFeedback::Accepted(DragOperation::Move),
            Some(Insertion::On(3)),
        );
        let option = Modifiers {
            alt: true,
            ..Default::default()
        };
        // Press Option mid-drag, then release before drop: still a move.
        drag.pointer_moved(point(px(20.), px(10.)), option);
        let result = drag.drop(Modifiers::default());
        assert_eq!(result, DropResult::accepted(DragOperation::Move));

        let mut drag = session().default_operation(DragOperation::Move);
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        drag.set_candidate(None, DropFeedback::None, None);
        let copy = Modifiers {
            alt: true,
            ..Default::default()
        };
        let result = drag.drop(copy);
        assert_eq!(result.operation, DragOperation::Copy);
    }

    #[test]
    fn a_disallowed_operation_falls_back_to_an_advertised_one() {
        let mut drag = session()
            .allowed_operations([DragOperation::Copy])
            .default_operation(DragOperation::Move);
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        assert_eq!(drag.operation(), DragOperation::Copy);
    }

    #[test]
    fn a_destination_accepts_only_when_it_accepts_every_item() {
        let mut targets = DropTargets::new();
        targets.register(DropTarget::new("all", |items: &Vec<&'static str>| {
            items.iter().all(|item| item.len() == 1)
        }));
        let mut drag = session();
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        drag.set_candidate(Some("all".into()), DropFeedback::None, None);
        assert_eq!(targets.negotiate(&mut drag), Some("all".into()));

        let mut drag = DragSession::new(
            vec!["long", "a"],
            "list",
            point(px(10.), px(10.)),
            DragImage::transparent(40., 40.),
        );
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        drag.set_candidate(Some("all".into()), DropFeedback::None, None);
        assert_eq!(targets.negotiate(&mut drag), None);
        assert_eq!(drag.feedback(), DropFeedback::Rejected);
        assert_eq!(drag.drop(Modifiers::default()), DropResult::rejected());
    }

    #[test]
    fn dropping_without_a_candidate_zooms_back() {
        let mut drag = session();
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        assert_eq!(drag.drop(Modifiers::default()), DropResult::rejected());
        assert!(!drag.is_captured());
    }

    #[test]
    fn autoscroll_is_limited_to_the_source_window_and_its_edges() {
        let inside = autoscroll(
            point(px(5.), px(50.)),
            size(px(100.), px(100.)),
            20.0,
            10.0,
            true,
        );
        assert!(inside.delta.x < 0.0);
        assert_eq!(inside.delta.y, 0.0);
        let inactive = autoscroll(
            point(px(5.), px(50.)),
            size(px(100.), px(100.)),
            20.0,
            10.0,
            false,
        );
        assert_eq!(inactive, Autoscroll::default());
        let centre = autoscroll(
            point(px(50.), px(50.)),
            size(px(100.), px(100.)),
            20.0,
            10.0,
            true,
        );
        assert_eq!(centre, Autoscroll::default());
    }

    #[test]
    fn cancel_releases_capture_and_clears_feedback() {
        let mut drag = session();
        drag.pointer_moved(point(px(20.), px(10.)), Modifiers::default());
        drag.set_candidate(
            Some("folder".into()),
            DropFeedback::Accepted(DragOperation::Move),
            Some(Insertion::Gap(1)),
        );
        drag.cancel();
        assert_eq!(drag.phase(), DragPhase::Cancelled);
        assert!(!drag.is_captured());
        assert_eq!(drag.feedback(), DropFeedback::None);
        assert_eq!(drag.insertion(), None);
    }
}
