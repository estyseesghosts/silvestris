//! Font bytes for the four Silvestris themes, compiled into the binary.
//!
//! Aqua uses Lato and Source Code Pro; Geist uses Geist Sans and Geist Mono,
//! with Inter retained as an auxiliary family (handbook 11 "Exact typography
//! requirements"). Every face here is listed,
//! with release, archive and file SHA-256, internal names and license, in
//! `fonts/manifest.json`, which `tools/fonts/manifest.py --check` verifies.
//! Nothing is downloaded at runtime.

/// One vendored face.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    /// Typographic family (name ID 16, else 1) that GPUI should resolve.
    pub family: &'static str,
    /// Path under `fonts/`.
    pub file: &'static str,
    /// OS/2 usWeightClass.
    pub weight: u16,
    pub italic: bool,
    pub bytes: &'static [u8],
}

macro_rules! face {
    ($family:expr, $file:literal, $weight:expr, $italic:expr) => {
        Face {
            family: $family,
            file: $file,
            weight: $weight,
            italic: $italic,
            bytes: include_bytes!(concat!("../fonts/", $file)),
        }
    };
}

pub const LATO: &str = "Lato";
pub const SOURCE_CODE_PRO: &str = "Source Code Pro";
pub const INTER: &str = "Inter";
/// Internal typographic family (name ID 16) of the static Geist TTFs.
pub const GEIST_SANS: &str = "Geist";
pub const GEIST_MONO: &str = "Geist Mono";

/// Every vendored face. Weights are those the themes use: regular and bold
/// for Aqua (10.6 uses regular labels and bold headings, hig-09 text.md),
/// plus 500/600 for Geist (vercel.md: medium buttons, semibold headings).
pub static FACES: &[Face] = &[
    face!(LATO, "lato/Lato-Regular.ttf", 400, false),
    face!(LATO, "lato/Lato-Italic.ttf", 400, true),
    face!(LATO, "lato/Lato-Bold.ttf", 700, false),
    face!(LATO, "lato/Lato-BoldItalic.ttf", 700, true),
    face!(
        SOURCE_CODE_PRO,
        "source-code-pro/SourceCodePro-Regular.ttf",
        400,
        false
    ),
    face!(
        SOURCE_CODE_PRO,
        "source-code-pro/SourceCodePro-It.ttf",
        400,
        true
    ),
    face!(
        SOURCE_CODE_PRO,
        "source-code-pro/SourceCodePro-Bold.ttf",
        700,
        false
    ),
    face!(INTER, "inter/Inter-Regular.ttf", 400, false),
    face!(INTER, "inter/Inter-Italic.ttf", 400, true),
    face!(INTER, "inter/Inter-Medium.ttf", 500, false),
    face!(INTER, "inter/Inter-SemiBold.ttf", 600, false),
    face!(INTER, "inter/Inter-Bold.ttf", 700, false),
    // Static faces from the same pinned v1.9.2 release as Geist Mono. The
    // Medium and SemiBold files carry legacy family names, but their typographic
    // family is "Geist"; name ID 16 is what GPUI resolves for weight selection.
    face!(GEIST_SANS, "geist-sans/Geist-Regular.ttf", 400, false),
    face!(GEIST_SANS, "geist-sans/Geist-Italic.ttf", 400, true),
    face!(GEIST_SANS, "geist-sans/Geist-Medium.ttf", 500, false),
    face!(GEIST_SANS, "geist-sans/Geist-SemiBold.ttf", 600, false),
    face!(GEIST_SANS, "geist-sans/Geist-Bold.ttf", 700, false),
    face!(GEIST_MONO, "geist-mono/GeistMono-Regular.ttf", 400, false),
    face!(GEIST_MONO, "geist-mono/GeistMono-Medium.ttf", 500, false),
    face!(GEIST_MONO, "geist-mono/GeistMono-Bold.ttf", 700, false),
];

/// Families a consumer must find registered before opening a window.
pub const FAMILIES: [&str; 5] = [
    LATO,
    SOURCE_CODE_PRO,
    INTER,
    GEIST_SANS,
    GEIST_MONO,
];

/// Machine-readable provenance for About/Licenses screens and audits.
pub const MANIFEST: &str = include_str!("../fonts/manifest.json");

/// License texts to ship next to binaries (handbook 11 "Package
/// attribution/licenses next to binaries").
pub static LICENSES: &[(&str, &str)] = &[
    ("Lato", include_str!("../fonts/lato/OFL.txt")),
    (
        "Source Code Pro",
        include_str!("../fonts/source-code-pro/LICENSE.md"),
    ),
    ("Inter", include_str!("../fonts/inter/LICENSE.txt")),
    ("Geist", include_str!("../fonts/geist-sans/OFL.txt")),
    ("Geist Mono", include_str!("../fonts/geist-mono/OFL.txt")),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_is_a_truetype_file_listed_in_the_manifest() {
        for f in FACES {
            assert_eq!(&f.bytes[..4], &[0, 1, 0, 0], "{} is not TrueType", f.file);
            assert!(
                MANIFEST.contains(&format!("\"file\": \"{}\"", f.file)),
                "{} missing",
                f.file
            );
        }
        let listed = MANIFEST.matches("\"file\": ").count();
        assert_eq!(
            listed,
            FACES.len(),
            "manifest lists faces that are not compiled in"
        );
    }

    #[test]
    fn every_family_has_regular_and_bold_and_a_license() {
        for fam in FAMILIES {
            for w in [400, 700] {
                assert!(
                    FACES
                        .iter()
                        .any(|f| f.family == fam && f.weight == w && !f.italic),
                    "{fam} {w}"
                );
            }
            let (_, text) = LICENSES.iter().find(|(n, _)| *n == fam).unwrap();
            assert!(text.contains("SIL Open Font License"), "{fam}");
        }
    }
}
