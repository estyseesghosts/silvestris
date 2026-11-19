//! Theme identity: two independent dimensions, family and appearance (#121).

use serde::{Deserialize, Serialize};

/// Drawing family. Both families render the same shell and controls; only
/// materials and metrics differ (handbook 03 "Layout is app policy, not style
/// policy").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeFamily {
    /// Mac OS X 10.6 Snow Leopard Aqua, measured from 10.6 captures.
    Aqua,
    /// Geist, the neutral palette Lotus shipped before Silvestris.
    Geist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Appearance {
    Light,
    Dark,
}

/// A resolved family/appearance pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ThemeId {
    pub family: ThemeFamily,
    pub appearance: Appearance,
}

impl ThemeId {
    pub const AQUA_LIGHT: ThemeId = ThemeId::new(ThemeFamily::Aqua, Appearance::Light);
    pub const AQUA_DARK: ThemeId = ThemeId::new(ThemeFamily::Aqua, Appearance::Dark);
    pub const GEIST_LIGHT: ThemeId = ThemeId::new(ThemeFamily::Geist, Appearance::Light);
    pub const GEIST_DARK: ThemeId = ThemeId::new(ThemeFamily::Geist, Appearance::Dark);
    pub const ALL: [ThemeId; 4] = [
        Self::AQUA_LIGHT,
        Self::AQUA_DARK,
        Self::GEIST_LIGHT,
        Self::GEIST_DARK,
    ];

    pub const fn new(family: ThemeFamily, appearance: Appearance) -> Self {
        Self { family, appearance }
    }

    pub fn label(self) -> &'static str {
        match (self.family, self.appearance) {
            (ThemeFamily::Aqua, Appearance::Light) => "Aqua",
            (ThemeFamily::Aqua, Appearance::Dark) => "Dark Aqua",
            (ThemeFamily::Geist, Appearance::Light) => "Geist Light",
            (ThemeFamily::Geist, Appearance::Dark) => "Geist Dark",
        }
    }
}

/// What the user asked for. `appearance: None` means "follow the system"
/// (#121: system preference is a possible mode; an explicit choice wins).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemePreference {
    pub family: ThemeFamily,
    #[serde(default)]
    pub appearance: Option<Appearance>,
}

impl ThemePreference {
    pub const fn explicit(id: ThemeId) -> Self {
        Self {
            family: id.family,
            appearance: Some(id.appearance),
        }
    }

    /// Resolve against the platform's current appearance.
    pub fn resolve(self, system: Appearance) -> ThemeId {
        ThemeId::new(self.family, self.appearance.unwrap_or(system))
    }
}

impl Default for ThemePreference {
    /// Aqua Light. Consumers that must keep an older default (Lotus keeps
    /// Geist Dark for existing users until #138's cutover) pass their own.
    fn default() -> Self {
        Self::explicit(ThemeId::AQUA_LIGHT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_choice_beats_system() {
        let p = ThemePreference::explicit(ThemeId::GEIST_LIGHT);
        assert_eq!(p.resolve(Appearance::Dark), ThemeId::GEIST_LIGHT);
        let follow = ThemePreference {
            family: ThemeFamily::Aqua,
            appearance: None,
        };
        assert_eq!(follow.resolve(Appearance::Dark), ThemeId::AQUA_DARK);
    }

    #[test]
    fn preference_serialises_with_stable_names() {
        let json = serde_json::to_string(&ThemePreference::explicit(ThemeId::AQUA_DARK)).unwrap();
        assert_eq!(json, r#"{"family":"Aqua","appearance":"Dark"}"#);
        let old: ThemePreference = serde_json::from_str(r#"{"family":"Geist"}"#).unwrap();
        assert_eq!(old.appearance, None);
    }
}
