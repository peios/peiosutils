// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! `/proc/self/mountinfo` parsing — the live mount-state source (§1.2, §11).
//!
//! Fields are kept as opaque bytes; the octal `\\nnn` escapes the kernel emits
//! in paths/options are decoded via [`unmangle`]. Reading is not atomic (§11);
//! [`read`] re-reads once on a detected torn snapshot.

/// One parsed mountinfo line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountEntry {
    pub id: i32,
    pub parent_id: i32,
    pub major: u32,
    pub minor: u32,
    /// Subtree root within the filesystem (`/` for a whole-fs mount; non-`/`
    /// for a bind or btrfs subvolume).
    pub root: Vec<u8>,
    pub mount_point: Vec<u8>,
    /// Per-mount (VFS) options field.
    pub vfs_opts: Vec<u8>,
    /// Optional fields (propagation tags: `shared:N`, `master:N`, …).
    pub optional: Vec<Vec<u8>>,
    pub fstype: Vec<u8>,
    /// Mount source as the kernel recorded it.
    pub source: Vec<u8>,
    /// Superblock options field.
    pub super_opts: Vec<u8>,
}

impl MountEntry {
    /// A bind / subvolume mount: its fs-root is not the filesystem root.
    pub fn is_bind(&self) -> bool {
        self.root != b"/"
    }

    /// True if any optional field marks this mount as shared.
    pub fn is_shared(&self) -> bool {
        self.optional.iter().any(|f| f.starts_with(b"shared:"))
    }
}

/// Read and parse `/proc/self/mountinfo`, re-reading once if the snapshot looks
/// torn (a child line whose parent id never appears — a sign of a concurrent
/// mount/umount mid-read, §11).
pub fn read() -> std::io::Result<Vec<MountEntry>> {
    let first = parse(&std::fs::read("/proc/self/mountinfo")?);
    if looks_torn(&first) {
        return Ok(parse(&std::fs::read("/proc/self/mountinfo")?));
    }
    Ok(first)
}

fn looks_torn(entries: &[MountEntry]) -> bool {
    let ids: std::collections::HashSet<i32> = entries.iter().map(|e| e.id).collect();
    // The root mount's parent is outside the set legitimately; tolerate one.
    entries
        .iter()
        .filter(|e| !ids.contains(&e.parent_id))
        .count()
        > 1
}

/// Parse mountinfo bytes into entries, skipping malformed lines.
pub fn parse(data: &[u8]) -> Vec<MountEntry> {
    data.split(|&b| b == b'\n')
        .filter(|l| !l.is_empty())
        .filter_map(parse_line)
        .collect()
}

fn parse_line(line: &[u8]) -> Option<MountEntry> {
    let fields: Vec<&[u8]> = line.split(|&b| b == b' ').filter(|f| !f.is_empty()).collect();
    // Minimum: 6 pre-fields + separator + 3 post-fields = 10, with ≥0 optional.
    if fields.len() < 10 {
        return None;
    }
    let id = ascii_int(fields[0])?;
    let parent_id = ascii_int(fields[1])?;
    let (major, minor) = {
        let mm = fields[2];
        let i = mm.iter().position(|&b| b == b':')?;
        (ascii_uint(&mm[..i])?, ascii_uint(&mm[i + 1..])?)
    };
    let root = unmangle(fields[3]);
    let mount_point = unmangle(fields[4]);
    let vfs_opts = fields[5].to_vec();

    // Optional fields run until the "-" separator.
    let sep = fields.iter().position(|&f| f == b"-")?;
    let optional: Vec<Vec<u8>> = fields[6..sep].iter().map(|f| f.to_vec()).collect();
    let post = &fields[sep + 1..];
    if post.len() < 3 {
        return None;
    }
    Some(MountEntry {
        id,
        parent_id,
        major,
        minor,
        root,
        mount_point,
        vfs_opts,
        optional,
        fstype: post[0].to_vec(),
        source: unmangle(post[1]),
        super_opts: post[2].to_vec(),
    })
}

fn ascii_int(b: &[u8]) -> Option<i32> {
    std::str::from_utf8(b).ok()?.parse().ok()
}

fn ascii_uint(b: &[u8]) -> Option<u32> {
    std::str::from_utf8(b).ok()?.parse().ok()
}

/// Find the entry mounted at exactly `mount_point` (the last one wins — the
/// topmost over-mount).
pub fn at_mountpoint<'a>(entries: &'a [MountEntry], mount_point: &[u8]) -> Option<&'a MountEntry> {
    entries.iter().rev().find(|e| e.mount_point == mount_point)
}

/// All entries whose source matches `source`.
pub fn by_source<'a>(entries: &'a [MountEntry], source: &[u8]) -> Vec<&'a MountEntry> {
    entries.iter().filter(|e| e.source == source).collect()
}

/// Decode mountinfo octal escapes (`\040`→space, `\011`→tab, `\012`→newline,
/// `\134`→backslash, and any `\nnn`).
pub fn unmangle(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        // A `\nnn` escape needs the backslash plus three octal digits.
        if s[i] == b'\\' && i + 4 <= s.len() {
            let d = &s[i + 1..i + 4];
            if d.iter().all(|&b| (b'0'..=b'7').contains(&b)) {
                let v = (d[0] - b'0') * 64 + (d[1] - b'0') * 8 + (d[2] - b'0');
                out.push(v);
                i += 4;
                continue;
            }
        }
        out.push(s[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = b"\
36 35 98:0 / /mnt rw,noatime shared:1 - ext4 /dev/sda1 rw,errors=continue
37 36 98:0 /sub /mnt/bind rw,relatime - ext4 /dev/sda1 rw
38 36 0:42 / /mnt/space\\040dir rw - tmpfs tmpfs rw
";

    #[test]
    fn parses_fields() {
        let e = parse(SAMPLE);
        assert_eq!(e.len(), 3);
        assert_eq!(e[0].id, 36);
        assert_eq!(e[0].parent_id, 35);
        assert_eq!(e[0].major, 98);
        assert_eq!(e[0].root, b"/");
        assert_eq!(e[0].mount_point, b"/mnt");
        assert_eq!(e[0].fstype, b"ext4");
        assert_eq!(e[0].source, b"/dev/sda1");
        assert!(e[0].is_shared());
        assert!(!e[0].is_bind());
    }

    #[test]
    fn detects_bind() {
        let e = parse(SAMPLE);
        assert!(e[1].is_bind()); // root /sub
        assert_eq!(e[1].root, b"/sub");
    }

    #[test]
    fn unmangles_mountpoint() {
        let e = parse(SAMPLE);
        assert_eq!(e[2].mount_point, b"/mnt/space dir");
    }

    #[test]
    fn lookup_helpers() {
        let e = parse(SAMPLE);
        assert_eq!(at_mountpoint(&e, b"/mnt").unwrap().id, 36);
        assert_eq!(by_source(&e, b"/dev/sda1").len(), 2);
    }

    #[test]
    fn unmangle_handles_escapes() {
        assert_eq!(unmangle(b"a\\040b"), b"a b");
        assert_eq!(unmangle(b"\\134"), b"\\");
        assert_eq!(unmangle(b"plain"), b"plain");
    }
}
