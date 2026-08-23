use std::ffi::{CString, OsStr};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{
    FileKind, MountEntry, StrataMount, Stratum, absolute_lexical, candidate_path, display_path,
    file_kind, find_mount, json_path, parse_origin_lines, relative_to_mount,
};

const ORIGIN_XATTR: &str = "system.stratafs.origin";
const VISIBILITY_PROBE_XATTR: &str = "user.stratafs.visibility-probe";
const MAX_DIFF_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Serialize)]
pub struct MountReport {
    pub mount: String,
    pub read_only: bool,
    pub strata: Vec<StratumReport>,
}

#[derive(Debug, Serialize)]
pub struct StratumReport {
    pub index: usize,
    pub path: String,
    pub flags: Vec<&'static str>,
    pub state: &'static str,
}

#[derive(Debug, Serialize)]
pub struct ResolveReport {
    pub path: String,
    pub mount: String,
    pub object_type: Option<FileKind>,
    pub strata: Vec<ResolutionStratum>,
    pub write: ActionReport,
    pub delete: ActionReport,
}

#[derive(Debug, Serialize)]
pub struct ResolutionStratum {
    pub index: usize,
    pub stratum: String,
    pub object: String,
    pub flags: Vec<&'static str>,
    pub state: &'static str,
    pub object_type: Option<FileKind>,
}

#[derive(Debug, Serialize)]
pub struct ActionReport {
    pub action: &'static str,
    pub target: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SweepState {
    Gap,
    Override,
    Shadowed,
}

impl std::fmt::Display for SweepState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Gap => "gap",
            Self::Override => "override",
            Self::Shadowed => "shadowed",
        })
    }
}

#[derive(Debug, Serialize)]
pub struct SweepEntry {
    pub path: String,
    pub state: SweepState,
    pub create_object: String,
    pub related_object: Option<String>,
}

struct Candidate {
    path: PathBuf,
    metadata: Option<Metadata>,
}

pub fn list_reports(
    mounts: &[StrataMount],
    requested: Option<&OsStr>,
    json: bool,
) -> Result<Vec<MountReport>> {
    let selected: Vec<&StrataMount> = if let Some(requested) = requested {
        let path = absolute_lexical(Path::new(requested))?;
        let mount = mounts
            .iter()
            .rev()
            .find(|mount| mount.mount_point == path)
            .ok_or_else(|| Error::NotStratafs(display_path(&path)))?;
        vec![mount]
    } else {
        mounts.iter().collect()
    };
    selected
        .into_iter()
        .map(|mount| mount_report(mount, json))
        .collect()
}

fn mount_report(mount: &StrataMount, json: bool) -> Result<MountReport> {
    let mut strata = Vec::with_capacity(mount.strata.len());
    for (index, stratum) in mount.strata.iter().enumerate() {
        let state = match std::fs::metadata(&stratum.path) {
            Ok(metadata) if metadata.is_dir() => "present",
            Ok(_) => "not_directory",
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => "absent",
            Err(error) => {
                return Err(Error::NotVisible {
                    path: format!("stratum {index} ({})", display_path(&stratum.path)),
                    source: error,
                });
            }
        };
        strata.push(StratumReport {
            index,
            path: output_path(&stratum.path, json)?,
            flags: flags(stratum),
            state,
        });
    }
    Ok(MountReport {
        mount: output_path(&mount.mount_point, json)?,
        read_only: mount.read_only,
        strata,
    })
}

pub fn read_origin(path: &Path) -> Result<Vec<u8>> {
    match xattr::get(path, ORIGIN_XATTR) {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err(Error::NotStratafs(display_path(path))),
        Err(error) => Err(Error::path_io("read origin of", path, error)),
    }
}

