//! Split-view layout and divider interaction (#53).
//!
//! This is a one-dimensional splitter, not a tiling or docking system. It
//! answers three questions: how much room each pane gets, where the dividers
//! sit, and how a drag changes those positions. Consumers own pane contents,
//! collapse policy and application state; only divider IDs and positions are
//! meant to be persisted.
//!
//! HIG pp. 336-338: two or more views with a splitter between each pair; the
//! zero-width splitter is 1 px wide and its hot zone extends 2 px on both
//! sides; the thick splitter is 9 px with a circular dimple. A single split
//! view never mixes orientations.

use serde::{Deserialize, Serialize};

/// A horizontal splitter stacks views; a vertical splitter places them side by
/// side (HIG p. 336).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitOrientation {
    /// A horizontal divider, panes stacked top to bottom.
    Horizontal,
    /// A vertical divider, panes side by side.
    #[default]
    Vertical,
}

impl SplitOrientation {
    /// True when panes are laid out left to right.
    pub fn is_side_by_side(self) -> bool {
        self == Self::Vertical
    }
}

/// The approved Aqua divider styles (HIG p. 336).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitDividerStyle {
    /// 1 px, the 10.5+ standard.
    #[default]
    Thin,
    /// 9 px, less common, with a circular dimple.
    Thick,
}

impl SplitDividerStyle {
    pub const THIN_WIDTH: f32 = 1.0;
    pub const THICK_WIDTH: f32 = 9.0;
    /// A zero-width splitter includes 2 px on both sides in its hot zone.
    pub const THIN_HOT_ZONE: f32 = 2.0;
    /// Dimple diameter measured from the 10.6 Automator thick splitter
    /// (4 px across, centred in the 9 px bar).
    pub const THICK_DIMPLE: f32 = 4.0;

    pub fn width(self) -> f32 {
        match self {
            Self::Thin => Self::THIN_WIDTH,
            Self::Thick => Self::THICK_WIDTH,
        }
    }

    /// Extra hit area on each side.
    pub fn hot_zone(self) -> f32 {
        match self {
            Self::Thin => Self::THIN_HOT_ZONE,
            Self::Thick => 0.0,
        }
    }
}

/// A pane's sizing contract. `max` may be `f32::INFINITY`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitPane {
    pub min: f32,
    pub max: f32,
    /// The consumer has collapsed this pane. SplitView never decides which
    /// pane collapses; it only honours this flag.
    pub collapsed: bool,
}

impl Default for SplitPane {
    fn default() -> Self {
        Self {
            min: 0.0,
            max: f32::INFINITY,
            collapsed: false,
        }
    }
}

impl SplitPane {
    pub fn new(min: f32, max: f32) -> Self {
        Self {
            min: min.max(0.0),
            max: if max.is_finite() {
                max.max(min.max(0.0))
            } else {
                f32::INFINITY
            },
            ..Self::default()
        }
    }

    pub fn collapsed(mut self) -> Self {
        self.collapsed = true;
        self
    }

    /// The effective maximum after normalising an inverted range.
    fn cap(&self) -> f32 {
        if self.collapsed {
            0.0
        } else if self.max < self.min {
            self.min
        } else {
            self.max
        }
    }
}

/// One divider's persisted identity and leading-edge position.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SplitDividerState {
    pub id: String,
    /// Distance from the split view's leading edge to the divider's leading
    /// edge, in logical pixels. This is the only geometry persisted.
    pub position: f32,
}

/// Divider positions for one split view. Persist this value; do not persist
/// pane contents or application state with it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SplitState {
    pub dividers: Vec<SplitDividerState>,
}

