//! Linux host places, filename rules and freedesktop trash support.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::file_panel::provider::FileError;
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
        for (path, icon) in xdg_user_dirs(home) {
            if path == home {
                continue;
            }
            let label = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| path.display().to_string());
            out.push(HostPlace::place(path, label, icon, PlaceSection::Places));
        }
    }
    out.push(HostPlace::place(
        PathBuf::from("/"),
        "File System",
        Icon::Volume,
        PlaceSection::Devices,
    ));
    for mount in removable_mounts() {
        out.push(
            HostPlace::place(
                mount.clone(),
                mount
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| mount.display().to_string()),
                Icon::Volume,
                PlaceSection::Devices,
            )
            .eject(true),
        );
    }
    out
}

fn xdg_user_dirs(home: &Path) -> Vec<(PathBuf, Icon)> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let file = config.join("user-dirs.dirs");
    let mut configured = HashMap::new();
    let present = match std::fs::read_to_string(&file) {
        Ok(text) => {
            parse_user_dirs(&text, home, &mut configured);
            true
        }
        Err(_) => false,
    };
    if !present {
        for (key, folder) in [
            ("XDG_DESKTOP_DIR", "Desktop"),
            ("XDG_DOCUMENTS_DIR", "Documents"),
            ("XDG_DOWNLOAD_DIR", "Downloads"),
            ("XDG_PICTURES_DIR", "Pictures"),
        ] {
            configured.insert(key.to_string(), home.join(folder));
        }
    }
    [
        ("XDG_DESKTOP_DIR", Icon::Desktop),
        ("XDG_DOCUMENTS_DIR", Icon::Documents),
        ("XDG_DOWNLOAD_DIR", Icon::Documents),
        ("XDG_PICTURES_DIR", Icon::Documents),
    ]
    .into_iter()
    .filter_map(|(key, icon)| configured.remove(key).map(|path| (path, icon)))
    .collect()
}

fn parse_user_dirs(text: &str, home: &Path, out: &mut HashMap<String, PathBuf>) {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some(value) = unquote(value.trim()) else {
            continue;
        };
        let Some(expanded) = expand_home(&value, home) else {
            continue;
        };
        out.insert(key.trim().to_string(), expanded);
    }
}

fn unquote(value: &str) -> Option<String> {
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '\\' {
            match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                '$' => out.push('$'),
                other => {
                    out.push('\\');
                    out.push(other);
                }
            }
        } else {
            out.push(character);
        }
    }
    Some(out)
}

fn expand_home(value: &str, home: &Path) -> Option<PathBuf> {
    if let Some(rest) = value.strip_prefix("$HOME") {
        return Some(home.join(rest.trim_start_matches('/')));
    }
    if let Some(rest) = value.strip_prefix("${HOME}") {
        return Some(home.join(rest.trim_start_matches('/')));
    }
    let path = PathBuf::from(value);
    path.is_absolute().then_some(path)
}

fn removable_mounts() -> Vec<PathBuf> {
    let Ok(mounts) = std::fs::read_to_string("/proc/mounts") else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = Vec::new();
    for line in mounts.lines() {
        let mut fields = line.split_whitespace();
        let _device = fields.next();
        let Some(point) = fields.next() else {
            continue;
        };
        let point = PathBuf::from(unescape_mount(point));
        let removable = ["/media/", "/run/media/", "/mnt/"]
            .iter()
            .any(|prefix| point.starts_with(prefix));
        if removable && !out.contains(&point) {
            out.push(point);
        }
    }
    out
}

fn unescape_mount(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\'
            && index + 3 < bytes.len()
            && bytes[index + 1..index + 4].iter().all(u8::is_ascii_digit)
        {
            let octal = std::str::from_utf8(&bytes[index + 1..index + 4]).unwrap_or("0");
            if let Ok(byte) = u8::from_str_radix(octal, 8) {
                out.push(byte);
                index += 4;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
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
    let source = absolute(path);
    let metadata = std::fs::metadata(&source)?;
    let home_device = std::fs::metadata(home)
        .map(|metadata| metadata.dev())
        .unwrap_or(u64::MAX);
    let trash_root = if metadata.dev() == home_device {
        data_home(home).join("Trash")
    } else {
        mount_top(&source)?.join(format!(".Trash-{}", uid()))
    };
    move_into(&source, &trash_root)
}

fn data_home(home: &Path) -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".local/share"))
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