pub fn resolve_report(
    entries: &[MountEntry],
    mounts: &[StrataMount],
    requested: &OsStr,
    json: bool,
) -> Result<ResolveReport> {
    let path = absolute_lexical(Path::new(requested))?;
    let mount = find_mount(mounts, &path).ok_or_else(|| Error::NotStratafs(display_path(&path)))?;
    let relative = relative_to_mount(mount, &path)?;
    let merged_metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(Error::path_io("inspect", &path, error)),
    };
    let candidates = inspect_candidates(mount, relative)?;

    let origins = if let Some(metadata) = &merged_metadata {
        let value = match xattr::get(&path, ORIGIN_XATTR) {
            Ok(Some(value)) => value,
            Ok(None) => return Err(Error::NotStratafs(display_path(&path))),
            Err(error) if is_denied(&error) && metadata.is_dir() => {
                return Err(identify_invisible_participant(mount, &candidates, error));
            }
            Err(error) => return Err(Error::path_io("read origin of", &path, error)),
        };
        if metadata.is_dir() {
            parse_origin_lines(&value)?
        } else {
            vec![PathBuf::from(OsStr::from_bytes(&value))]
        }
    } else {
        Vec::new()
    };

    let provider = candidates
        .iter()
        .position(|candidate| candidate.metadata.is_some());
    let provider_kind =
        provider.and_then(|index| candidates[index].metadata.as_ref().map(file_kind));
    validate_origins(
        &origins,
        &candidates,
        provider,
        merged_metadata.as_ref().is_some_and(Metadata::is_dir),
    )?;

    let parent_present = if provider.is_none() {
        let parent = path.parent().unwrap_or(Path::new("/"));
        match std::fs::symlink_metadata(parent) {
            Ok(metadata) => metadata.is_dir(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(Error::path_io("inspect parent", parent, error)),
        }
    } else {
        true
    };

    let mut strata = Vec::with_capacity(candidates.len());
    for (index, (stratum, candidate)) in mount.strata.iter().zip(&candidates).enumerate() {
        let kind = candidate.metadata.as_ref().map(file_kind);
        let state = resolution_state(
            index,
            provider,
            provider_kind,
            kind,
            &origins,
            &candidate.path,
        );
        strata.push(ResolutionStratum {
            index,
            stratum: output_path(&stratum.path, json)?,
            object: output_path(&candidate.path, json)?,
            flags: flags(stratum),
            state,
            object_type: kind,
        });
    }

    let write = write_action(
        entries,
        mount,
        &candidates,
        provider,
        provider_kind,
        parent_present,
        json,
    )?;
    let delete = delete_action(entries, mount, &candidates, provider, provider_kind, json)?;
    Ok(ResolveReport {
        path: output_path(&path, json)?,
        mount: output_path(&mount.mount_point, json)?,
        object_type: merged_metadata.as_ref().map(file_kind),
        strata,
        write,
        delete,
    })
}

fn inspect_candidates(mount: &StrataMount, relative: &Path) -> Result<Vec<Candidate>> {
    mount
        .strata
        .iter()
        .enumerate()
        .map(|(index, stratum)| {
            let path = candidate_path(stratum, relative);
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => Some(metadata),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => {
                    return Err(Error::NotVisible {
                        path: format!("stratum {index} ({})", display_path(&path)),
                        source: error,
                    });
                }
            };
            Ok(Candidate { path, metadata })
        })
        .collect()
}

fn validate_origins(
    origins: &[PathBuf],
    candidates: &[Candidate],
    provider: Option<usize>,
    directory: bool,
) -> Result<()> {
    if origins.is_empty() {
        return if provider.is_none() {
            Ok(())
        } else {
            Err(Error::Unsupported(
                "the path changed while it was being inspected; retry the command".into(),
            ))
        };
    }
    let every_known = origins
        .iter()
        .all(|origin| candidates.iter().any(|candidate| candidate.path == *origin));
    if !every_known {
        return Err(Error::InvalidMountTable(
            "origin attribute names an object outside the reported stratum stack".into(),
        ));
    }
    if provider.is_none_or(|index| origins.first() != Some(&candidates[index].path)) {
        return Err(Error::Unsupported(
            "the stratum stack changed while it was being inspected; retry the command".into(),
        ));
    }
    if directory {
        let expected: Vec<&PathBuf> = candidates
            .iter()
            .filter(|candidate| candidate.metadata.as_ref().is_some_and(Metadata::is_dir))
            .map(|candidate| &candidate.path)
            .collect();
        if origins.iter().collect::<Vec<_>>() != expected {
            return Err(Error::Unsupported(
                "the directory participants changed while they were being inspected; retry the command"
                    .into(),
            ));
        }
    } else if origins.len() != 1 {
        return Err(Error::InvalidMountTable(
            "non-directory origin contains more than one provider".into(),
        ));
    }
    Ok(())
}

