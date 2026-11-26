//! macOS host places, filename rules and Trash support.

use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use crate::file_browser::provider::FileError;
use crate::icons::Icon;

use super::HostPlace;
use super::PlaceSection;

pub(super) fn host_places(home: Option<&Path>) -> Vec<HostPlace> {
    let mut out = Vec::new();
    if let Some(home) = home {
        out.push(HostPlace::place(
            home.to_path_buf(),
            "Home",
            Icon::Home,
            PlaceSection::Places,
        ));
        for (folder, icon) in [
            ("Desktop", Icon::Desktop),
            ("Documents", Icon::Documents),
            ("Downloads", Icon::Documents),
            ("Pictures", Icon::Documents),
        ] {
            let path = home.join(folder);
            if path.is_dir() {
                out.push(HostPlace::place(path, folder, icon, PlaceSection::Places));
            }
        }
        let applications = PathBuf::from("/Applications");
        if applications.is_dir() {
            out.push(HostPlace::place(
                applications,
                "Applications",
                Icon::Application,
                PlaceSection::Places,
            ));
        }
    }
    out.push(HostPlace::place(
        PathBuf::from("/"),
        "Macintosh HD",
        Icon::Volume,
        PlaceSection::Devices,
    ));
    if let Ok(entries) = std::fs::read_dir("/Volumes") {
        for entry in entries.flatten() {
            let path = entry.path();
            let label = entry.file_name().to_string_lossy().into_owned();
            out.push(HostPlace::place(
                path,
                label,
                Icon::Volume,
                PlaceSection::Devices,
            ));
        }
    }
    out
}

pub(super) fn validate_name(name: &str) -> Result<(), FileError> {
    if name.len() > 255 {
        return Err(FileError::new("The name is too long."));
    }
    if name.contains('/') || name.contains(':') || name.contains('\0') {
        return Err(FileError::new(
            "Names cannot contain a slash, a colon or NUL.",
        ));
    }
    Ok(())
}

pub(super) fn move_to_trash(path: &Path, home: Option<&Path>) -> io::Result<()> {
    let home = home
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the home directory is unknown"))?;
    let source = absolute(path);
    match move_into(&source, &home.join(".Trash")) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::CrossesDevices => {
            let volume = mount_top(&source)?;
            let trash = volume.join(".Trashes").join(uid().to_string());
            std::fs::create_dir_all(&trash)?;
            move_into(&source, &trash)
        }
        Err(error) => Err(error),
    }
}

fn move_into(source: &Path, trash: &Path) -> io::Result<()> {
    std::fs::create_dir_all(trash)?;
    let original = source
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| OsStr::new("trashed"));
    let mut candidate = original.to_os_string();
    let mut counter = 1;
    while trash.join(&candidate).try_exists()? {
        let mut next = original.to_os_string();
        next.push(format!(".{counter}"));
        candidate = next;
        counter += 1;
    }
    std::fs::rename(source, trash.join(candidate))
}

fn mount_top(path: &Path) -> io::Result<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let device = std::fs::metadata(path)?.dev();
    let mut top = path.parent().unwrap_or(path).to_path_buf();
    while let Some(parent) = top.parent() {
        match std::fs::metadata(parent) {
            Ok(metadata) if metadata.dev() == device => top = parent.to_path_buf(),
            _ => break,
        }
    }
    Ok(top)
}

fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and does not fail.
    unsafe { libc::getuid() }
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn finder_names_reject_colons() {
        assert!(validate_name("report:2024.txt").is_err());
        assert!(validate_name("report 2024.txt").is_ok());
    }
}