fn move_into(source: &Path, trash_root: &Path) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;

    let files = trash_root.join("files");
    let info = trash_root.join("info");
    std::fs::create_dir_all(&files)?;
    std::fs::create_dir_all(&info)?;

    let original_name = source
        .file_name()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| OsStr::new("trashed"));
    let mut candidate = original_name.to_os_string();
    let mut counter = 1;
    loop {
        let target = files.join(&candidate);
        let mut info_name = candidate.clone();
        info_name.push(".trashinfo");
        let info_path = info.join(info_name);
        if !target.try_exists()? && !info_path.try_exists()? {
            break;
        }
        let mut next = original_name.to_os_string();
        next.push(format!(".{counter}"));
        candidate = next;
        counter += 1;
    }

    let target = files.join(&candidate);
    std::fs::rename(source, &target)?;

    let mut info_name = candidate.clone();
    info_name.push(".trashinfo");
    let info_path = info.join(info_name);
    let contents = format!(
        "[Trash Info]\nPath={}\nDeletionDate={}\n",
        percent_encode(source.as_os_str().as_bytes()),
        timestamp()
    );
    if let Err(error) = std::fs::write(&info_path, contents) {
        let _ = std::fs::rename(&target, source);
        return Err(error);
    }
    Ok(())
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

fn percent_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    for &byte in bytes {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/');
        if keep {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn timestamp() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let (year, month, day, hour, minute, second) = civil(seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")
}

fn civil(epoch: u64) -> (i64, u32, u32, u32, u32, u32) {
    let seconds = (epoch % 86_400) as u32;
    let days = (epoch / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (
        year,
        m,
        d,
        seconds / 3600,
        (seconds % 3600) / 60,
        seconds % 60,
    )
}

#[cfg(test)]
mod tests {
    use super::{move_into, parse_user_dirs, percent_encode, unescape_mount, validate_name};
    use std::collections::HashMap;
    use std::path::Path;

    #[test]
    fn unix_names_allow_colons_and_reject_slashes() {
        assert!(validate_name("report:2024.txt").is_ok());
        assert!(validate_name("back\\slash.txt").is_ok());
        assert!(validate_name("bad/name").is_err());
        assert!(validate_name("bad\0name").is_err());
    }

    #[test]
    fn user_dirs_configuration_is_expanded_not_assumed() {
        let mut dirs = HashMap::new();
        parse_user_dirs(
            "XDG_DOCUMENTS_DIR=\"$HOME/Belgeler\"\nXDG_DESKTOP_DIR=\"/data/desk\"\n",
            Path::new("/home/test"),
            &mut dirs,
        );
        assert_eq!(dirs["XDG_DOCUMENTS_DIR"], Path::new("/home/test/Belgeler"));
        assert_eq!(dirs["XDG_DESKTOP_DIR"], Path::new("/data/desk"));
    }

    #[test]
    fn trash_paths_are_url_encoded() {
        assert_eq!(percent_encode(b"/home/a b/c"), "/home/a%20b/c");
        assert_eq!(percent_encode(b"/home/caf\xc3\xa9"), "/home/caf%C3%A9");
    }

    #[test]
    fn mount_escapes_are_decoded() {
        assert_eq!(unescape_mount("/media/My\\040Disk"), "/media/My Disk");
    }

    #[test]
    fn a_file_is_moved_with_a_trashinfo_record() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let root =
            std::env::temp_dir().join(format!("silvestris-trash-{}-{nanos}", std::process::id()));
        let trash = root.join("Trash");
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("note.txt");
        std::fs::write(&source, b"hello").unwrap();

        move_into(&source, &trash).unwrap();
        assert!(!source.exists(), "the original path is gone");
        assert!(trash.join("files").join("note.txt").exists());
        let info = std::fs::read_to_string(trash.join("info").join("note.txt.trashinfo"))
            .expect("trash info");
        assert!(info.starts_with("[Trash Info]\n"));
        assert!(info.contains("Path="));
        assert!(info.contains("DeletionDate="));

        let second = root.join("note.txt");
        std::fs::write(&second, b"again").unwrap();
        move_into(&second, &trash).unwrap();
        assert!(trash.join("files").join("note.txt.1").exists());

        std::fs::remove_dir_all(root).ok();
    }
}