fn identify_invisible_participant(
    mount: &StrataMount,
    candidates: &[Candidate],
    original: std::io::Error,
) -> Error {
    for (index, candidate) in candidates.iter().enumerate() {
        if !candidate.metadata.as_ref().is_some_and(Metadata::is_dir) {
            continue;
        }
        if let Err(error) = xattr::get(&candidate.path, VISIBILITY_PROBE_XATTR)
            && is_denied(&error)
        {
            return Error::NotVisible {
                path: format!("stratum {index} ({})", display_path(&candidate.path)),
                source: error,
            };
        }
    }
    Error::NotVisible {
        path: format!(
            "one or more participants of {}",
            display_path(&mount.mount_point)
        ),
        source: original,
    }
}

fn is_denied(error: &std::io::Error) -> bool {
    matches!(error.raw_os_error(), Some(libc::EACCES | libc::EPERM))
}

fn resolution_state(
    index: usize,
    provider: Option<usize>,
    provider_kind: Option<FileKind>,
    kind: Option<FileKind>,
    origins: &[PathBuf],
    path: &Path,
) -> &'static str {
    let Some(kind) = kind else {
        return "absent";
    };
    let Some(provider) = provider else {
        return "absent";
    };
    if index == provider {
        return "provider";
    }
    if provider_kind == Some(FileKind::Directory)
        && kind == FileKind::Directory
        && origins.iter().any(|origin| origin == path)
    {
        return "participant";
    }
    if provider_kind == Some(kind) {
        "shadowed"
    } else {
        "masked"
    }
}

fn write_action(
    entries: &[MountEntry],
    mount: &StrataMount,
    candidates: &[Candidate],
    provider: Option<usize>,
    provider_kind: Option<FileKind>,
    parent_present: bool,
    json: bool,
) -> Result<ActionReport> {
    let Some(provider) = provider else {
        if !parent_present {
            return Ok(ActionReport {
                action: "enoent",
                target: None,
                reason: "the parent directory is absent".into(),
            });
        }
        let create = mount.strata.iter().position(|stratum| stratum.create);
        return if mount.read_only {
            Ok(erofs("the StrataFS mount is read-only"))
        } else if let Some(index) = create {
            if stratum_present(&mount.strata[index])? {
                Ok(ActionReport {
                    action: "create",
                    target: Some(output_path(&candidates[index].path, json)?),
                    reason: format!("the path is absent and stratum {index} is the create stratum"),
                })
            } else {
                Ok(erofs(&format!("create stratum {index} is absent")))
            }
        } else {
            Ok(erofs("the mount has no create stratum"))
        };
    };

    if provider_kind == Some(FileKind::Symlink) {
        return Ok(ActionReport {
            action: "follow_symlink",
            target: Some(output_path(&candidates[provider].path, json)?),
            reason: "content I/O follows the link; routing applies to the resolved target, not the link entry"
                .into(),
        });
    }

    if matches!(
        provider_kind,
        Some(FileKind::Fifo | FileKind::Socket | FileKind::BlockDevice | FileKind::CharDevice)
    ) {
        return Ok(ActionReport {
            action: "in_place",
            target: Some(output_path(&candidates[provider].path, json)?),
            reason: "special-file I/O does not modify the filesystem object".into(),
        });
    }
    match accepts_modification(
        entries,
        mount,
        &mount.strata[provider],
        &candidates[provider].path,
    )? {
        Acceptance::Yes => Ok(ActionReport {
            action: "in_place",
            target: Some(output_path(&candidates[provider].path, json)?),
            reason: format!("provider stratum {provider} accepts modification"),
        }),
        Acceptance::Unknown(reason) => Ok(ActionReport {
            action: "unknown",
            target: Some(output_path(&candidates[provider].path, json)?),
            reason,
        }),
        Acceptance::No(reason) => {
            let create = mount.strata.iter().position(|stratum| stratum.create);
            if let Some(create) = create
                && create < provider
                && stratum_present(&mount.strata[create])?
                && matches!(
                    provider_kind,
                    Some(FileKind::Regular | FileKind::Directory | FileKind::Symlink)
                )
            {
                Ok(ActionReport {
                    action: "copy_up",
                    target: Some(output_path(&candidates[create].path, json)?),
                    reason: format!(
                        "provider does not accept modification ({reason}); create stratum {create} is higher"
                    ),
                })
            } else {
                Ok(erofs(&format!(
                    "provider does not accept modification ({reason}) and no present create stratum is higher"
                )))
            }
        }
    }
}

