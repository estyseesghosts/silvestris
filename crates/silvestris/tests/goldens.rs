//! Golden files must match the accepted manifest and carry a reason.
//!
//! Changing a PNG without `gate.py --accept --reason ...` leaves the manifest
//! stale, so this test fails before the change can be mistaken for accepted
//! output.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

fn goldens_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../goldens")
}

fn pngs(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read golden directory") {
        let path = entry.expect("golden entry").path();
        if path.is_dir() {
            pngs(&path, found);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("png") {
            found.push(path);
        }
    }
}

fn hash(path: &Path) -> String {
    let bytes = fs::read(path).expect("read golden");
    let mut digest = Sha256::new();
    digest.update(&bytes);
    format!("{:x}", digest.finalize())
}

#[test]
fn every_golden_matches_the_accepted_manifest() {
    let root = goldens_root();
    let manifest: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string(root.join("manifest.json")).expect("read goldens/manifest.json"),
    )
    .expect("parse goldens/manifest.json");
    let reasons: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string(root.join("reasons.json")).expect("read goldens/reasons.json"),
    )
    .expect("parse goldens/reasons.json");

    let mut files = Vec::new();
    pngs(&root.join("renders"), &mut files);
    assert!(!files.is_empty(), "no goldens under goldens/renders");

    let mut seen = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(root.join("renders"))
            .expect("golden under renders")
            .to_string_lossy()
            .replace('\\', "/");
        seen.push(relative.clone());
        let accepted = manifest.get(&relative).unwrap_or_else(|| {
            panic!("golden {relative} is not in manifest.json; accept it with gate.py --accept")
        });
        assert_eq!(
            &hash(&path),
            accepted,
            "golden {relative} differs from manifest.json; use gate.py --accept --reason ..."
        );
        let reason = reasons.get(&relative).map(String::as_str).unwrap_or("");
        assert!(
            !reason.trim().is_empty(),
            "golden {relative} has no reason in reasons.json"
        );
    }
    seen.sort();
    let mut accepted: Vec<String> = manifest.keys().cloned().collect();
    accepted.sort();
    assert_eq!(
        seen, accepted,
        "manifest.json lists goldens that are missing from goldens/renders"
    );
}
