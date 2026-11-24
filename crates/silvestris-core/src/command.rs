//! Command model shared by menus, toolbars, context menus and shortcuts (#129).
//!
//! A command is identified by a stable namespaced string. Descriptors are
//! snapshots: the consumer publishes new ones when enabled/checked state
//! changes (handbook 03 "Descriptors are snapshots"). The toolkit never runs
//! application logic here. The GPUI adapter dispatches registered consumer
//! callbacks through these stable IDs.

use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(pub Arc<str>);

impl CommandId {
    pub fn new(id: impl AsRef<str>) -> Self {
        Self(Arc::from(id.as_ref()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for CommandId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl std::fmt::Display for CommandId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    Action,
    /// Checkmark when on.
    Toggle,
    /// One of a mutually exclusive group (HIG p. 165-190: radio items show a checkmark).
    Radio {
        group: Arc<str>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct CommandDescriptor {
    pub id: CommandId,
    pub label: String,
    pub tooltip: Option<String>,
    /// Icon role understood by the consumer's icon provider.
    pub icon: Option<Arc<str>>,
    /// GPUI keystroke syntax; `secondary-` is Command on macOS and Ctrl
    /// elsewhere (gpui/src/platform/keystroke.rs `parse_with_os`).
    pub shortcut: Option<Arc<str>>,
    pub kind: CommandKind,
    pub enabled: bool,
    pub checked: bool,
    /// The attribute applies to part of the selection; menus show a dash
    /// instead of a checkmark (HIG pp. 170-171). Exclusive with `checked`.
    pub mixed: bool,
    pub destructive: bool,
    /// The command asks for more input before acting; menus append "…"
    /// (HIG pp. 131-132 via hig-09 text.md).
    pub needs_input: bool,
}

impl CommandDescriptor {
    pub fn action(id: &str, label: &str) -> Self {
        Self {
            id: CommandId::new(id),
            label: label.to_string(),
            tooltip: None,
            icon: None,
            shortcut: None,
            kind: CommandKind::Action,
            enabled: true,
            checked: false,
            mixed: false,
            destructive: false,
            needs_input: false,
        }
    }
    pub fn toggle(id: &str, label: &str, checked: bool) -> Self {
        Self {
            kind: CommandKind::Toggle,
            checked,
            ..Self::action(id, label)
        }
    }
    pub fn radio(id: &str, label: &str, group: &str, checked: bool) -> Self {
        Self {
            kind: CommandKind::Radio {
                group: Arc::from(group),
            },
            checked,
            ..Self::action(id, label)
        }
    }
    pub fn shortcut(mut self, keys: &str) -> Self {
        self.shortcut = Some(Arc::from(keys));
        self
    }
    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = Some(Arc::from(icon));
        self
    }
    pub fn tooltip(mut self, tip: &str) -> Self {
        self.tooltip = Some(tip.to_string());
        self
    }
    /// Mark the attribute as applying to part of the selection (HIG pp. 170-171).
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.mixed = mixed;
        if mixed {
            self.checked = false;
        }
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn destructive(mut self) -> Self {
        self.destructive = true;
        self
    }
    pub fn needs_input(mut self) -> Self {
        self.needs_input = true;
        self
    }

    /// Label as shown in a menu: ellipsis when more input follows.
    pub fn menu_label(&self) -> String {
        if self.needs_input && !self.label.ends_with('…') {
            format!("{}…", self.label)
        } else {
            self.label.clone()
        }
    }
}

/// Host platform for shortcut display and accelerator policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// Render a GPUI keystroke string for menus and tooltips: Mac symbols on
/// macOS, `Ctrl+Shift+B` elsewhere (handbook 12: "never hardcode an
/// incorrect ⌘ symbol on Windows").
pub fn display_shortcut(keys: &str, platform: Platform) -> String {
    keys.split(' ')
        .map(|stroke| {
            let parts: Vec<&str> = stroke.split('-').collect();
            let (mods, key) = parts.split_at(parts.len().saturating_sub(1));
            let key = key.first().copied().unwrap_or("");
            let key = match key {
                "comma" => ",".to_string(),
                "escape" => {
                    if platform == Platform::Mac {
                        "⎋".into()
                    } else {
                        "Esc".into()
                    }
                }
                "enter" => {
                    if platform == Platform::Mac {
                        "↩".into()
                    } else {
                        "Enter".into()
                    }
                }
                "backspace" => {
                    if platform == Platform::Mac {
                        "⌫".into()
                    } else {
                        "Backspace".into()
                    }
                }
                "delete" => {
                    if platform == Platform::Mac {
                        "⌦".into()
                    } else {
                        "Del".into()
                    }
                }
                "space" => "Space".into(),
                k if k.len() == 1 => k.to_uppercase(),
                k => {
                    let mut c = k.chars();
                    match c.next() {
                        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                        None => String::new(),
                    }
                }
            };
            let mut out = String::new();
            // Mac order: Control, Option, Shift, Command (HIG Appendix A).
            let has = |m: &str| mods.iter().any(|x| x.eq_ignore_ascii_case(m));
            let secondary = has("secondary");
            if platform == Platform::Mac {
                if has("ctrl") {
                    out.push('⌃');
                }
                if has("alt") {
                    out.push('⌥');
                }
                if has("shift") {
                    out.push('⇧');
                }
                if has("cmd") || secondary {
                    out.push('⌘');
                }
                out.push_str(&key);
            } else {
                if has("ctrl") || secondary {
                    out.push_str("Ctrl+");
                }
                if has("alt") {
                    out.push_str("Alt+");
                }
                if has("shift") {
                    out.push_str("Shift+");
                }
                if has("cmd") || has("super") || has("win") {
                    out.push_str("Win+");
                }
                out.push_str(&key);
            }
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A menu entry referencing commands by ID.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    Command(CommandId),
    Separator,
    Submenu {
        label: String,
        entries: Vec<MenuEntry>,
    },
    /// Non-interactive group heading.
    Heading(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct MenuModel {
    pub label: String,
    pub entries: Vec<MenuEntry>,
}

/// A consistent set of descriptors.
#[derive(Clone, Debug, Default)]
pub struct CommandSet {
    commands: BTreeMap<CommandId, CommandDescriptor>,
    /// Bumped on every change so views can cheaply detect staleness.
    pub version: u64,
}

impl CommandSet {
    pub fn new(commands: impl IntoIterator<Item = CommandDescriptor>) -> Self {
        let mut set = Self::default();
        for c in commands {
            set.commands.insert(c.id.clone(), c);
        }
        set
    }
    pub fn get(&self, id: &CommandId) -> Option<&CommandDescriptor> {
        self.commands.get(id)
    }
    pub fn upsert(&mut self, command: CommandDescriptor) {
        if self.commands.get(&command.id) != Some(&command) {
            self.commands.insert(command.id.clone(), command);
            self.version += 1;
        }
    }
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        if let Some(c) = self.commands.get_mut(&CommandId::new(id)) {
            if c.enabled != enabled {
                c.enabled = enabled;
                self.version += 1;
            }
        }
    }
    /// Show a dash for an attribute that applies to part of the selection
    /// (HIG pp. 170-171). Setting it clears `checked`.
    pub fn set_mixed(&mut self, id: &str, mixed: bool) {
        let Some(c) = self.commands.get_mut(&CommandId::new(id)) else {
            return;
        };
        let changed = c.mixed != mixed || (mixed && c.checked);
        c.mixed = mixed;
        if mixed {
            c.checked = false;
        }
        if changed {
            self.version += 1;
        }
    }
    pub fn set_checked(&mut self, id: &str, checked: bool) {
        let id = CommandId::new(id);
        let group = match self.commands.get(&id).map(|c| c.kind.clone()) {
            Some(CommandKind::Radio { group }) if checked => Some(group),
            Some(_) => None,
            None => return,
        };
        let mut changed = false;
        if let Some(group) = group {
            for c in self.commands.values_mut() {
                if matches!(&c.kind, CommandKind::Radio { group: g } if *g == group) {
                    let value = c.id == id;
                    changed |= c.checked != value;
                    c.checked = value;
                }
            }
        }
        if let Some(c) = self.commands.get_mut(&id) {
            changed |= c.checked != checked;
            c.checked = checked;
            if checked && c.mixed {
                c.mixed = false;
                changed = true;
            }
        }
        if changed {
            self.version += 1;
        }
    }

    pub fn remove(&mut self, id: &CommandId) -> Option<CommandDescriptor> {
        let removed = self.commands.remove(id);
        if removed.is_some() {
            self.version += 1;
        }
        removed
    }
    pub fn iter(&self) -> impl Iterator<Item = &CommandDescriptor> {
        self.commands.values()
    }

    /// Shortcuts bound to more than one command (handbook 12: conflict detection).
    pub fn shortcut_conflicts(&self) -> Vec<(Arc<str>, Vec<CommandId>)> {
        let mut by_key: BTreeMap<Arc<str>, Vec<CommandId>> = BTreeMap::new();
        for c in self.commands.values() {
            if let Some(k) = &c.shortcut {
                by_key.entry(k.clone()).or_default().push(c.id.clone());
            }
        }
        by_key
            .into_iter()
            .filter(|(_, ids)| ids.len() > 1)
            .collect()
    }

    /// Menu entries that name commands this set does not define.
    pub fn missing(&self, menus: &[MenuModel]) -> Vec<CommandId> {
        fn walk(set: &CommandSet, entries: &[MenuEntry], out: &mut Vec<CommandId>) {
            for e in entries {
                match e {
                    MenuEntry::Command(id) if set.get(id).is_none() => out.push(id.clone()),
                    MenuEntry::Submenu { entries, .. } => walk(set, entries, out),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        for m in menus {
            walk(self, &m.entries, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_op_checked_updates_do_not_invalidate_and_removal_does() {
        let mut set = CommandSet::new([
            CommandDescriptor::radio("a", "A", "view", true),
            CommandDescriptor::radio("b", "B", "view", false),
            CommandDescriptor::toggle("toggle", "Toggle", false),
        ]);
        set.set_checked("a", true);
        set.set_checked("toggle", false);
        assert_eq!(set.version, 0);
        set.set_checked("b", true);
        assert_eq!(set.version, 1);
        set.set_checked("b", true);
        assert_eq!(set.version, 1);
        assert!(set.remove(&"a".into()).is_some());
        assert_eq!(set.version, 2);
        assert!(set.remove(&"missing".into()).is_none());
        assert_eq!(set.version, 2);
    }

    #[test]
    fn shortcuts_render_per_platform() {
        assert_eq!(display_shortcut("secondary-b", Platform::Mac), "⌘B");
        assert_eq!(display_shortcut("secondary-b", Platform::Windows), "Ctrl+B");
        assert_eq!(display_shortcut("secondary-shift-n", Platform::Mac), "⇧⌘N");
        assert_eq!(
            display_shortcut("secondary-comma", Platform::Linux),
            "Ctrl+,"
        );
        assert_eq!(
            display_shortcut("alt-secondary-escape", Platform::Mac),
            "⌥⌘⎋"
        );
    }

    #[test]
    fn radio_groups_are_exclusive_and_ellipsis_follows_needs_input() {
        let mut set = CommandSet::new([
            CommandDescriptor::radio("view.icons", "as Icons", "view", true),
            CommandDescriptor::radio("view.list", "as List", "view", false),
            CommandDescriptor::action("file.open", "Open")
                .needs_input()
                .shortcut("secondary-o"),
            CommandDescriptor::action("file.other", "Other").shortcut("secondary-o"),
        ]);
        set.set_checked("view.list", true);
        assert!(!set.get(&"view.icons".into()).unwrap().checked);
        assert!(set.get(&"view.list".into()).unwrap().checked);
        assert_eq!(set.get(&"file.open".into()).unwrap().menu_label(), "Open…");
        assert_eq!(set.shortcut_conflicts().len(), 1);
        let menus = [MenuModel {
            label: "File".into(),
            entries: vec![
                MenuEntry::Command("file.open".into()),
                MenuEntry::Command("file.gone".into()),
            ],
        }];
        assert_eq!(set.missing(&menus), vec![CommandId::new("file.gone")]);
    }
}