impl SplitState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_dividers(dividers: impl IntoIterator<Item = (impl Into<String>, f32)>) -> Self {
        Self {
            dividers: dividers
                .into_iter()
                .map(|(id, position)| SplitDividerState {
                    id: id.into(),
                    position: position.max(0.0),
                })
                .collect(),
        }
    }

    pub fn position(&self, id: &str) -> Option<f32> {
        self.dividers
            .iter()
            .find(|divider| divider.id == id)
            .map(|divider| divider.position)
    }

    pub fn set(&mut self, id: impl Into<String>, position: f32) {
        let id = id.into();
        let position = position.max(0.0);
        if let Some(divider) = self.dividers.iter_mut().find(|divider| divider.id == id) {
            divider.position = position;
        } else {
            self.dividers.push(SplitDividerState { id, position });
        }
    }

    /// Drop dividers that no longer exist (a pane was removed).
    pub fn retain(&mut self, ids: impl IntoIterator<Item = impl AsRef<str>>) {
        let ids: Vec<String> = ids.into_iter().map(|id| id.as_ref().to_owned()).collect();
        self.dividers.retain(|divider| ids.contains(&divider.id));
    }

    /// Positions in divider order, or `None` when the state does not cover
    /// every divider (it is treated as an uninitialised layout).
    pub fn positions(&self, count: usize) -> Option<Vec<f32>> {
        let mut positions = Vec::with_capacity(count);
        for divider in self.dividers.iter().take(count) {
            if divider.position.is_finite() && divider.position >= 0.0 {
                positions.push(divider.position);
            } else {
                return None;
            }
        }
        (positions.len() == count).then_some(positions)
    }
}

/// A span along the split axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SplitSlot {
    pub start: f32,
    pub extent: f32,
}

impl SplitSlot {
    pub fn end(self) -> f32 {
        self.start + self.extent
    }
}

/// The resolved geometry for one split view along its axis. Panes and dividers
/// alternate in leading-to-trailing order.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitLayout {
    pub panes: Vec<SplitSlot>,
    pub dividers: Vec<SplitSlot>,
    /// The panes' minimums did not fit the available extent; every pane was
    /// scaled down proportionally rather than growing the container.
    pub overconstrained: bool,
}

/// The widths the dividers consume for the current collapse/style state.
pub fn divider_widths(panes: &[SplitPane], style: SplitDividerStyle) -> Vec<f32> {
    panes
        .windows(2)
        .map(|pair| {
            let hidden = pair[0].collapsed || pair[1].collapsed;
            if hidden && style == SplitDividerStyle::Thin {
                // HIG p. 337: a zero-width splitter does not remain visible
                // when it has been dragged far enough to hide the subview.
                0.0
            } else {
                style.width()
            }
        })
        .collect()
}

/// Resolve pane and divider spans inside `available`.
///
/// Saved divider positions are leading-edge positions. Missing or inconsistent
/// positions fall back to an even distribution. The trailing pane absorbs
/// growth first and is shrunk last, so a window resize keeps the leading
/// dividers where the user left them.
pub fn layout(
    available: f32,
    panes: &[SplitPane],
    state: &SplitState,
    style: SplitDividerStyle,
) -> SplitLayout {
    let available = available.max(0.0);
    if panes.is_empty() {
        return SplitLayout {
            panes: Vec::new(),
            dividers: Vec::new(),
            overconstrained: false,
        };
    }
    let widths = divider_widths(panes, style);
    let divider_total: f32 = widths.iter().sum();
    let usable = (available - divider_total).max(0.0);
    let count = panes.len();
    let positions = state.positions(count.saturating_sub(1));

    let mut sizes = vec![0.0; count];
    if let Some(positions) = positions {
        // Reconstruct pane sizes from divider leading edges, including the
        // divider widths already consumed by earlier boundaries.
        let mut cursor = 0.0;
        for index in 0..count {
            if index == count - 1 {
                sizes[index] = (usable - cursor).max(0.0);
            } else {
                let edge = positions[index].max(cursor);
                sizes[index] = edge - cursor;
                cursor = edge + widths[index];
            }
        }
    } else {
        let even = usable / count as f32;
        sizes.fill(even);
    }

    let overconstrained = fit(&mut sizes, panes, usable);
    let mut pane_slots = Vec::with_capacity(count);
    let mut divider_slots = Vec::with_capacity(widths.len());
    let mut cursor = 0.0;
    for (index, size) in sizes.into_iter().enumerate() {
        pane_slots.push(SplitSlot {
            start: cursor,
            extent: size,
        });
        cursor += size;
        if let Some(width) = widths.get(index) {
            divider_slots.push(SplitSlot {
                start: cursor,
                extent: *width,
            });
            cursor += *width;
        }
    }
    SplitLayout {
        panes: pane_slots,
        dividers: divider_slots,
        overconstrained,
    }
}

