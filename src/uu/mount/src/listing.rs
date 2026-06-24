// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! List mode (§11): `mount` / `mount -t TYPE` with no operands.
//!
//! Renders `SOURCE on TARGET type FSTYPE (OPTIONS)` from `/proc/self/mountinfo`,
//! with the positional VFS→FS option merge and ro/rw coalescing, mountinfo
//! unmangling, and control-character replacement in the mount point.

use std::io::Write;

use crate::error::{MountError, Result};
use crate::mountinfo::{self, MountEntry};
use crate::request::ListRequest;

/// Run list mode.
pub fn run(req: &ListRequest) -> Result<()> {
    let entries = mountinfo::read().map_err(|e| MountError::System(format!("reading mountinfo: {e}")))?;
    let filter = req.type_filter.as_deref().map(TypeFilter::parse);

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for e in &entries {
        if let Some(f) = &filter {
            if !f.matches(&e.fstype) {
                continue;
            }
        }
        let line = render(e, req.show_labels);
        // Best-effort: a broken pipe (e.g. `| head`) is not an error.
        if out.write_all(&line).is_err() || out.write_all(b"\n").is_err() {
            break;
        }
    }
    Ok(())
}

/// `SOURCE on TARGET type FSTYPE (OPTIONS)` [`[LABEL]`].
fn render(e: &MountEntry, show_labels: bool) -> Vec<u8> {
    let mut line = Vec::new();
    line.extend_from_slice(&pretty_source(&e.source));
    line.extend_from_slice(b" on ");
    line.extend_from_slice(&sanitize_mountpoint(&e.mount_point));
    line.extend_from_slice(b" type ");
    line.extend_from_slice(&e.fstype);
    line.extend_from_slice(b" (");
    line.extend_from_slice(&merge_options(&e.vfs_opts, &e.super_opts));
    line.push(b')');
    if show_labels {
        if let Some(label) = crate::blkid::device_label(&e.source) {
            line.extend_from_slice(b" [");
            line.extend_from_slice(&label);
            line.push(b']');
        }
    }
    line
}

/// `mnt_pretty_path`-style source rendering (§11): full `realpath` of the
/// source, with `/dev/loopN`→backing-file and `/dev/dm-N`→`/dev/mapper/<name>`.
fn pretty_source(source: &[u8]) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    // /dev/loopN → backing file.
    if let Some(n) = source.strip_prefix(b"/dev/") {
        if n.starts_with(b"loop") && n[4..].iter().all(u8::is_ascii_digit) && n.len() > 4 {
            let p = format!("/sys/block/{}/loop/backing_file", String::from_utf8_lossy(n));
            if let Ok(data) = std::fs::read(&p) {
                let trimmed = data.strip_suffix(b"\n").unwrap_or(&data);
                if !trimmed.is_empty() {
                    return trimmed.to_vec();
                }
            }
        }
        // /dev/dm-N → /dev/mapper/<name>.
        if n.starts_with(b"dm-") && n[3..].iter().all(u8::is_ascii_digit) && n.len() > 3 {
            let p = format!("/sys/block/{}/dm/name", String::from_utf8_lossy(n));
            if let Ok(data) = std::fs::read(&p) {
                let name = data.strip_suffix(b"\n").unwrap_or(&data);
                if !name.is_empty() {
                    let mut out = b"/dev/mapper/".to_vec();
                    out.extend_from_slice(name);
                    return out;
                }
            }
        }
    }
    // Resolve symlinks via realpath for an absolute device path; pseudo
    // sources (`tmpfs`, `proc`, `none`, …) are not paths — pass them through.
    if source.starts_with(b"/") {
        std::fs::canonicalize(std::ffi::OsStr::from_bytes(source))
            .map_or_else(|_| source.to_vec(), |p| p.as_os_str().as_bytes().to_vec())
    } else {
        source.to_vec()
    }
}

/// Positional VFS→FS merge with ro/rw coalescing (§11): per-mount options
/// first, then superblock options, dropping a duplicate ro/rw and not
/// reordering anything else.
fn merge_options(vfs: &[u8], sb: &[u8]) -> Vec<u8> {
    let mut toks: Vec<&[u8]> = vfs.split(|&b| b == b',').filter(|t| !t.is_empty()).collect();
    // mountinfo's vfs field leads with ro/rw; the sb field repeats it — drop the
    // repeat so the merged view shows it once (coalesced).
    let have_rorw = toks.first().is_some_and(|t| *t == b"ro" || *t == b"rw");
    for t in sb.split(|&b| b == b',').filter(|t| !t.is_empty()) {
        if have_rorw && (t == b"ro" || t == b"rw") {
            continue;
        }
        if !toks.contains(&t) {
            toks.push(t);
        }
    }
    toks.join(&b","[..])
}

/// Replace control characters in the mount point with `?` (§11; source/fstype
/// /options are printed raw, matching mount(8)).
fn sanitize_mountpoint(mp: &[u8]) -> Vec<u8> {
    mp.iter()
        .map(|&b| if b < 0x20 || b == 0x7f { b'?' } else { b })
        .collect()
}

/// A parsed `-t` listing filter (a comma list, optionally `no`-negated).
struct TypeFilter {
    types: Vec<Vec<u8>>,
    negate: bool,
}

impl TypeFilter {
    fn parse(spec: &[u8]) -> Self {
        // A leading `no` negates the whole list: `-t noext4,xfs` excludes both.
        let (negate, body) = match spec.strip_prefix(b"no") {
            Some(rest) => (true, rest),
            None => (false, spec),
        };
        let types = body
            .split(|&b| b == b',')
            .filter(|t| !t.is_empty())
            .map(<[u8]>::to_vec)
            .collect();
        Self { types, negate }
    }

    fn matches(&self, fstype: &[u8]) -> bool {
        let listed = self.types.iter().any(|t| t == fstype);
        listed != self.negate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_filter_include() {
        let f = TypeFilter::parse(b"ext4,xfs");
        assert!(f.matches(b"ext4"));
        assert!(f.matches(b"xfs"));
        assert!(!f.matches(b"tmpfs"));
    }

    #[test]
    fn type_filter_negate() {
        let f = TypeFilter::parse(b"nosysfs,proc");
        assert!(!f.matches(b"sysfs"));
        assert!(!f.matches(b"proc"));
        assert!(f.matches(b"ext4"));
    }

    #[test]
    fn option_merge_coalesces_rorw() {
        // vfs leads with rw; sb repeats rw — show it once.
        let merged = merge_options(b"rw,noatime", b"rw,errors=continue");
        assert_eq!(merged, b"rw,noatime,errors=continue");
    }

    #[test]
    fn mountpoint_control_chars_sanitized() {
        assert_eq!(sanitize_mountpoint(b"/a\tb"), b"/a?b");
    }
}
