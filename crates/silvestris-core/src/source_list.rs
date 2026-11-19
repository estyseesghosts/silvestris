//! Generic source-list state (#128/#130; handbook 09 section C).
//! Expansion and selection are independent. Stable IDs survive reorder;
//! collapsed descendants retain selection until a navigation request.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceNode {
    pub id: String,
    pub label: String,
    pub disabled: bool,
    /// Containers may disclose children without being selection targets.
    pub selectable: bool,
    /// Nonselectable, always-open section heading (handbook 13).
    pub section: bool,
    /// Semantic icon role resolved by the consumer's icon renderer.
    pub icon: Option<String>,
    pub badge: Option<String>,
    pub children: Vec<SourceNode>,
}

impl SourceNode {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            disabled: false,
            selectable: true,
            section: false,
            icon: None,
            badge: None,
            children: Vec::new(),
        }
    }

    pub fn section(id: impl Into<String>, label: impl Into<String>, children: Vec<Self>) -> Self {
        Self {
            section: true,
            children,
            ..Self::new(id, label)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceRow {
    pub id: String,
    pub label: String,
    pub depth: usize,
    pub parent: Option<String>,
    pub disabled: bool,
    pub selectable: bool,
    pub branch: bool,
    pub expanded: bool,
    pub section: bool,
    pub icon: Option<String>,
    pub badge: Option<String>,
}

impl SourceRow {
    pub fn selectable(&self) -> bool {
        self.selectable && !self.disabled && !self.section
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Navigation {
    Up,
    Down,
    Home,
    End,
    Left,
    Right,
}

#[derive(Clone, Debug, Default)]
pub struct SourceTree {
    nodes: Vec<SourceNode>,
    expanded: BTreeSet<String>,
    selected: Option<String>,
    rows: Arc<Vec<SourceRow>>,
    // All nodes, including collapsed ones, for selection/reconciliation.
    available: BTreeMap<String, Availability>,
}

#[derive(Clone, Debug)]
struct Availability {
    selectable: bool,
    expandable: bool,
    branch: bool,
}

impl SourceTree {
    /// Duplicate IDs reject the entire replacement without changing state.
    pub fn replace(&mut self, nodes: Vec<SourceNode>) -> Result<(), String> {
        if self.nodes == nodes {
            return Ok(());
        }
        fn index(
            nodes: &[SourceNode],
            out: &mut BTreeMap<String, Availability>,
        ) -> Result<(), String> {
            for n in nodes {
                if out
                    .insert(
                        n.id.clone(),
                        Availability {
                            selectable: n.selectable && !n.disabled && !n.section,
                            expandable: !n.disabled && !n.section && !n.children.is_empty(),
                            branch: !n.children.is_empty(),
                        },
                    )
                    .is_some()
                {
                    return Err(format!("duplicate source-list ID: {}", n.id));
                }
                index(&n.children, out)?;
            }
            Ok(())
        }
        let mut available = BTreeMap::new();
        index(&nodes, &mut available)?;
        self.expanded
            .retain(|id| available.get(id).is_some_and(|s| s.branch));
        self.selected = self
            .selected
            .take()
            .filter(|id| available.get(id).is_some_and(|s| s.selectable));
        self.nodes = nodes;
        self.available = available;
        self.rebuild();
        Ok(())
    }

    pub fn rows(&self) -> &[SourceRow] {
        &self.rows
    }
    /// Cheap immutable snapshot for virtualized render closures.
    pub fn rows_snapshot(&self) -> Arc<Vec<SourceRow>> {
        self.rows.clone()
    }
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub fn clear_selection(&mut self) -> bool {
        self.selected.take().is_some()
    }

    pub fn select(&mut self, id: &str) -> bool {
        if self.available.get(id).is_none_or(|s| !s.selectable) || self.selected() == Some(id) {
            return false;
        }
        self.selected = Some(id.to_owned());
        true
    }

    /// Returns whether expansion changed. Never selects a node.
    pub fn expand(&mut self, id: &str, expanded: bool) -> bool {
        if !self.available.get(id).is_some_and(|s| s.expandable) {
            return false;
        }
        let changed = if expanded {
            self.expanded.insert(id.to_owned())
        } else {
            self.expanded.remove(id)
        };
        if changed {
            self.rebuild();
        }
        changed
    }

    pub fn navigate(&mut self, direction: Navigation) {
        let current = self
            .rows
            .iter()
            .position(|r| Some(r.id.as_str()) == self.selected());
        let selectable: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| r.selectable().then_some(i))
            .collect();
        let target = match direction {
            Navigation::Home => selectable.first().copied(),
            Navigation::End => selectable.last().copied(),
            Navigation::Down => selectable
                .iter()
                .copied()
                .find(|i| current.is_none_or(|c| *i > c)),
            Navigation::Up => selectable
                .iter()
                .rev()
                .copied()
                .find(|i| current.is_none_or(|c| *i < c)),
            Navigation::Right => {
                if let Some(i) = current {
                    let row = self.rows[i].clone();
                    if row.branch && !row.expanded {
                        self.expand(&row.id, true);
                        return;
                    }
                    self.rows
                        .iter()
                        .enumerate()
                        .skip(i + 1)
                        .take_while(|(_, r)| r.depth > row.depth)
                        .find(|(_, r)| r.selectable())
                        .map(|(i, _)| i)
                } else {
                    selectable.first().copied()
                }
            }
            Navigation::Left => {
                if let Some(i) = current {
                    let row = self.rows[i].clone();
                    if row.expanded {
                        self.expand(&row.id, false);
                        return;
                    }
                    let mut parent = row.parent;
                    let mut target = None;
                    while let Some(id) = parent {
                        let Some(i) = self.rows.iter().position(|r| r.id == id) else {
                            break;
                        };
                        if self.rows[i].selectable() {
                            target = Some(i);
                            break;
                        }
                        parent = self.rows[i].parent.clone();
                    }
                    target
                } else {
                    selectable.first().copied()
                }
            }
        };
        if let Some(i) = target {
            self.selected = Some(self.rows[i].id.clone());
        }
    }

    fn rebuild(&mut self) {
        fn visit(
            nodes: &[SourceNode],
            depth: usize,
            parent: Option<&str>,
            expanded: &BTreeSet<String>,
            rows: &mut Vec<SourceRow>,
        ) {
            for n in nodes {
                let open = !n.children.is_empty() && (n.section || expanded.contains(&n.id));
                rows.push(SourceRow {
                    id: n.id.clone(),
                    label: n.label.clone(),
                    depth,
                    parent: parent.map(str::to_owned),
                    disabled: n.disabled,
                    selectable: n.selectable,
                    branch: !n.children.is_empty(),
                    expanded: open,
                    section: n.section,
                    icon: n.icon.clone(),
                    badge: n.badge.clone(),
                });
                if open {
                    visit(
                        &n.children,
                        depth + usize::from(!n.section),
                        Some(&n.id),
                        expanded,
                        rows,
                    );
                }
            }
        }
        let mut rows = Vec::new();
        visit(&self.nodes, 0, None, &self.expanded, &mut rows);
        self.rows = Arc::new(rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree() -> SourceTree {
        let mut root = SourceNode::new("root", "Collection");
        root.children = vec![SourceNode::new("a", "Alpha"), SourceNode::new("b", "Beta")];
        let mut disabled = SourceNode::new("disabled", "Unavailable");
        disabled.disabled = true;
        let mut t = SourceTree::default();
        t.replace(vec![root, disabled, SourceNode::new("last", "Last")])
            .unwrap();
        t
    }
    #[test]
    fn disclosure_does_not_select_and_hidden_selection_survives() {
        let mut t = tree();
        assert_eq!(t.rows().len(), 3);
        t.select("last");
        assert!(t.expand("root", true));
        assert_eq!(t.selected(), Some("last"));
        t.select("a");
        t.expand("root", false);
        assert_eq!(t.selected(), Some("a"));
        t.navigate(Navigation::Down);
        assert_eq!(t.selected(), Some("root"));
    }
    #[test]
    fn arrows_expand_enter_children_and_return_to_parent() {
        let mut t = tree();
        t.navigate(Navigation::Home);
        t.navigate(Navigation::Right);
        assert_eq!(t.selected(), Some("root"));
        assert_eq!(t.rows().len(), 5);
        t.navigate(Navigation::Right);
        assert_eq!(t.selected(), Some("a"));
        t.navigate(Navigation::Down);
        assert_eq!(t.selected(), Some("b"));
        t.navigate(Navigation::Left);
        assert_eq!(t.selected(), Some("root"));
        t.navigate(Navigation::Left);
        assert_eq!(t.rows().len(), 3);
    }
    #[test]
    fn disabled_and_edges_and_empty() {
        let mut t = tree();
        assert!(!t.select("disabled"));
        assert!(!t.expand("disabled", true));
        t.navigate(Navigation::Home);
        t.navigate(Navigation::Up);
        assert_eq!(t.selected(), Some("root"));
        t.navigate(Navigation::Down);
        assert_eq!(t.selected(), Some("last"));
        t.navigate(Navigation::Down);
        assert_eq!(t.selected(), Some("last"));
        assert!(t.clear_selection());
        assert_eq!(t.selected(), None);
        assert!(!t.clear_selection());
        t.replace(vec![]).unwrap();
        for n in [
            Navigation::Up,
            Navigation::Down,
            Navigation::Home,
            Navigation::End,
            Navigation::Left,
            Navigation::Right,
        ] {
            t.navigate(n);
        }
        assert_eq!(t.selected(), None);
    }
    #[test]
    fn replacement_retains_identity_and_rejects_duplicates_atomically() {
        let mut t = tree();
        t.expand("root", true);
        t.select("a");
        let old = t.rows().to_vec();
        assert!(t
            .replace(vec![
                SourceNode::new("x", "One"),
                SourceNode::new("x", "Two")
            ])
            .is_err());
        assert_eq!(t.rows(), old);
        assert_eq!(t.selected(), Some("a"));
        let mut nodes = t.nodes.clone();
        nodes.reverse();
        t.replace(nodes).unwrap();
        assert_eq!(t.selected(), Some("a"));
        assert!(t.rows().iter().any(|r| r.id == "root" && r.expanded));
        t.replace(vec![SourceNode::new("new", "New")]).unwrap();
        assert_eq!(t.selected(), None);
    }
    #[test]
    fn snapshots_are_shared_until_structure_changes() {
        let mut t = tree();
        let snapshot = t.rows_snapshot();
        assert!(Arc::ptr_eq(&snapshot, &t.rows_snapshot()));
        t.select("root");
        assert!(Arc::ptr_eq(&snapshot, &t.rows_snapshot()));
        t.expand("root", true);
        assert_eq!(snapshot.len(), 3);
        assert_eq!(t.rows().len(), 5);
        assert!(!Arc::ptr_eq(&snapshot, &t.rows_snapshot()));
    }
    #[test]
    fn sections_are_open_but_never_selected_or_collapsed() {
        let mut a = SourceNode::new("a", "Alpha");
        a.icon = Some("document".into());
        a.badge = Some("3".into());
        let mut t = SourceTree::default();
        t.replace(vec![SourceNode::section(
            "group",
            "Documents",
            vec![a, SourceNode::new("b", "Beta")],
        )])
        .unwrap();
        assert_eq!(t.rows().len(), 3);
        assert!(!t.select("group"));
        assert!(!t.expand("group", false));
        assert_eq!(t.rows()[1].depth, 0);
        assert_eq!(t.rows()[1].icon.as_deref(), Some("document"));
        assert_eq!(t.rows()[1].badge.as_deref(), Some("3"));
        t.navigate(Navigation::Home);
        assert_eq!(t.selected(), Some("a"));
        t.navigate(Navigation::Left);
        assert_eq!(t.selected(), Some("a"));
        t.navigate(Navigation::End);
        assert_eq!(t.selected(), Some("b"));
        let mut node = SourceNode::new("b", "Now a heading");
        node.section = true;
        t.replace(vec![node]).unwrap();
        assert_eq!(t.selected(), None);
    }
    #[test]
    fn nonselectable_containers_still_disclose_and_identical_updates_share_rows() {
        let mut root = SourceNode::new("offline-card", "Offline Card");
        root.selectable = false;
        root.children.push(SourceNode::new("copy", "Running Copy"));
        let mut tree = SourceTree::default();
        tree.replace(vec![root.clone()]).unwrap();
        let snapshot = tree.rows_snapshot();
        tree.replace(vec![root]).unwrap();
        assert!(Arc::ptr_eq(&snapshot, &tree.rows_snapshot()));
        assert!(!tree.select("offline-card"));
        assert!(tree.expand("offline-card", true));
        tree.navigate(Navigation::Home);
        assert_eq!(tree.selected(), Some("copy"));
        tree.navigate(Navigation::Left);
        assert_eq!(tree.selected(), Some("copy"));
        assert!(tree.expand("offline-card", false));
        assert_eq!(tree.selected(), Some("copy"));
    }

    #[test]
    fn thousands_of_rows_keep_visible_order() {
        let mut t = SourceTree::default();
        t.replace(
            (0..10000)
                .map(|i| SourceNode::new(i.to_string(), format!("Item {i}")))
                .collect(),
        )
        .unwrap();
        t.navigate(Navigation::End);
        assert_eq!(t.selected(), Some("9999"));
        t.navigate(Navigation::Up);
        assert_eq!(t.selected(), Some("9998"));
    }
}