fn delete_action(
    entries: &[MountEntry],
    mount: &StrataMount,
    candidates: &[Candidate],
    provider: Option<usize>,
    provider_kind: Option<FileKind>,
    json: bool,
) -> Result<ActionReport> {
    let Some(provider) = provider else {
        return Ok(ActionReport {
            action: "enoent",
            target: None,
            reason: "no stratum holds the path".into(),
        });
    };
    match accepts_modification(
        entries,
        mount,
        &mount.strata[provider],
        &candidates[provider].path,
    )? {
        Acceptance::Yes => {
            let resurfaces = candidates
                .iter()
                .enumerate()
                .skip(provider + 1)
                .find(|(_, candidate)| candidate.metadata.is_some());
            let mut reason = if let Some((index, candidate)) = resurfaces {
                format!(
                    "remove the provider from stratum {provider}; stratum {index} then resurfaces at {}",
                    display_path(&candidate.path)
                )
            } else {
                format!("remove the provider from stratum {provider}; the name then becomes absent")
            };
            if provider_kind == Some(FileKind::Directory) {
                reason.push_str("; removal is conditional on the merged directory being empty");
            }
            Ok(ActionReport {
                action: "remove",
                target: Some(output_path(&candidates[provider].path, json)?),
                reason,
            })
        }
        Acceptance::No(reason) => Ok(erofs(&format!(
            "provider does not accept removal ({reason})"
        ))),
        Acceptance::Unknown(reason) => Ok(ActionReport {
            action: "unknown",
            target: Some(output_path(&candidates[provider].path, json)?),
            reason,
        }),
    }
}

enum Acceptance {
    Yes,
    No(String),
    Unknown(String),
}

fn accepts_modification(
    entries: &[MountEntry],
    mount: &StrataMount,
    stratum: &Stratum,
    object: &Path,
) -> Result<Acceptance> {
    if mount.read_only {
        return Ok(Acceptance::No("StrataFS mount is read-only".into()));
    }
    if stratum.read_only {
        return Ok(Acceptance::No("stratum carries +ro".into()));
    }
    if path_mount_read_only(entries, object) {
        return Ok(Acceptance::No(
            "stratum filesystem is mounted read-only".into(),
        ));
    }
    match immutable(object)? {
        Some(true) => Ok(Acceptance::No("provider object is immutable".into())),
        Some(false) => Ok(Acceptance::Yes),
        None => Ok(Acceptance::Unknown(
            "the provider filesystem does not report whether the object is immutable".into(),
        )),
    }
}

fn path_mount_read_only(entries: &[MountEntry], path: &Path) -> bool {
    entries
        .iter()
        .filter(|entry| path.starts_with(&entry.mount_point))
        .max_by_key(|entry| entry.mount_point.components().count())
        .is_some_and(|entry| {
            entry
                .vfs_options
                .split(|byte| *byte == b',')
                .any(|option| option == b"ro")
        })
}

fn immutable(path: &Path) -> Result<Option<bool>> {
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        return Ok(None);
    }

    #[cfg(target_os = "linux")]
    {
        const STATX_BASIC_STATS: libc::c_uint = 0x07ff;
        const STATX_ATTR_IMMUTABLE: u64 = 0x0010;

        let c_path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            Error::Unsupported(format!("path contains NUL: {}", display_path(path)))
        })?;
        // statx is a fixed 256-byte Linux UAPI structure. Keep it opaque here: the
        // two u64 fields needed below have stable offsets (attributes at byte 8,
        // attributes_mask at byte 56). This avoids libc target differences where
        // the musl bindings omit statx despite the kernel ABI being available.
        let mut statx = [0_u64; 32];
        // SAFETY: c_path is NUL-terminated; statx is aligned writable storage of
        // the kernel ABI's exact 256-byte size; all scalar syscall arguments have
        // their documented types.
        let result = unsafe {
            libc::syscall(
                libc::SYS_statx,
                libc::AT_FDCWD,
                c_path.as_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
                STATX_BASIC_STATS,
                statx.as_mut_ptr().cast::<libc::c_void>(),
            )
        };
        if result != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ENOSYS) {
                return Ok(None);
            }
            return Err(Error::path_io("statx", path, error));
        }
        let attributes = statx[1];
        let attributes_mask = statx[7];
        if attributes_mask & STATX_ATTR_IMMUTABLE == 0 {
            Ok(None)
        } else {
            Ok(Some(attributes & STATX_ATTR_IMMUTABLE != 0))
        }
    }
}

