//! Generic Unix host places and Trash support.

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
        ] {
            let path = home.join(folder);
            if path.is_dir() {
                out.push(HostPlace::place(path, folder, icon, PlaceSection::Places));
            }
        }
    }
    out.push(HostPlace::place(
        PathBuf::from("/"),
        "File System",
        Icon::Volume,
        PlaceSection::Devices,
    ));
    out
}

pub(super) fn validate_name(name: &str) -> Result<(), FileError> {
    if name.len() > 255 {
        return Err(FileError::new("The name is too long."));
    }
    if name.contains('/') || name.contains('\0') {
        return Err(FileError::new("Names cannot contain a slash or NUL."));
    }
    Ok(())
}

pub(super) fn move_to_trash(path: &Path, home: Option<&Path>) -> io::Result<()> {
    let home = home
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "the home directory is unknown"))?;
    let source = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let trash = home.join(".Trash");
    std::fs::create_dir_all(&trash)?;
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
