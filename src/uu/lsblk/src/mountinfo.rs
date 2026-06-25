// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! `MOUNTPOINTS` lookup from `/proc/self/mountinfo`.
//!
//! mountinfo keys each mount by its device's `MAJ:MIN`, which is exactly how we
//! join back to a sysfs device. A device may be mounted at several points
//! (bind mounts), so the map holds a list per device.

use std::collections::BTreeMap;
use std::path::Path;

/// `MAJ:MIN` → mount points.
#[derive(Default)]
pub struct MountMap {
    by_dev: BTreeMap<(u32, u32), Vec<String>>,
}

impl MountMap {
    /// Parse `<root>/proc/self/mountinfo`; an unreadable file yields an empty map
    /// (every device then reports no mount point, which is honest).
    pub fn read(root: &Path) -> Self {
        let mut by_dev: BTreeMap<(u32, u32), Vec<String>> = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(root.join("proc/self/mountinfo")) {
            for line in text.lines() {
                if let Some((dev, mountpoint)) = parse_line(line) {
                    by_dev.entry(dev).or_default().push(mountpoint);
                }
            }
        }
        Self { by_dev }
    }

    /// Mount points for a device, in mountinfo order.
    pub fn lookup(&self, maj: u32, min: u32) -> Vec<String> {
        self.by_dev.get(&(maj, min)).cloned().unwrap_or_default()
    }
}

/// A mountinfo line: field 2 is `MAJ:MIN`, field 4 is the (octal-escaped) mount
/// point.
fn parse_line(line: &str) -> Option<((u32, u32), String)> {
    let mut fields = line.split(' ');
    let majmin = fields.nth(2)?; // skip 0,1 then take 2
    let mountpoint = fields.nth(1)?; // now at 3; take next (4)
    let (maj, min) = majmin.split_once(':')?;
    Some(((maj.parse().ok()?, min.parse().ok()?), unescape(mountpoint)))
}

/// mountinfo escapes space/tab/newline/backslash as octal `\NNN`. Each escape
/// decodes to a single *byte*, so we accumulate bytes and decode UTF-8 once at
/// the end — casting `u8 as char` per byte would corrupt any multibyte path
/// (e.g. a UTF-8 mount point like `/mnt/café`).
fn unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            // The three octal digits are ASCII; decode them as a byte.
            if let Ok(oct) = std::str::from_utf8(&bytes[i + 1..i + 4]) {
                if let Ok(v) = u8::from_str_radix(oct, 8) {
                    out.push(v);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_majmin_and_mountpoint() {
        let line = "36 35 98:0 / /mnt rw,noatime shared:1 - ext4 /dev/sda1 rw";
        let (dev, mp) = parse_line(line).unwrap();
        assert_eq!(dev, (98, 0));
        assert_eq!(mp, "/mnt");
    }

    #[test]
    fn unescapes_spaces() {
        let line = "1 1 8:1 / /mnt/with\\040space rw - ext4 /dev/sda1 rw";
        let (_, mp) = parse_line(line).unwrap();
        assert_eq!(mp, "/mnt/with space");
    }

    #[test]
    fn unescape_preserves_utf8() {
        // `/mnt/café` — the é is the raw two-byte UTF-8 sequence \303\251.
        let mp = unescape("/mnt/caf\\303\\251");
        assert_eq!(mp, "/mnt/café");
    }
}