fn stratum_present(stratum: &Stratum) -> Result<bool> {
    match std::fs::metadata(&stratum.path) {
        Ok(metadata) => Ok(metadata.is_dir()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(Error::path_io("inspect stratum", &stratum.path, error)),
    }
}

fn erofs(reason: &str) -> ActionReport {
    ActionReport {
        action: "erofs",
        target: None,
        reason: reason.into(),
    }
}

pub fn sweep_reports(
    mounts: &[StrataMount],
    requested: Option<&OsStr>,
    json: bool,
) -> Result<Vec<SweepEntry>> {
    let mount = select_one_mount(mounts, requested)?;
    let create = mount
        .strata
        .iter()
        .position(|stratum| stratum.create)
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "{} has no create stratum",
                display_path(&mount.mount_point)
            ))
        })?;
    let root = &mount.strata[create].path;
    if !stratum_present(&mount.strata[create])? {
        return Err(Error::Unsupported(format!(
            "create stratum {create} ({}) is absent",
            display_path(root)
        )));
    }
    let mut relative_paths = Vec::new();
    let directory = open_directory_nofollow(root)?;
    collect_create_entries(root, &directory, Path::new(""), &mut relative_paths)?;
    relative_paths.sort_by(|left, right| {
        left.as_os_str()
            .as_bytes()
            .cmp(right.as_os_str().as_bytes())
    });

    let mut reports = Vec::with_capacity(relative_paths.len());
    for relative in relative_paths {
        let create_object = candidate_path(&mount.strata[create], &relative);
        let higher = find_existing(&mount.strata[..create], &relative, true)?;
        let lower = find_existing(&mount.strata[create + 1..], &relative, false)?
            .map(|(offset, path)| (create + 1 + offset, path));
        let (state, related) = if let Some((_, path)) = higher {
            (SweepState::Shadowed, Some(path))
        } else if let Some((_, path)) = lower {
            (SweepState::Override, Some(path))
        } else {
            (SweepState::Gap, None)
        };
        reports.push(SweepEntry {
            path: output_path(&relative, json)?,
            state,
            create_object: output_path(&create_object, json)?,
            related_object: related
                .as_deref()
                .map(|path| output_path(path, json))
                .transpose()?,
        });
    }
    Ok(reports)
}

fn select_one_mount<'a>(
    mounts: &'a [StrataMount],
    requested: Option<&OsStr>,
) -> Result<&'a StrataMount> {
    if let Some(requested) = requested {
        let path = absolute_lexical(Path::new(requested))?;
        mounts
            .iter()
            .rev()
            .find(|mount| mount.mount_point == path)
            .ok_or_else(|| Error::NotStratafs(display_path(&path)))
    } else if mounts.len() == 1 {
        Ok(&mounts[0])
    } else if mounts.is_empty() {
        Err(Error::NoMounts)
    } else {
        Err(Error::Unsupported(
            "more than one StrataFS mount exists; specify a mount point".into(),
        ))
    }
}

fn collect_create_entries(
    root: &Path,
    directory: &File,
    relative: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<()> {
    let proc_path = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()));
    let mut entries: Vec<_> = std::fs::read_dir(&proc_path)
        .map_err(|error| Error::path_io("enumerate", &root.join(relative), error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| Error::path_io("enumerate", &root.join(relative), error))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        let child = relative.join(&name);
        let metadata = entry
            .metadata()
            .map_err(|error| Error::path_io("inspect", &root.join(&child), error))?;
        output.push(child.clone());
        if metadata.is_dir() {
            let child_directory =
                open_child_directory_nofollow(directory, &name, &root.join(&child))?;
            let opened_metadata = child_directory
                .metadata()
                .map_err(|error| Error::path_io("inspect", &root.join(&child), error))?;
            if metadata.dev() != opened_metadata.dev() || metadata.ino() != opened_metadata.ino() {
                return Err(Error::Unsupported(format!(
                    "{} changed while it was being inspected; retry the command",
                    display_path(&root.join(&child))
                )));
            }
            collect_create_entries(root, &child_directory, &child, output)?;
        }
    }
    Ok(())
}

