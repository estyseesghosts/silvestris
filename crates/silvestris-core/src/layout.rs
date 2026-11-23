//! Shell layout planner: persistent source list, inspector rail, docked or
//! overlay inspector, and the workspace between them (#130-#132, handbook 13).
//!
//! Pure functions over GPUI logical pixels. The same planner runs for every
//! theme; themes only change the geometry it is given.
//!
//! Policy, in the order space is given away when the window narrows:
//! 1. A pinned inspector that no longer fits is shown as an overlay; its saved
//!    `pinned` stays true and it docks again when room returns.
//! 2. The expanded source list shrinks toward its minimum.
//! 3. The inspector rail is hidden.
//! 4. The source list collapses to its configured rail width. It never becomes an overlay.
//!
//! Opening or closing an unpinned (overlay) inspector never changes the
//! workspace rectangle. Demotions are per-viewport and never written back to
//! preferences; `hysteresis` keeps a demotion until the window is that much
//! wider than the threshold that caused it.

/// Constraints supplied by the consumer and the active theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellGeometry {
    /// Width of the collapsed sidebar. Zero hides it entirely.
    pub sidebar_rail: f32,
    pub sidebar_min: f32,
    pub sidebar_max: f32,
    pub inspector_rail: f32,
    pub inspector_min: f32,
    pub inspector_max: f32,
    /// Narrowest usable workspace.
    pub workspace_min: f32,
    pub hysteresis: f32,
}

impl ShellGeometry {
    /// Values Lotus shipped before Silvestris (lotus crates/desktop/src/layout.rs:
    /// SESSION_RAIL 52, SESSION_MIN/MAX 200/440, INSPECTOR_RAIL 44,
    /// INSPECTOR_MIN/MAX 300/760, MAIN_MIN 320). Handbook 13 keeps them as the
    /// starting point; they are not 10.6 measurements.
    pub const LOTUS_LEGACY: ShellGeometry = ShellGeometry {
        sidebar_rail: 52.0,
        sidebar_min: 200.0,
        sidebar_max: 440.0,
        inspector_rail: 44.0,
        inspector_min: 300.0,
        inspector_max: 760.0,
        workspace_min: 320.0,
        hysteresis: 16.0,
    };

    fn clamp_sidebar(&self, w: f32) -> f32 {
        w.clamp(self.sidebar_min, self.sidebar_max)
    }
    fn clamp_inspector(&self, w: f32) -> f32 {
        w.clamp(self.inspector_min, self.inspector_max)
    }
}

/// The user's source-list choice. `width` survives collapse.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SidebarPreference {
    pub expanded: bool,
    pub width: f32,
}

/// The user's inspector choice. Visibility and pinning are independent
/// (handbook 13: "Do not equate inspector_open with docked").
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InspectorPreference {
    pub visible: bool,
    pub pinned: bool,
    pub width: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Presentation {
    Closed,
    Overlay,
    Docked,
}

/// A horizontal span in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Span {
    pub x: f32,
    pub width: f32,
}

