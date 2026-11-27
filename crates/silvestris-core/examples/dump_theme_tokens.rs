//! Print a resolved theme's token snapshot for fixture review.
//!
//! Used when an Aqua fixture must be regenerated after an authorized shared
//! token change. The output is a baseline record, not a test oracle.

#[path = "../tests/support/snapshot.rs"]
mod snapshot;

use silvestris_core::{theme, ThemeId};
use snapshot::Snapshot;

fn main() {
    let id = match std::env::args().nth(1).as_deref() {
        Some("aqua-light") => ThemeId::AQUA_LIGHT,
        Some("aqua-dark") => ThemeId::AQUA_DARK,
        Some("geist-light") => ThemeId::GEIST_LIGHT,
        Some("geist-dark") => ThemeId::GEIST_DARK,
        _ => panic!("expected aqua-light, aqua-dark, geist-light or geist-dark"),
    };
    let theme = theme(id);
    println!("{:#?}", Snapshot::from(&theme));
}
