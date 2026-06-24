// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Mount-namespace entry for `-N` (§13).
//!
//! The spec orders source/UUID resolution and canonicalization in the *origin*
//! namespace, then `setns()` into the target mount ns before the mount
//! syscall. pkm's mount-namespace authz is not yet fully implemented (it falls
//! back to the privilege→capability map), so cross-ns behaviour may be clumsy —
//! surfaced honestly (§1.5), not papered over.

use std::ffi::OsStr;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;

use crate::error::{MountError, Result};

/// `setns()` the calling thread into the mount namespace named by `ns`, which
/// may be a PID (→ `/proc/<pid>/ns/mnt`), an ns-file path, or a named ns.
pub fn enter(ns: &[u8]) -> Result<()> {
    let path = resolve(ns);
    let f = File::open(OsStr::from_bytes(&path)).map_err(|e| {
        MountError::from_syscall("open namespace", e)
    })?;
    // SAFETY: setns with a valid ns fd and the mount-ns type.
    let rc = unsafe { libc::setns(f.as_raw_fd(), libc::CLONE_NEWNS) };
    if rc < 0 {
        return Err(MountError::from_syscall("setns", std::io::Error::last_os_error()));
    }
    Ok(())
}

/// Resolve the `-N` argument to an ns-file path. All-digits → a PID's mount ns;
/// an absolute path → used directly; otherwise a named ns under the conventional
/// `/run/mnt/namespaces/` directory (util-linux's named-ns location).
fn resolve(ns: &[u8]) -> Vec<u8> {
    if !ns.is_empty() && ns.iter().all(u8::is_ascii_digit) {
        let mut p = b"/proc/".to_vec();
        p.extend_from_slice(ns);
        p.extend_from_slice(b"/ns/mnt");
        return p;
    }
    if ns.starts_with(b"/") {
        return ns.to_vec();
    }
    let mut p = b"/run/mnt/namespaces/".to_vec();
    p.extend_from_slice(ns);
    p
}

#[cfg(test)]
mod tests {
    use super::resolve;

    #[test]
    fn pid_form() {
        assert_eq!(resolve(b"1234"), b"/proc/1234/ns/mnt");
    }
    #[test]
    fn path_form() {
        assert_eq!(resolve(b"/proc/9/ns/mnt"), b"/proc/9/ns/mnt");
    }
    #[test]
    fn named_form() {
        assert_eq!(resolve(b"work"), b"/run/mnt/namespaces/work");
    }
}
