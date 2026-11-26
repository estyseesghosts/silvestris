//! Windows host places and filename rules.

use std::path::{Path, PathBuf};

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Documents, FOLDERID_Downloads, FOLDERID_Music, FOLDERID_Pictures,
    FOLDERID_Profile, FOLDERID_Videos, SHFileOperationW, SHGetKnownFolderPath, FOF_ALLOWUNDO,
    FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE, KF_FLAG_DEFAULT, SHFILEOPSTRUCTW,
};

use crate::file_browser::provider::FileError;
use crate::icons::Icon;

use super::HostPlace;
use super::PlaceSection;

pub(super) fn host_places(home: Option<&Path>) -> Vec<HostPlace> {
    let mut out = Vec::new();
    let mut has_home = false;
    for (id, label, icon) in [
        (&FOLDERID_Profile, "Home", Icon::Home),
        (&FOLDERID_Desktop, "Desktop", Icon::Desktop),
        (&FOLDERID_Documents, "Documents", Icon::Documents),
        (&FOLDERID_Downloads, "Downloads", Icon::Documents),
        (&FOLDERID_Pictures, "Pictures", Icon::Documents),
        (&FOLDERID_Music, "Music", Icon::Documents),
        (&FOLDERID_Videos, "Videos", Icon::Documents),
    ] {
        if let Some(path) = known_folder(id) {
            has_home |= id == &FOLDERID_Profile;
            out.push(HostPlace::place(path, label, icon, PlaceSection::Places));
        }
    }
    if !has_home {
        if let Some(home) = home {
            out.insert(
                0,
                HostPlace::place(home.to_path_buf(), "Home", Icon::Home, PlaceSection::Places),
            );
        }
    }
    out.extend(volume_places());
    out
}

fn known_folder(id: &windows::core::GUID) -> Option<PathBuf> {
    // SAFETY: the shell writes a NUL-terminated string into `raw`; it is
    // copied immediately and released with CoTaskMemFree as required.
    unsafe {
        let raw = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None).ok()?;
        if raw.is_null() {
            return None;
        }
        let path = raw.to_string().ok().map(PathBuf::from);
        CoTaskMemFree(Some(raw.0.cast()));
        path
    }
}

fn volume_places() -> Vec<HostPlace> {
    let mut out = Vec::new();
    // SAFETY: GetLogicalDrives has no parameters and returns a bitmask.
    let mask = unsafe { GetLogicalDrives() };
    for index in 0..26 {
        if mask & (1 << index) == 0 {
            continue;
        }
        let letter = (b'A' + index as u8) as char;
        let root = format!("{letter}:\\");
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: `wide` is NUL-terminated and outlives the call.
        let drive_type = unsafe { GetDriveTypeW(PCWSTR(wide.as_ptr())) };
        let (label, icon) = match drive_type {
            2 => ("Removable Disk", Icon::Volume),
            3 => ("Local Disk", Icon::Volume),
            4 => ("Network Drive", Icon::Network),
            5 => ("CD Drive", Icon::Volume),
            6 => ("RAM Disk", Icon::Volume),
            0 | 1 => continue,
            _ => ("Drive", Icon::Volume),
        };
        out.push(HostPlace::place(
            PathBuf::from(root),
            format!("{label} ({letter}:)"),
            icon,
            PlaceSection::Devices,
        ));
    }
    out
}

const INVALID_CHARS: &[char] = &['<', '>', ':', '"', '/', '\\', '|', '?', '*'];
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

pub(super) fn validate_name(name: &str) -> Result<(), FileError> {
    if name.encode_utf16().count() > 255 {
        return Err(FileError::new("The name is too long."));
    }
    if name
        .chars()
        .any(|character| character.is_control() || INVALID_CHARS.contains(&character))
    {
        return Err(FileError::new(
            "Names cannot contain < > : \" / \\ | ? * or control characters.",
        ));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Err(FileError::new("Names cannot end with a space or a period."));
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    if RESERVED_NAMES
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(FileError::new(format!(
            "\u{201C}{name}\u{201D} is a reserved Windows device name."
        )));
    }
    Ok(())
}

pub(super) fn move_to_trash(path: &Path, _home: Option<&Path>) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.extend([0, 0]);
    let flags = FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0;
    let mut operation = SHFILEOPSTRUCTW {
        hwnd: HWND::default(),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(wide.as_ptr()),
        pTo: PCWSTR::null(),
        fFlags: flags as u16,
        fAnyOperationsAborted: windows::core::BOOL(0),
        hNameMappings: std::ptr::null_mut(),
        lpszProgressTitle: PCWSTR::null(),
    };
    // SAFETY: `wide` is double-NUL-terminated and alive for the call; the
    // struct matches the shell API and has no other borrowed pointers.
    let result = unsafe { SHFileOperationW(&mut operation) };
    if result != 0 {
        return Err(std::io::Error::other(format!(
            "the shell could not move the item to the Recycle Bin (code {result:#x})"
        )));
    }
    if operation.fAnyOperationsAborted.as_bool() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Interrupted,
            "the move to the Recycle Bin was cancelled",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_name;

    #[test]
    fn windows_reserved_names_and_trailing_characters_are_rejected() {
        for name in [
            "CON",
            "con.txt",
            "LPT9.log",
            "bad<name",
            "bad\\name",
            "trail.",
            "trail ",
        ] {
            assert!(validate_name(name).is_err(), "{name}");
        }
        for name in ["report.txt", "report 2024.txt", "a.b.c"] {
            assert!(validate_name(name).is_ok(), "{name}");
        }
    }
}