impl Span {
    pub fn right(&self) -> f32 {
        self.x + self.width
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellPlan {
    pub sidebar: Span,
    /// The source list shows its full rows (false: its rail or no sidebar).
    pub sidebar_expanded: bool,
    /// Expanded by preference but drawn as a rail or narrowed for lack of room.
    pub sidebar_demoted: bool,
    pub workspace: Span,
    /// Docked inspector column, right of the workspace.
    pub dock: Option<Span>,
    /// Inspector rail, outermost on the right.
    pub rail: Option<Span>,
    pub presentation: Presentation,
    /// Pinned by preference but shown as an overlay for lack of room.
    pub pin_demoted: bool,
    /// Overlay panel span; it floats over the workspace and never moves it.
    pub overlay: Option<Span>,
    /// Inspector rail wanted but hidden for lack of room.
    pub rail_demoted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlanInput<'a> {
    pub viewport: f32,
    pub geometry: ShellGeometry,
    pub sidebar: SidebarPreference,
    pub inspector: InspectorPreference,
    /// The consumer shows an inspector rail at all.
    pub rail_enabled: bool,
    /// Previous plan for hysteresis.
    pub previous: Option<&'a ShellPlan>,
}

pub fn plan(input: PlanInput) -> ShellPlan {
    let PlanInput {
        viewport,
        geometry: g,
        sidebar,
        inspector,
        rail_enabled,
        previous,
    } = input;
    // A demotion made at the previous size is kept until the window grows by
    // `hysteresis` past what the promoted state needs.
    let slack = |demoted: bool| if demoted { g.hysteresis } else { 0.0 };
    let prev = previous.copied();

    // Source list.
    let mut s_expanded = sidebar.expanded;
    let mut s = if sidebar.expanded {
        g.clamp_sidebar(sidebar.width)
    } else {
        g.sidebar_rail
    };
    let mut s_demoted = false;
    if sidebar.expanded {
        let was = prev.map(|p| p.sidebar_demoted).unwrap_or(false);
        let room = viewport - g.workspace_min - slack(was);
        if s > room {
            s_demoted = true;
            if room >= g.sidebar_min {
                s = room;
            } else {
                s = g.sidebar_rail;
                s_expanded = false;
            }
        }
    }

    // Inspector rail.
    let mut r = 0.0;
    let mut rail_demoted = false;
    if rail_enabled {
        let was = prev.map(|p| p.rail_demoted).unwrap_or(false);
        if viewport - s - g.inspector_rail - slack(was) >= g.workspace_min {
            r = g.inspector_rail;
        } else {
            rail_demoted = true;
        }
    }

    // Docked inspector.
    let mut dock = 0.0;
    let mut pin_demoted = false;
    let presentation = if !inspector.visible {
        Presentation::Closed
    } else if inspector.pinned {
        let was = prev.map(|p| p.pin_demoted).unwrap_or(false);
        let room = viewport - s - r - g.workspace_min - slack(was);
        let want = g.clamp_inspector(inspector.width);
        if room >= g.inspector_min {
            dock = want.min(room);
            Presentation::Docked
        } else {
            pin_demoted = true;
            Presentation::Overlay
        }
    } else {
        Presentation::Overlay
    };

    let workspace = Span {
        x: s,
        width: (viewport - s - r - dock).max(0.0),
    };
    let overlay = (presentation == Presentation::Overlay).then(|| {
        let w = g.clamp_inspector(inspector.width).min(workspace.width);
        Span {
            x: workspace.right() - w,
            width: w,
        }
    });
    ShellPlan {
        sidebar: Span { x: 0.0, width: s },
        sidebar_expanded: s_expanded,
        sidebar_demoted: s_demoted,
        workspace,
        dock: (dock > 0.0).then_some(Span {
            x: workspace.right(),
            width: dock,
        }),
        rail: (r > 0.0).then_some(Span {
            x: viewport - r,
            width: r,
        }),
        presentation,
        pin_demoted,
        overlay,
        rail_demoted,
    }
}

/// Which divider is being dragged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Divider {
    Sidebar,
    /// Docked inspector, or the overlay's leading edge.
    Inspector,
}

/// Width for a divider at window x `pointer`. Below half the minimum the pane
/// snaps closed (`None`) and keeps `start_width` in preferences; the same
/// threshold applies in both directions (VS Code splitview `onSashStart`
/// `halfSize = Math.floor(viewItem.viewMinimumSize / 2)`, as Lotus layout.rs
/// cites). `other` is the rendered width of everything else that is not the
/// workspace.
pub fn drag(
    divider: Divider,
    g: &ShellGeometry,
    pointer: f32,
    viewport: f32,
    other: f32,
) -> Option<f32> {
    let (min, max, proposed) = match divider {
        Divider::Sidebar => (g.sidebar_min, g.sidebar_max, pointer),
        Divider::Inspector => (g.inspector_min, g.inspector_max, viewport - pointer - other),
    };
    if proposed < min / 2.0 {
        return None;
    }
    let room = match divider {
        Divider::Sidebar => viewport - other - g.workspace_min,
        Divider::Inspector => f32::INFINITY,
    };
    Some(proposed.min(max).min(room).max(min))
}

/// A toolbar item for overflow fitting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarSlot {
    pub width: f32,
    /// Higher stays visible longer.
    pub priority: u8,
}

