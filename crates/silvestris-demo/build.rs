//! windows-gnu only: keep GPUI's manifest as the executable's only RT_MANIFEST.
//!
//! gpui_platform enables gpui's `windows-manifest` feature, which embeds ID 1 (PerMonitorV2 DPI
//! awareness, Common Controls v6). The MinGW GCC driver also links
//! `%{!shared:%:if-exists(default-manifest.o%s)}` (`gcc -dumpspecs`). WinLibs ships that object
//! with language 1033 instead of 0, so ld cannot discard it as the default and warns
//! ".rsrc merge failure: multiple non-default manifests". The result had two ID 1 / 1033 manifests,
//! leaving it undefined which one Windows applies. `%s` searches `-B` prefixes first, so an empty
//! COFF object of that name here replaces the toolchain's.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.ends_with("-windows-gnu") {
        return;
    }
    let machine: u16 = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => 0x8664,
        Ok("aarch64") => 0xaa64,
        Ok("x86") => 0x14c,
        _ => return,
    };
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    // IMAGE_FILE_HEADER with no sections and no symbols: Machine, then 18 zero bytes.
    let mut object = machine.to_le_bytes().to_vec();
    object.resize(20, 0);
    std::fs::write(out.join("default-manifest.o"), object).expect("write default-manifest.o");
    // A trailing separator makes GCC treat the prefix as a directory.
    println!("cargo:rustc-link-arg-bins=-B{}/", out.display());
}
