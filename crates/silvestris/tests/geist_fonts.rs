//! Geist Sans provenance and registration metadata.
//!
//! Headless GPUI tests use a no-op text system, so they cannot prove that the
//! platform selected a face. The native capture run in `docs/geist-refinement.md`
//! is the registration check; these tests prove the bytes, names and manifest
//! that the native path receives.

#[test]
fn geist_sans_faces_have_the_real_internal_family_and_all_required_weights() {
    let manifest: serde_json::Value =
        serde_json::from_str(silvestris_fonts::MANIFEST).expect("parse bundled font manifest");
    let families = manifest["families"].as_array().expect("family list");
    let geist = families
        .iter()
        .find(|family| family["dir"] == "geist-sans")
        .expect("geist-sans family");
    assert_eq!(geist["release"], "v1.9.2");
    assert_eq!(
        geist["archive_sha256"],
        "15f066d58dcb99fb10014a49913009fdaf41e464cfb8851a44b44e593c95d3d8"
    );
    assert_eq!(geist["license"], "geist-sans/OFL.txt");
    let faces = geist["faces"].as_array().expect("geist-sans faces");
    let mut actual: Vec<(u16, bool)> = Vec::new();
    for face in faces {
        assert_eq!(
            face["family"], "Geist",
            "typographic family in {}",
            face["file"]
        );
        assert_eq!(face["variable"], false, "static face required");
        actual.push((
            face["weight"].as_u64().expect("weight") as u16,
            face["italic"].as_bool().expect("italic"),
        ));
    }
    actual.sort_unstable();
    assert_eq!(
        actual,
        vec![
            (400, false),
            (400, true),
            (500, false),
            (600, false),
            (700, false)
        ]
    );
}

#[test]
fn bundled_families_keep_their_declared_weights_and_shared_names() {
    assert_eq!(
        silvestris_core::fonts::GEIST_SANS,
        silvestris_fonts::GEIST_SANS
    );
    assert_eq!(
        silvestris_core::fonts::GEIST_MONO,
        silvestris_fonts::GEIST_MONO
    );
    let weights: Vec<(u16, bool)> = silvestris_fonts::FACES
        .iter()
        .filter(|face| face.family == silvestris_fonts::GEIST_SANS)
        .map(|face| (face.weight, face.italic))
        .collect();
    assert_eq!(
        weights,
        vec![
            (400, false),
            (400, true),
            (500, false),
            (600, false),
            (700, false)
        ]
    );
    // Inter remains an auxiliary family; Aqua's faces are untouched.
    assert!(silvestris_fonts::FAMILIES.contains(&silvestris_fonts::INTER));
    assert!(silvestris_fonts::FACES
        .iter()
        .any(|face| face.family == silvestris_fonts::LATO));
    assert!(silvestris_fonts::FACES
        .iter()
        .any(|face| face.family == silvestris_fonts::SOURCE_CODE_PRO));
}

#[test]
fn geist_resolves_to_geist_sans_and_not_inter() {
    let theme = silvestris_core::theme(silvestris_core::ThemeId::GEIST_LIGHT);
    assert_eq!(theme.type_.ui, silvestris_core::fonts::GEIST_SANS);
    assert_eq!(theme.type_.mono, silvestris_core::fonts::GEIST_MONO);
    let aqua = silvestris_core::theme(silvestris_core::ThemeId::AQUA_LIGHT);
    assert_eq!(aqua.type_.ui, silvestris_core::fonts::LATO);
    assert_eq!(aqua.type_.mono, silvestris_core::fonts::SOURCE_CODE_PRO);
}