/// Bring raw sizes into the panes' constraints and the usable extent.
fn fit(sizes: &mut [f32], panes: &[SplitPane], usable: f32) -> bool {
    if usable <= 0.0 {
        sizes.fill(0.0);
        return panes.iter().any(|pane| !pane.collapsed && pane.min > 0.0);
    }
    let mut overconstrained = false;
    for (size, pane) in sizes.iter_mut().zip(panes) {
        *size = if pane.collapsed {
            0.0
        } else {
            size.clamp(pane.min, pane.cap())
        };
    }
    let mut total: f32 = sizes.iter().sum();
    if total > usable {
        // Shrink the trailing panes first.
        for (size, pane) in sizes.iter_mut().zip(panes).rev() {
            if total <= usable {
                break;
            }
            if pane.collapsed {
                continue;
            }
            let excess = total - usable;
            let reduction = excess.min((*size - pane.min).max(0.0));
            *size -= reduction;
            total -= reduction;
        }
    } else if total < usable {
        // Grow the trailing panes first, up to their maximum.
        for (size, pane) in sizes.iter_mut().zip(panes).rev() {
            if total >= usable {
                break;
            }
            if pane.collapsed {
                continue;
            }
            let room = (pane.cap() - *size).max(0.0);
            let addition = room.min(usable - total);
            *size += addition;
            total += addition;
        }
    }
    if total > usable + 0.01 && usable > 0.0 {
        // Minimums do not fit. Scale every visible pane, never the container.
        let scale = usable / total;
        for (size, pane) in sizes.iter_mut().zip(panes) {
            if !pane.collapsed {
                *size *= scale;
            }
        }
        overconstrained = true;
    } else if total < usable - 0.01 {
        // A maximum prevented filling the container. Leave the remainder
        // after the panes; callers can surface it as background.
    }
    overconstrained
}