fn open_directory_nofollow(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| Error::path_io("open directory", path, error))
}

fn open_child_directory_nofollow(parent: &File, name: &OsStr, display: &Path) -> Result<File> {
    let name = CString::new(name.as_bytes())
        .map_err(|_| Error::Unsupported(format!("path contains NUL: {}", display_path(display))))?;
    // SAFETY: name is NUL-terminated, parent is a live directory descriptor,
    // and a non-negative result is an owned descriptor transferred to File.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return Err(Error::path_io(
            "open directory",
            display,
            std::io::Error::last_os_error(),
        ));
    }
    // SAFETY: openat returned a fresh owned descriptor and no other owner exists.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn find_existing(
    strata: &[Stratum],
    relative: &Path,
    non_directory_ancestor_masks: bool,
) -> Result<Option<(usize, PathBuf)>> {
    for (index, stratum) in strata.iter().enumerate() {
        let path = candidate_path(stratum, relative);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Ok(Some((index, path))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error)
                if error.raw_os_error() == Some(libc::ENOTDIR) && non_directory_ancestor_masks =>
            {
                let blocker = find_non_directory_ancestor(stratum, relative)?.unwrap_or(path);
                return Ok(Some((index, blocker)));
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOTDIR) => {}
            Err(error) => {
                return Err(Error::NotVisible {
                    path: display_path(&path),
                    source: error,
                });
            }
        }
    }
    Ok(None)
}

fn find_non_directory_ancestor(stratum: &Stratum, relative: &Path) -> Result<Option<PathBuf>> {
    let mut path = stratum.path.clone();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            continue;
        };
        path.push(component);
        match std::fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => {}
            Ok(_) => return Ok(Some(path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(Error::NotVisible {
                    path: display_path(&path),
                    source: error,
                });
            }
        }
    }
    Ok(None)
}

pub fn diff_path(mounts: &[StrataMount], requested: &OsStr) -> Result<bool> {
    let path = absolute_lexical(Path::new(requested))?;
    let mount = find_mount(mounts, &path).ok_or_else(|| Error::NotStratafs(display_path(&path)))?;
    // Require the merged path to be visible to the caller before directly
    // inspecting any stratum object.
    let _ = read_origin(&path)?;
    let relative = relative_to_mount(mount, &path)?;
    let create = mount
        .strata
        .iter()
        .position(|stratum| stratum.create)
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "{} has no create stratum",
                display_path(&mount.mount_point)
            ))
        })?;
    let create_path = candidate_path(&mount.strata[create], relative);
    let create_metadata = std::fs::symlink_metadata(&create_path)
        .map_err(|error| Error::path_io("inspect override", &create_path, error))?;
    let (_, default_path) = find_existing(&mount.strata[create + 1..], relative, false)?
        .ok_or_else(|| {
            Error::Unsupported(format!(
                "{} is not an override: no lower default exists",
                display_path(&path)
            ))
        })?;
    let default_metadata = std::fs::symlink_metadata(&default_path)
        .map_err(|error| Error::path_io("inspect default", &default_path, error))?;
    let create_kind = file_kind(&create_metadata);
    let default_kind = file_kind(&default_metadata);
    if create_kind != default_kind {
        println!("--- {} ({default_kind})", display_path(&default_path));
        println!("+++ {} ({create_kind})", display_path(&create_path));
        println!("type changed");
        return Ok(true);
    }
    match create_kind {
        FileKind::Regular => diff_regular(&default_path, &create_path),
        FileKind::Symlink => {
            let old = std::fs::read_link(&default_path)
                .map_err(|error| Error::path_io("read symlink", &default_path, error))?;
            let new = std::fs::read_link(&create_path)
                .map_err(|error| Error::path_io("read symlink", &create_path, error))?;
            if old == new {
                Ok(false)
            } else {
                println!("--- {}", display_path(&default_path));
                println!("+++ {}", display_path(&create_path));
                println!("-{}", display_path(&old));
                println!("+{}", display_path(&new));
                Ok(true)
            }
        }
        _ => Err(Error::Unsupported(format!(
            "cannot diff {create_kind} objects"
        ))),
    }
}

