//! Curated token snapshot used by the fixture tests and the dump example.
//!
//! Window-control sprite rasters are excluded: they are measured assets, not
//! colour or metric tokens.

use silvestris_core::tokens::{
    ChromeSet, Colors, Controls, Lists, Metrics, Provenance, ResizeGrip, Typography,
};
use silvestris_core::{Theme, ThemeId};

// The fields are read by the derived Debug output, not by name.
#[allow(dead_code)]
#[derive(Debug)]
pub struct Snapshot<'a> {
    pub id: ThemeId,
    pub provenance: Provenance,
    pub typography: &'a Typography,
    pub colors: &'a Colors,
    pub metrics: &'a Metrics,
    pub chrome: &'a ChromeSet,
    pub controls: &'a Controls,
    pub lists: &'a Lists,
    pub grip: &'a ResizeGrip,
}

impl<'a> From<&'a Theme> for Snapshot<'a> {
    fn from(value: &'a Theme) -> Self {
        Snapshot {
            id: value.id,
            provenance: value.provenance,
            typography: &value.type_,
            colors: &value.colors,
            metrics: &value.metrics,
            chrome: &value.chrome,
            controls: &value.controls,
            lists: &value.lists,
            grip: &value.grip,
        }
    }
}
