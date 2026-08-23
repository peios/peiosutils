use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Component, Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub mount_point: PathBuf,
    pub fstype: Vec<u8>,
    pub vfs_options: Vec<u8>,
    pub super_options: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stratum {
    pub path: PathBuf,
    pub create: bool,
    pub read_only: bool,
    pub allow_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrataMount {
    pub mount_point: PathBuf,
    pub read_only: bool,
    pub strata: Vec<Stratum>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Regular,
    Directory,
    Symlink,
    Fifo,
    Socket,
    BlockDevice,
    CharDevice,
    Unknown,
}

impl std::fmt::Display for FileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Regular => "regular file",
            Self::Directory => "directory",
            Self::Symlink => "symbolic link",
            Self::Fifo => "FIFO",
            Self::Socket => "socket",
            Self::BlockDevice => "block device",
            Self::CharDevice => "character device",
            Self::Unknown => "unknown",
        })
    }
}

pub fn file_kind(metadata: &std::fs::Metadata) -> FileKind {
    use std::os::unix::fs::FileTypeExt;

    let kind = metadata.file_type();
    if kind.is_file() {
        FileKind::Regular
    } else if kind.is_dir() {
        FileKind::Directory
    } else if kind.is_symlink() {
        FileKind::Symlink
    } else if kind.is_fifo() {
        FileKind::Fifo
    } else if kind.is_socket() {
        FileKind::Socket
    } else if kind.is_block_device() {
        FileKind::BlockDevice
    } else if kind.is_char_device() {
        FileKind::CharDevice
    } else {
        FileKind::Unknown
    }
}

pub fn read_mountinfo() -> Result<Vec<MountEntry>> {
    let bytes = std::fs::read("/proc/self/mountinfo")
        .map_err(|error| Error::io("read /proc/self/mountinfo", error))?;
    parse_mountinfo(&bytes)
}

pub fn parse_mountinfo(bytes: &[u8]) -> Result<Vec<MountEntry>> {
    let mut entries = Vec::new();
    for (line_no, line) in bytes.split(|byte| *byte == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&[u8]> = line
            .split(|byte| *byte == b' ')
            .filter(|field| !field.is_empty())
            .collect();
        let Some(separator) = fields.iter().position(|field| *field == b"-") else {
            return Err(Error::InvalidMountTable(format!(
                "line {} has no separator",
                line_no + 1
            )));
        };
        if separator < 6 || fields.len() < separator + 4 {
            return Err(Error::InvalidMountTable(format!(
                "line {} is incomplete",
                line_no + 1
            )));
        }
        entries.push(MountEntry {
            mount_point: PathBuf::from(OsString::from_vec(unmangle(fields[4]))),
            fstype: fields[separator + 1].to_vec(),
            vfs_options: fields[5].to_vec(),
            super_options: unmangle(fields[separator + 3]),
        });
    }
    Ok(entries)
}

pub fn strata_mounts(entries: &[MountEntry]) -> Result<Vec<StrataMount>> {
    entries
        .iter()
        .filter(|entry| entry.fstype == b"stratafs")
        .map(parse_strata_mount)
        .collect()
}

fn parse_strata_mount(entry: &MountEntry) -> Result<StrataMount> {
    let mut value = None;
    for option in split_escaped(&entry.super_options, b',')? {
        if let Some(rest) = option.strip_prefix(b"strata=") {
            if value.replace(rest).is_some() {
                return Err(Error::InvalidMountTable("duplicate strata= option".into()));
            }
        }
    }
    let value = value.ok_or_else(|| Error::InvalidMountTable("missing strata= option".into()))?;
    let elements = split_escaped(value, b':')?;
    if elements.is_empty() {
        return Err(Error::InvalidMountTable("empty strata= option".into()));
    }
    let mut strata = Vec::with_capacity(elements.len());
    for element in elements {
        let parts = split_escaped(element, b'+')?;
        let (path, flags) = parts
            .split_first()
            .ok_or_else(|| Error::InvalidMountTable("empty stratum".into()))?;
        let path = unescape_path(path)?;
        if !path.starts_with(b"/") {
            return Err(Error::InvalidMountTable("relative stratum path".into()));
        }
        let mut stratum = Stratum {
            path: PathBuf::from(OsString::from_vec(path)),
            create: false,
            read_only: false,
            allow_missing: false,
        };
        for flag in flags {
            let slot = match *flag {
                b"create" => &mut stratum.create,
                b"ro" => &mut stratum.read_only,
                b"am" => &mut stratum.allow_missing,
                _ => {
                    return Err(Error::InvalidMountTable(format!(
                        "unknown flag {}",
                        display_bytes(flag)
                    )));
                }
            };
            if *slot {
                return Err(Error::InvalidMountTable(format!(
                    "duplicate flag {}",
                    display_bytes(flag)
                )));
            }
            *slot = true;
        }
        strata.push(stratum);
    }
    Ok(StrataMount {
        mount_point: entry.mount_point.clone(),
        read_only: has_option(&entry.vfs_options, b"ro"),
        strata,
    })
}

fn split_escaped(value: &[u8], separator: u8) -> Result<Vec<&[u8]>> {
    let mut output = Vec::new();
    let mut start = 0;
    let mut escaped = false;
    for (index, byte) in value.iter().copied().enumerate() {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == separator {
            if index == start {
                return Err(Error::InvalidMountTable(
                    "empty escaped-list element".into(),
                ));
            }
            output.push(&value[start..index]);
            start = index + 1;
        }
    }
    if escaped {
        return Err(Error::InvalidMountTable("dangling escape".into()));
    }
    if start == value.len() {
        return Err(Error::InvalidMountTable("empty trailing element".into()));
    }
    output.push(&value[start..]);
    Ok(output)
}