fn diff_regular(old_path: &Path, new_path: &Path) -> Result<bool> {
    let old = read_regular_bounded(old_path)?;
    let new = read_regular_bounded(new_path)?;
    if old == new {
        return Ok(false);
    }
    println!("--- {}", display_path(old_path));
    println!("+++ {}", display_path(new_path));
    if old.contains(&0) || new.contains(&0) {
        println!("Binary files differ");
        return Ok(true);
    }
    let old_lines: Vec<&[u8]> = old.split_inclusive(|byte| *byte == b'\n').collect();
    let new_lines: Vec<&[u8]> = new.split_inclusive(|byte| *byte == b'\n').collect();
    let prefix = old_lines
        .iter()
        .zip(&new_lines)
        .take_while(|(left, right)| left == right)
        .count();
    let suffix = old_lines[prefix..]
        .iter()
        .rev()
        .zip(new_lines[prefix..].iter().rev())
        .take_while(|(left, right)| left == right)
        .count();
    let old_end = old_lines.len() - suffix;
    let new_end = new_lines.len() - suffix;
    println!(
        "@@ -{},{} +{},{} @@",
        prefix + 1,
        old_end - prefix,
        prefix + 1,
        new_end - prefix
    );
    let mut stdout = std::io::stdout().lock();
    for line in &old_lines[prefix..old_end] {
        stdout
            .write_all(b"-")
            .map_err(|error| Error::io("write diff", error))?;
        stdout
            .write_all(line)
            .map_err(|error| Error::io("write diff", error))?;
        if !line.ends_with(b"\n") {
            stdout
                .write_all(b"\n\\ No newline at end of file\n")
                .map_err(|error| Error::io("write diff", error))?;
        }
    }
    for line in &new_lines[prefix..new_end] {
        stdout
            .write_all(b"+")
            .map_err(|error| Error::io("write diff", error))?;
        stdout
            .write_all(line)
            .map_err(|error| Error::io("write diff", error))?;
        if !line.ends_with(b"\n") {
            stdout
                .write_all(b"\n\\ No newline at end of file\n")
                .map_err(|error| Error::io("write diff", error))?;
        }
    }
    Ok(true)
}

fn read_regular_bounded(path: &Path) -> Result<Vec<u8>> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|error| Error::path_io("open", path, error))?;
    if !file
        .metadata()
        .map_err(|error| Error::path_io("inspect", path, error))?
        .is_file()
    {
        return Err(Error::Unsupported(format!(
            "{} changed type while being inspected",
            display_path(path)
        )));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_DIFF_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| Error::path_io("read", path, error))?;
    if bytes.len() as u64 > MAX_DIFF_BYTES {
        return Err(Error::Unsupported(format!(
            "{} exceeds the {} MiB diff limit",
            display_path(path),
            MAX_DIFF_BYTES / (1024 * 1024)
        )));
    }
    Ok(bytes)
}

fn flags(stratum: &Stratum) -> Vec<&'static str> {
    let mut output = Vec::new();
    if stratum.create {
        output.push("create");
    }
    if stratum.read_only {
        output.push("ro");
    }
    if stratum.allow_missing {
        output.push("am");
    }
    output
}