/// Which items fit in `available` (order preserved); the rest go to the
/// overflow menu, whose button takes `overflow_width` when anything spills.
/// Every command stays reachable (handbook 16: "Toolbar actions all
/// accessible via visible buttons or overflow").
pub fn fit_toolbar(
    items: &[ToolbarSlot],
    available: f32,
    gap: f32,
    overflow_width: f32,
) -> Vec<bool> {
    let total = |vis: &[bool], with_overflow: bool| {
        let mut w = 0.0;
        let mut n = 0;
        for (item, v) in items.iter().zip(vis) {
            if *v {
                w += item.width;
                n += 1;
            }
        }
        if with_overflow {
            w += overflow_width;
            n += 1;
        }
        w + gap * (n.max(1) - 1) as f32
    };
    let mut visible = vec![true; items.len()];
    if total(&visible, false) <= available {
        return visible;
    }
    let mut order: Vec<usize> = (0..items.len()).collect();
    // Lowest priority first; among equals, the rightmost first.
    order.sort_by(|a, b| items[*a].priority.cmp(&items[*b].priority).then(b.cmp(a)));
    for i in order {
        visible[i] = false;
        if total(&visible, true) <= available {
            break;
        }
    }
    visible
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: ShellGeometry = ShellGeometry::LOTUS_LEGACY;

    fn input(viewport: f32, s: SidebarPreference, i: InspectorPreference) -> PlanInput<'static> {
        PlanInput {
            viewport,
            geometry: G,
            sidebar: s,
            inspector: i,
            rail_enabled: true,
            previous: None,
        }
    }
    const S: SidebarPreference = SidebarPreference {
        expanded: true,
        width: 264.0,
    };
    const CLOSED: InspectorPreference = InspectorPreference {
        visible: false,
        pinned: false,
        width: 420.0,
    };
    const OVERLAY: InspectorPreference = InspectorPreference {
        visible: true,
        pinned: false,
        width: 420.0,
    };
    const PINNED: InspectorPreference = InspectorPreference {
        visible: true,
        pinned: true,
        width: 420.0,
    };

    #[test]
    fn overlay_never_moves_the_workspace() {
        for w in [720.0, 900.0, 1280.0, 1920.0] {
            let a = plan(input(w, S, CLOSED));
            let b = plan(input(w, S, OVERLAY));
            assert_eq!(a.workspace, b.workspace, "{w}");
            let o = b.overlay.unwrap();
            assert!(o.x >= b.workspace.x && o.right() <= b.workspace.right() + 0.001);
        }
    }

    #[test]
    fn pinning_docks_and_narrow_windows_demote_without_forgetting() {
        let wide = plan(input(1600.0, S, PINNED));
        assert_eq!(wide.presentation, Presentation::Docked);
        assert_eq!(wide.dock.unwrap().width, 420.0);
        assert_eq!(wide.workspace.width, 1600.0 - 264.0 - 44.0 - 420.0);
        let narrow = plan(input(800.0, S, PINNED));
        assert_eq!(narrow.presentation, Presentation::Overlay);
        assert!(narrow.pin_demoted);
        // Growing back docks again (preference untouched).
        let back = plan(input(1600.0, S, PINNED));
        assert_eq!(back.presentation, Presentation::Docked);
    }

    #[test]
    fn hysteresis_keeps_a_demotion_until_the_window_is_clearly_wider() {
        // Docking needs 264 + 44 + 300 + 320 = 928.
        let at = plan(input(927.0, S, PINNED));
        assert!(at.pin_demoted);
        let mut i = input(930.0, S, PINNED);
        i.previous = Some(&at);
        assert!(plan(i).pin_demoted, "within hysteresis");
        let mut i = input(950.0, S, PINNED);
        i.previous = Some(&at);
        assert!(!plan(i).pin_demoted);
    }

    #[test]
    fn sidebar_is_never_an_overlay_and_rail_goes_before_it() {
        let p = plan(input(600.0, S, OVERLAY));
        assert!(p.sidebar_expanded);
        assert!(p.rail.is_none() && p.rail_demoted);
        let p = plan(input(500.0, S, OVERLAY));
        assert!(!p.sidebar_expanded && p.sidebar_demoted);
        assert_eq!(p.sidebar.width, G.sidebar_rail);
    }

    /// Handbook 16 "Layout property tests", every 1 px from 200 to 2600.
    #[test]
    fn properties_hold_at_every_width() {
        let sidebars = [
            S,
            SidebarPreference {
                expanded: false,
                width: 264.0,
            },
            SidebarPreference {
                expanded: true,
                width: 440.0,
            },
        ];
        let inspectors = [CLOSED, OVERLAY, PINNED];
        for s in sidebars {
            for i in inspectors {
                for rail_enabled in [true, false] {
                    let mut prev: Option<ShellPlan> = None;
                    for w in 200..=2600 {
                        let w = w as f32;
                        let mut inp = input(w, s, i);
                        inp.rail_enabled = rail_enabled;
                        let p = plan(inp);
                        let parts = p.sidebar.width
                            + p.workspace.width
                            + p.dock.map_or(0.0, |d| d.width)
                            + p.rail.map_or(0.0, |r| r.width);
                        // Reserved panes are counted exactly once and never overlap.
                        if w >= G.sidebar_rail + G.workspace_min {
                            assert!((parts - w).abs() < 0.01, "{w} {p:?}");
                        }
                        assert!(p.sidebar.width >= 0.0 && p.workspace.width >= 0.0);
                        assert_eq!(p.workspace.x, p.sidebar.right());
                        if let Some(d) = p.dock {
                            assert_eq!(d.x, p.workspace.right());
                            assert!(i.pinned && i.visible);
                        }
                        if let Some(r) = p.rail {
                            assert!((r.right() - w).abs() < 0.01);
                        }
                        // Pinned only consumes width when actually docked.
                        assert_eq!(p.dock.is_some(), p.presentation == Presentation::Docked);
                        // Source list stays a pane or rail.
                        assert!(p.sidebar.width >= G.sidebar_rail.min(w));
                        // Usable workspace whenever the window can hold one beside the rail.
                        if w >= G.sidebar_rail + G.workspace_min {
                            assert!(p.workspace.width >= G.workspace_min - 0.01, "{w} {p:?}");
                        }
                        // Overlay stays inside the workspace.
                        if let Some(o) = p.overlay {
                            assert!(
                                o.x >= p.workspace.x - 0.01
                                    && o.right() <= p.workspace.right() + 0.01
                            );
                        }
                        // Overlay open/close: bit-identical workspace.
                        if i.visible && !i.pinned {
                            let closed = plan(PlanInput {
                                inspector: CLOSED,
                                ..inp
                            });
                            assert_eq!(closed.workspace, p.workspace);
                        }
                        let mut with_prev = inp;
                        with_prev.previous = prev.as_ref();
                        prev = Some(plan(with_prev));
                    }
                }
            }
        }
    }

    #[test]
    fn drag_snaps_at_half_minimum_like_lotus() {
        assert_eq!(drag(Divider::Sidebar, &G, 99.0, 1600.0, 464.0), None);
        assert_eq!(
            drag(Divider::Sidebar, &G, 100.0, 1600.0, 464.0),
            Some(200.0)
        );
        assert_eq!(
            drag(Divider::Sidebar, &G, 333.0, 1600.0, 464.0),
            Some(333.0)
        );
        assert_eq!(
            drag(Divider::Sidebar, &G, 900.0, 1600.0, 464.0),
            Some(440.0)
        );
        assert_eq!(
            drag(Divider::Sidebar, &G, 400.0, 1000.0, 464.0),
            Some(216.0)
        );
        // Inspector: 1600 - 1200 - 44 = 356.
        assert_eq!(
            drag(Divider::Inspector, &G, 1200.0, 1600.0, 44.0),
            Some(356.0)
        );
        assert_eq!(
            drag(Divider::Inspector, &G, 1600.0 - 44.0 - 149.0, 1600.0, 44.0),
            None
        );
    }

    #[test]
    fn toolbar_overflow_drops_low_priority_first_and_keeps_order() {
        let items = [
            ToolbarSlot {
                width: 60.0,
                priority: 9,
            },
            ToolbarSlot {
                width: 60.0,
                priority: 1,
            },
            ToolbarSlot {
                width: 60.0,
                priority: 5,
            },
            ToolbarSlot {
                width: 60.0,
                priority: 1,
            },
        ];
        assert_eq!(fit_toolbar(&items, 400.0, 8.0, 30.0), vec![true; 4]);
        // 60*3+30 + 3*8 = 234 fits after dropping the rightmost priority-1 item.
        assert_eq!(
            fit_toolbar(&items, 240.0, 8.0, 30.0),
            vec![true, true, true, false]
        );
        assert_eq!(
            fit_toolbar(&items, 170.0, 8.0, 30.0),
            vec![true, false, true, false]
        );
    }
}
