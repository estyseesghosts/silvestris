use std::fs;
use std::path::{Path, PathBuf};

/// A control must not draw a one-device line by hand. Use `paint::hline`,
/// `paint::vline`, `paint::snap_edge` or `convert::hairline`; those apply the
/// documented device-grid rule. This scans the source because Rust cannot see
/// through the builder expression.
#[test]
fn controls_do_not_draw_a_one_device_line_by_hand() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut failures = Vec::new();
    visit(&root, &mut failures);
    assert!(
        failures.is_empty(),
        "one-device line without the hairline helper:\n{}",
        failures.join("\n")
    );
}

fn visit(directory: &Path, failures: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("read source directory") {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            visit(&path, failures);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if matches!(name, "paint.rs" | "convert.rs") {
            continue;
        }
        let text = fs::read_to_string(&path).expect("read source file");
        for (index, line) in text.lines().enumerate() {
            let hard_coded = [".h(px(1.", ".w(px(1.", "thickness: px(1.", "stroke(px(1."]
                .iter()
                .any(|needle| line.contains(needle));
            if hard_coded && !line.contains("hairline") {
                failures.push(format!("{}:{}: {}", path.display(), index + 1, line.trim()));
            }
        }
    }
}