/// Move divider `index` by `delta` logical pixels and return the new divider
/// leading-edge positions. The adjacent panes keep their pair extent; neither
/// may pass its min or max. No collapse decision is made here.
pub fn drag(
    panes: &[SplitPane],
    sizes: &[f32],
    style: SplitDividerStyle,
    index: usize,
    delta: f32,
) -> Option<Vec<f32>> {
    if index + 1 >= panes.len() || sizes.len() != panes.len() {
        return None;
    }
    let lead = panes[index];
    let trail = panes[index + 1];
    if lead.collapsed || trail.collapsed {
        return None;
    }
    let widths = divider_widths(panes, style);
    let pair = sizes[index] + sizes[index + 1];
    let lower = (pair - trail.cap()).max(lead.min);
    let upper = (pair - trail.min).min(lead.cap());
    let proposed = (sizes[index] + delta).clamp(lower.min(upper), upper.max(lower));
    let mut next = sizes.to_vec();
    next[index] = proposed;
    next[index + 1] = (pair - proposed).max(0.0);
    let mut cursor = 0.0;
    let mut positions = Vec::with_capacity(widths.len());
    for pane in 0..panes.len() - 1 {
        cursor += next[pane];
        positions.push(cursor);
        cursor += widths[pane];
    }
    Some(positions)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panes(count: usize, min: f32) -> Vec<SplitPane> {
        (0..count)
            .map(|_| SplitPane::new(min, f32::INFINITY))
            .collect()
    }

    #[test]
    fn divider_styles_match_the_hig() {
        assert_eq!(SplitDividerStyle::Thin.width(), 1.0);
        assert_eq!(SplitDividerStyle::Thick.width(), 9.0);
        assert_eq!(SplitDividerStyle::Thin.hot_zone(), 2.0);
        assert_eq!(SplitDividerStyle::Thick.hot_zone(), 0.0);
    }

    #[test]
    fn saved_positions_define_pane_spans() {
        let panes = panes(3, 0.0);
        let state = SplitState::with_dividers([("a", 120.0), ("b", 340.0)]);
        let layout = layout(500.0, &panes, &state, SplitDividerStyle::Thin);
        assert_eq!(
            layout.panes,
            vec![
                SplitSlot {
                    start: 0.0,
                    extent: 120.0
                },
                SplitSlot {
                    start: 121.0,
                    extent: 219.0
                },
                SplitSlot {
                    start: 341.0,
                    extent: 159.0
                }
            ]
        );
        assert_eq!(layout.dividers[0].start, 120.0);
        assert_eq!(layout.dividers[1].start, 340.0);
    }

    #[test]
    fn trailing_panes_absorb_growth_and_shrink_before_leading_ones() {
        let panes = panes(3, 0.0);
        let state = SplitState::with_dividers([("a", 100.0), ("b", 200.0)]);
        let small = layout(300.0, &panes, &state, SplitDividerStyle::Thin);
        assert_eq!(small.panes[0].extent, 100.0);
        assert_eq!(small.panes[1].extent, 99.0);
        assert_eq!(small.panes[2].extent, 99.0);
        let large = layout(600.0, &panes, &state, SplitDividerStyle::Thin);
        assert_eq!(large.panes[0].extent, 100.0);
        assert_eq!(large.panes[1].extent, 99.0);
        assert_eq!(large.panes[2].extent, 399.0);
    }

    #[test]
    fn minimums_never_push_the_layout_past_the_available_extent() {
        let panes = panes(3, 220.0);
        let layout = layout(500.0, &panes, &SplitState::new(), SplitDividerStyle::Thin);
        let total: f32 = layout.panes.iter().map(|pane| pane.extent).sum();
        assert!(layout.overconstrained);
        assert!((total - 498.0).abs() < 0.001, "total {total}");
        assert!(layout.panes.iter().all(|pane| pane.extent > 0.0));
    }

    #[test]
    fn zero_extent_never_leaves_an_oversized_layout() {
        let panes = panes(2, 80.0);
        let layout = layout(0.0, &panes, &SplitState::new(), SplitDividerStyle::Thin);
        assert!(layout.panes.iter().all(|pane| pane.extent == 0.0));
        assert!(layout.overconstrained);
    }

    #[test]
    fn maximums_leave_the_unused_remainder_after_the_panes() {
        let panes = vec![
            SplitPane::new(0.0, 100.0),
            SplitPane::new(0.0, 100.0),
            SplitPane::new(0.0, 100.0),
        ];
        let layout = layout(500.0, &panes, &SplitState::new(), SplitDividerStyle::Thin);
        let total: f32 = layout.panes.iter().map(|pane| pane.extent).sum();
        assert_eq!(total, 300.0);
        assert!(!layout.overconstrained);
    }

    #[test]
    fn a_collapsed_pane_keeps_a_thick_divider_and_hides_a_thin_one() {
        let mut panes = panes(2, 0.0);
        panes[0].collapsed = true;
        let thick = layout(400.0, &panes, &SplitState::new(), SplitDividerStyle::Thick);
        assert_eq!(thick.panes[0].extent, 0.0);
        assert_eq!(thick.dividers[0].extent, 9.0);
        assert_eq!(thick.panes[1].start, 9.0);
        let thin = layout(400.0, &panes, &SplitState::new(), SplitDividerStyle::Thin);
        assert_eq!(thin.dividers[0].extent, 0.0);
        assert_eq!(thin.panes[1].start, 0.0);
    }

    #[test]
    fn drag_clamps_to_both_adjacent_constraints() {
        let panes = vec![SplitPane::new(80.0, 220.0), SplitPane::new(80.0, 300.0)];
        let positions =
            drag(&panes, &[140.0, 200.0], SplitDividerStyle::Thin, 0, 1000.0).expect("divider");
        assert_eq!(positions, vec![220.0]);
        let positions =
            drag(&panes, &[140.0, 200.0], SplitDividerStyle::Thin, 0, -1000.0).expect("divider");
        assert_eq!(positions, vec![80.0]);
        let positions =
            drag(&panes, &[140.0, 200.0], SplitDividerStyle::Thin, 0, 0.0).expect("divider");
        assert_eq!(positions, vec![140.0]);
    }

    #[test]
    fn state_round_trips_only_positions_and_ids() {
        let state = SplitState::with_dividers([("sidebar", 240.0), ("list", 520.0)]);
        let json = serde_json::to_string(&state).expect("serialize");
        assert!(json.contains("sidebar") && json.contains("240"));
        let back: SplitState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, state);
    }

    #[test]
    fn removing_a_pane_drops_its_divider_state() {
        let mut state = SplitState::with_dividers([("a", 100.0), ("b", 200.0)]);
        state.retain(["a"]);
        assert_eq!(state.dividers.len(), 1);
        assert_eq!(state.position("a"), Some(100.0));
        assert_eq!(state.position("b"), None);
    }
}
