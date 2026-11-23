//! The coverage matrix must name every public control type.
//!
//! A new control without a row is a documentation bug: consumers cannot tell
//! whether Silvestris provides the component they need.

use std::fs;
use std::path::Path;

#[test]
fn every_public_control_has_a_coverage_row() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let controls = root.join("src/controls");
    let matrix =
        fs::read_to_string(root.join("../../docs/coverage.md")).expect("read docs/coverage.md");

    let mut missing = Vec::new();
    for entry in fs::read_dir(&controls).expect("read src/controls") {
        let path = entry.expect("directory entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("read control source");
        for name in public_types(&source) {
            if !matrix.contains(&format!("`{name}`")) {
                missing.push(format!("{}: {name}", path.display()));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "public control types without a docs/coverage.md row: {missing:#?}"
    );
}

fn public_types(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in source.lines() {
        let line = line.trim_start();
        let rest = if let Some(rest) = line.strip_prefix("pub struct ") {
            rest
        } else if let Some(rest) = line.strip_prefix("pub enum ") {
            rest
        } else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|ch| ch.is_alphanumeric() || *ch == '_')
            .collect();
        if !name.is_empty() {
            names.push(name);
        }
    }
    names
}