fn unescape_path(path: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(path.len());
    let mut index = 0;
    while index < path.len() {
        if path[index] != b'\\' {
            output.push(path[index]);
            index += 1;
            continue;
        }
        let Some(next) = path.get(index + 1).copied() else {
            return Err(Error::InvalidMountTable("dangling path escape".into()));
        };
        if !matches!(next, b':' | b'+' | b',' | b'\\') {
            return Err(Error::InvalidMountTable(format!(
                "invalid path escape \\{}",
                char::from(next)
            )));
        }
        output.push(next);
        index += 2;
    }
    if output.is_empty() {
        return Err(Error::InvalidMountTable("empty stratum path".into()));
    }
    Ok(output)
}

fn unmangle(value: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(value.len());
    let mut index = 0;
    while index < value.len() {
        if value[index] == b'\\' && index + 3 < value.len() {
            let digits = &value[index + 1..index + 4];
            if digits.iter().all(|digit| (b'0'..=b'7').contains(digit)) {
                output.push((digits[0] - b'0') * 64 + (digits[1] - b'0') * 8 + digits[2] - b'0');
                index += 4;
                continue;
            }
        }
        output.push(value[index]);
        index += 1;
    }
    output
}

fn has_option(options: &[u8], wanted: &[u8]) -> bool {
    options
        .split(|byte| *byte == b',')
        .any(|option| option == wanted)
}

pub fn absolute_lexical(path: &Path) -> Result<PathBuf> {
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| Error::io("get current directory", error))?
            .join(path)
    };
    let mut output = PathBuf::from("/");
    for component in joined.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            Component::Normal(name) => output.push(name),
            Component::Prefix(_) => unreachable!("Unix paths have no prefix component"),
        }
    }
    Ok(output)
}

pub fn find_mount<'a>(mounts: &'a [StrataMount], path: &Path) -> Option<&'a StrataMount> {
    mounts
        .iter()
        .filter(|mount| path.starts_with(&mount.mount_point))
        .max_by_key(|mount| mount.mount_point.components().count())
}

pub fn relative_to_mount<'a>(mount: &StrataMount, path: &'a Path) -> Result<&'a Path> {
    path.strip_prefix(&mount.mount_point)
        .map_err(|_| Error::NotStratafs(display_path(path)))
}

pub fn candidate_path(stratum: &Stratum, relative: &Path) -> PathBuf {
    stratum.path.join(relative)
}

pub fn display_path(path: &Path) -> String {
    display_bytes(path.as_os_str().as_bytes())
}

pub fn display_bytes(bytes: &[u8]) -> String {
    let mut output = String::new();
    for byte in bytes {
        match *byte {
            b' '..=b'~' if *byte != b'\\' => output.push(char::from(*byte)),
            b'\\' => output.push_str("\\\\"),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            byte => output.push_str(&format!("\\x{byte:02x}")),
        }
    }
    output
}

pub fn json_path(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        Error::Unsupported(format!(
            "--json cannot represent non-UTF-8 path {}; use human output",
            display_path(path)
        ))
    })
}

pub fn parse_origin_lines(value: &[u8]) -> Result<Vec<PathBuf>> {
    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut index = 0;
    while index < value.len() {
        match value[index] {
            b'\n' => {
                lines.push(PathBuf::from(OsString::from_vec(std::mem::take(
                    &mut current,
                ))));
                index += 1;
            }
            b'\\' => {
                let Some(next) = value.get(index + 1).copied() else {
                    return Err(Error::InvalidMountTable(
                        "origin attribute has dangling escape".into(),
                    ));
                };
                if !matches!(next, b'\\' | b'\n') {
                    return Err(Error::InvalidMountTable(
                        "origin attribute has invalid escape".into(),
                    ));
                }
                current.push(next);
                index += 2;
            }
            byte => {
                current.push(byte);
                index += 1;
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(PathBuf::from(OsString::from_vec(current)));
    }
    if lines.iter().any(|line| !line.is_absolute()) {
        return Err(Error::InvalidMountTable(
            "origin attribute contains a non-absolute path".into(),
        ));
    }
    Ok(lines)
}

pub fn os_arg(value: &OsStr) -> PathBuf {
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stratafs_mount_and_escapes() {
        let data = b"31 20 0:42 / /merged rw - stratafs none rw,strata=/hi\\134\\054x:/create+create:/lo\\134+name+ro\n";
        let entries = parse_mountinfo(data).unwrap();
        let mounts = strata_mounts(&entries).unwrap();
        assert_eq!(mounts.len(), 1);
        assert_eq!(mounts[0].strata.len(), 3);
        assert_eq!(mounts[0].strata[0].path.as_os_str().as_bytes(), b"/hi,x");
        assert!(mounts[0].strata[1].create);
        assert_eq!(mounts[0].strata[2].path.as_os_str().as_bytes(), b"/lo+name");
        assert!(mounts[0].strata[2].read_only);
    }

    #[test]
    fn longest_mount_wins() {
        let mount = |path: &str| StrataMount {
            mount_point: PathBuf::from(path),
            read_only: false,
            strata: Vec::new(),
        };
        let mounts = [mount("/a"), mount("/a/b")];
        assert_eq!(
            find_mount(&mounts, Path::new("/a/b/c"))
                .unwrap()
                .mount_point,
            Path::new("/a/b")
        );
    }

    #[test]
    fn origin_lines_unescape_newline_and_backslash() {
        let lines = parse_origin_lines(b"/a\\\nname\n/b\\\\name").unwrap();
        assert_eq!(lines[0].as_os_str().as_bytes(), b"/a\nname");
        assert_eq!(lines[1].as_os_str().as_bytes(), b"/b\\name");
    }
}