fn output_path(path: &Path, json: bool) -> Result<String> {
    if json {
        json_path(path)
    } else {
        Ok(display_path(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stratum(path: PathBuf, create: bool, read_only: bool) -> Stratum {
        Stratum {
            path,
            create,
            read_only,
            allow_missing: false,
        }
    }

    #[test]
    fn list_without_mounts_is_an_empty_success() {
        assert!(list_reports(&[], None, false).unwrap().is_empty());
    }

    #[test]
    fn origin_validation_rejects_a_path_created_during_inspection() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("appeared");
        std::fs::write(&path, b"x").unwrap();
        let metadata = std::fs::symlink_metadata(&path).unwrap();
        let candidates = [Candidate {
            path,
            metadata: Some(metadata),
        }];
        assert!(validate_origins(&[], &candidates, Some(0), false).is_err());
    }

    #[test]
    fn origin_validation_rejects_a_changed_directory_participant_set() {
        let temp = tempfile::tempdir().unwrap();
        let high = temp.path().join("high");
        let low = temp.path().join("low");
        std::fs::create_dir(&high).unwrap();
        std::fs::create_dir(&low).unwrap();
        let candidates = [
            Candidate {
                metadata: Some(std::fs::symlink_metadata(&high).unwrap()),
                path: high.clone(),
            },
            Candidate {
                metadata: Some(std::fs::symlink_metadata(&low).unwrap()),
                path: low,
            },
        ];
        assert!(validate_origins(&[high], &candidates, Some(0), true).is_err());
    }

    #[test]
    fn sweep_classification_is_precedence_aware() {
        let temp = tempfile::tempdir().unwrap();
        let high = temp.path().join("high");
        let create = temp.path().join("create");
        let low = temp.path().join("low");
        std::fs::create_dir_all(&high).unwrap();
        std::fs::create_dir_all(&create).unwrap();
        std::fs::create_dir_all(&low).unwrap();
        std::fs::write(create.join("gap"), b"x").unwrap();
        std::fs::write(create.join("override"), b"x").unwrap();
        std::fs::write(create.join("shadowed"), b"x").unwrap();
        std::fs::write(low.join("override"), b"y").unwrap();
        std::fs::write(high.join("shadowed"), b"z").unwrap();
        let mounts = [StrataMount {
            mount_point: temp.path().join("merged"),
            read_only: false,
            strata: vec![
                stratum(high, false, false),
                stratum(create, true, false),
                stratum(low, false, true),
            ],
        }];
        let reports =
            sweep_reports(&mounts, Some(mounts[0].mount_point.as_os_str()), false).unwrap();
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].state, SweepState::Gap);
        assert_eq!(reports[1].state, SweepState::Override);
        assert_eq!(reports[2].state, SweepState::Shadowed);
    }

    #[test]
    fn state_distinguishes_participant_shadow_and_mask() {
        let origins = [PathBuf::from("/lower-dir")];
        assert_eq!(
            resolution_state(
                1,
                Some(0),
                Some(FileKind::Directory),
                Some(FileKind::Directory),
                &origins,
                Path::new("/lower-dir")
            ),
            "participant"
        );
        assert_eq!(
            resolution_state(
                1,
                Some(0),
                Some(FileKind::Regular),
                Some(FileKind::Regular),
                &[],
                Path::new("/lower")
            ),
            "shadowed"
        );
        assert_eq!(
            resolution_state(
                1,
                Some(0),
                Some(FileKind::Regular),
                Some(FileKind::Directory),
                &[],
                Path::new("/lower")
            ),
            "masked"
        );
    }

    #[test]
    fn sweep_treats_a_higher_non_directory_ancestor_as_shadowing() {
        let temp = tempfile::tempdir().unwrap();
        let high = temp.path().join("high");
        let create = temp.path().join("create");
        let low = temp.path().join("low");
        std::fs::create_dir_all(&high).unwrap();
        std::fs::create_dir_all(create.join("blocked")).unwrap();
        std::fs::create_dir_all(&low).unwrap();
        std::fs::write(high.join("blocked"), b"mask").unwrap();
        std::fs::write(create.join("blocked/child"), b"stale").unwrap();
        let mounts = [StrataMount {
            mount_point: temp.path().join("merged"),
            read_only: false,
            strata: vec![
                stratum(high, false, false),
                stratum(create, true, false),
                stratum(low, false, true),
            ],
        }];
        let reports =
            sweep_reports(&mounts, Some(mounts[0].mount_point.as_os_str()), false).unwrap();
        let child = reports
            .iter()
            .find(|report| report.path == "blocked/child")
            .unwrap();
        assert_eq!(child.state, SweepState::Shadowed);
    }

    #[test]
    fn sweep_does_not_follow_create_stratum_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let create = temp.path().join("create");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&create).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret"), b"outside").unwrap();
        std::os::unix::fs::symlink(&outside, create.join("link")).unwrap();
        let mounts = [StrataMount {
            mount_point: temp.path().join("merged"),
            read_only: false,
            strata: vec![stratum(create, true, false)],
        }];
        let reports =
            sweep_reports(&mounts, Some(mounts[0].mount_point.as_os_str()), false).unwrap();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].path, "link");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn statx_immutable_probe_uses_the_portable_kernel_abi() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert_eq!(immutable(file.path()).unwrap(), Some(false));
    }
}
