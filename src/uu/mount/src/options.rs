// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! `-o` option-language parsing and the §6 category partitioning.
//!
//! Operates on opaque bytes (§1.8): option *names* and structural keywords are
//! ASCII, but `key=value` values, paths and SDDL pass through losslessly as
//! `Vec<u8>`. The `key=value` split is on the **first** `=`; `key="..."`
//! quoting protects embedded commas.
//!
//! The partitioner sorts every token into one of the §6 categories:
//!   A per-mount VFS attrs → `MOUNT_ATTR_*` masks (`attr_set`/`attr_clr`)
//!   B superblock flags
//!   C fs-specific opaque passthrough
//!   D userspace-only (loop/offset/sizelimit, meta-verbs, propagation, defaults)
//!   E KACS mount policy
//!   F functional `X-mount.*`
//! Cut options (§3: idmap, POSIX owner/group/mode) are rejected as a clear
//! usage error rather than silently forwarded.

use crate::error::{MountError, Result};

// MOUNT_ATTR atime mode values within the MOUNT_ATTR__ATIME mask.
const ATIME_RELATIME: u64 = libc::MOUNT_ATTR_RELATIME; // 0
const ATIME_NOATIME: u64 = libc::MOUNT_ATTR_NOATIME; // 0x10
const ATIME_STRICTATIME: u64 = libc::MOUNT_ATTR_STRICTATIME; // 0x20

/// A superblock flag (Category B). Routing to the exact new-API plumbing is
/// resolved in the executor; this names the normative behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbFlag {
    Synchronous,
    Dirsync,
    Lazytime,
    IVersion,
    Silent,
}

/// A mount-propagation change (Category D / `--make-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropKind {
    Shared,
    Slave,
    Private,
    Unbindable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PropChange {
    pub kind: PropKind,
    pub recursive: bool,
}

/// A KACS mount-policy kind (Category E).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyKind {
    DenyMissing,
    SynthEphemeral,
    SynthPersist,
}

/// Loop-device request (Category D / §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopRequest {
    /// `-o loop` — auto-allocate a free device.
    Auto,
    /// `loop=/dev/loopN` — use the named device.
    Device(Vec<u8>),
}

/// The read-only scope qualifier on `ro`/`rw` (`=vfs`/`=fs`/`=recursive`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoScope {
    Vfs,
    Fs,
    Recursive,
}

/// Fully partitioned `-o` options.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedOptions {
    // Category A — per-mount VFS attributes.
    pub attr_set: u64,
    pub attr_clr: u64,
    /// Any `ro=recursive`/`rw=recursive` was seen → apply attrs with AT_RECURSIVE.
    pub recursive_attr: bool,
    /// `ro=fs`/`rw=fs` — a superblock-level read-only request (Some(true)=ro).
    pub sb_rdonly: Option<bool>,

    // Category B — superblock flags: (flag, enabled).
    pub sb_flags: Vec<(SbFlag, bool)>,

    // Category C — fs-specific opaque params: (key, Some(value) | None).
    pub fs_params: Vec<(Vec<u8>, Option<Vec<u8>>)>,

    // Category D — userspace-only.
    pub meta_bind: bool,
    pub meta_rbind: bool,
    pub meta_move: bool,
    pub meta_remount: bool,
    pub loop_request: Option<LoopRequest>,
    pub offset: Option<u64>,
    pub sizelimit: Option<u64>,
    pub propagation: Vec<PropChange>,

    // Category E — KACS mount policy.
    pub policy: Option<PolicyKind>,

    // Category F — functional X-mount.*.
    pub mkdir: Option<u32>,
    pub subdir: Option<Vec<u8>>,
    pub noloop: bool,
    pub auto_fstypes: Option<Vec<u8>>,
    pub nocanon_source: bool,
    pub nocanon_target: bool,

    /// Non-fatal diagnostics (e.g. deprecated `mand`), surfaced under `-v`.
    pub notes: Vec<String>,

    /// A top-level `rw` token was given (not via `defaults`) — disables the
    /// auto-ro-fallback (§2.5.1), distinct from the mere absence of `ro`.
    pub explicit_rw: bool,
    /// A top-level `ro` token was given — request read-only directly.
    pub explicit_ro: bool,

    /// atime mode chosen within the _ATIME mask, if any was specified.
    atime_mode: Option<u64>,
}

impl ParsedOptions {
    /// Parse and partition a `-o` byte string (all `-o` occurrences joined by
    /// commas).
    pub fn parse(spec: &[u8]) -> Result<Self> {
        let mut out = Self::default();
        let tokens = tokenize(spec);
        // Explicit ro/rw is a *top-level* token (a `defaults` token expands to
        // `rw` internally but does not count as explicit, §2.5.1).
        out.explicit_rw = tokens.iter().any(|(k, _)| k == b"rw");
        out.explicit_ro = tokens.iter().any(|(k, _)| k == b"ro");
        out.apply_tokens(&tokens)?;
        out.finish_atime();
        Ok(out)
    }

    /// Fold the chosen atime mode into the attr masks (deferred so the
    /// last-specified atime token wins and `defaults` expansion composes).
    fn finish_atime(&mut self) {
        if let Some(mode) = self.atime_mode {
            self.attr_clr |= libc::MOUNT_ATTR__ATIME;
            self.attr_set = (self.attr_set & !libc::MOUNT_ATTR__ATIME) | mode;
        }
    }

    fn apply_tokens(&mut self, tokens: &[(Vec<u8>, Option<Vec<u8>>)]) -> Result<()> {
        for (key, value) in tokens {
            self.apply_one(key, value.as_deref())?;
        }
        Ok(())
    }

    fn apply_one(&mut self, key: &[u8], value: Option<&[u8]>) -> Result<()> {
        // Option names are ASCII; match on a lossy view but keep `value` bytes.
        let name = String::from_utf8_lossy(key);
        let name = name.as_ref();

        // `defaults` expands to rw,suid,dev,exec,async (§6.6); later tokens
        // override, so we just replay the expansion through this same path.
        if name == "defaults" {
            for tok in [b"rw".as_slice(), b"suid", b"dev", b"exec", b"async"] {
                self.apply_one(tok, None)?;
            }
            return Ok(());
        }

        // Category A — ro/rw carry an optional =vfs|fs|recursive qualifier.
        if name == "ro" || name == "rw" {
            return self.apply_ro_rw(name == "ro", value);
        }

        // Category A — simple per-mount attr bits.
        if let Some(handled) = self.try_attr(name) {
            return handled;
        }

        // Category B — superblock flags.
        if let Some(handled) = self.try_sb_flag(name) {
            return handled;
        }

        // Category D — meta-verbs, loop, propagation.
        if let Some(handled) = self.try_userspace(name, value)? {
            return Ok(handled);
        }

        // Category E — KACS policy.
        if name == "policy" {
            let v = value.ok_or_else(|| usage("policy= requires a value"))?;
            self.policy = Some(parse_policy(v)?);
            return Ok(());
        }

        // Category F — functional X-mount.*, and the cut ones.
        if let Some(rest) = name.strip_prefix("X-mount.") {
            return self.apply_xmount(rest, value);
        }

        // Otherwise: Category C — opaque fs-specific parameter.
        self.fs_params
            .push((key.to_vec(), value.map(<[u8]>::to_vec)));
        Ok(())
    }

    fn apply_ro_rw(&mut self, ro: bool, value: Option<&[u8]>) -> Result<()> {
        let scope = match value {
            None => RoScope::Vfs,
            Some(b"vfs") => RoScope::Vfs,
            Some(b"fs") => RoScope::Fs,
            Some(b"recursive") => RoScope::Recursive,
            Some(other) => {
                return Err(usage(&format!(
                    "invalid read-only scope: {}",
                    String::from_utf8_lossy(other)
                )));
            }
        };
        match scope {
            RoScope::Fs => self.sb_rdonly = Some(ro),
            RoScope::Recursive => {
                self.recursive_attr = true;
                self.set_or_clr(libc::MOUNT_ATTR_RDONLY, ro);
            }
            RoScope::Vfs => self.set_or_clr(libc::MOUNT_ATTR_RDONLY, ro),
        }
        Ok(())
    }

    /// Category A simple bits. Returns `Some(Ok)` if `name` is a per-mount attr.
    fn try_attr(&mut self, name: &str) -> Option<Result<()>> {
        match name {
            "nosuid" => self.set_or_clr(libc::MOUNT_ATTR_NOSUID, true),
            "suid" => self.set_or_clr(libc::MOUNT_ATTR_NOSUID, false),
            "nodev" => self.set_or_clr(libc::MOUNT_ATTR_NODEV, true),
            "dev" => self.set_or_clr(libc::MOUNT_ATTR_NODEV, false),
            "noexec" => self.set_or_clr(libc::MOUNT_ATTR_NOEXEC, true),
            "exec" => self.set_or_clr(libc::MOUNT_ATTR_NOEXEC, false),
            "nodiratime" => self.set_or_clr(libc::MOUNT_ATTR_NODIRATIME, true),
            "diratime" => self.set_or_clr(libc::MOUNT_ATTR_NODIRATIME, false),
            "nosymfollow" => self.set_or_clr(libc::MOUNT_ATTR_NOSYMFOLLOW, true),
            // atime modes share the _ATIME mask; last one wins (finish_atime).
            "noatime" => self.atime_mode = Some(ATIME_NOATIME),
            "atime" => self.atime_mode = Some(ATIME_RELATIME),
            "relatime" => self.atime_mode = Some(ATIME_RELATIME),
            "norelatime" => self.atime_mode = Some(ATIME_STRICTATIME),
            "strictatime" => self.atime_mode = Some(ATIME_STRICTATIME),
            "nostrictatime" => self.atime_mode = Some(ATIME_RELATIME),
            _ => return None,
        }
        Some(Ok(()))
    }

    fn try_sb_flag(&mut self, name: &str) -> Option<Result<()>> {
        let (flag, on) = match name {
            "sync" => (SbFlag::Synchronous, true),
            "async" => (SbFlag::Synchronous, false),
            "dirsync" => (SbFlag::Dirsync, true),
            "lazytime" => (SbFlag::Lazytime, true),
            "nolazytime" => (SbFlag::Lazytime, false),
            "iversion" => (SbFlag::IVersion, true),
            "noiversion" => (SbFlag::IVersion, false),
            "silent" => (SbFlag::Silent, true),
            "loud" => (SbFlag::Silent, false),
            // mand/nomand removed from 7.0.9: accept + ignore with a note.
            "mand" | "nomand" => {
                self.notes
                    .push(format!("option '{name}' is obsolete (removed from the kernel) — ignored"));
                return Some(Ok(()));
            }
            _ => return None,
        };
        self.sb_flags.push((flag, on));
        Some(Ok(()))
    }

    /// Category D. `Ok(Some(()))` = handled; `Ok(None)` = not a Category-D token.
    fn try_userspace(&mut self, name: &str, value: Option<&[u8]>) -> Result<Option<()>> {
        match name {
            "bind" => self.meta_bind = true,
            "rbind" => self.meta_rbind = true,
            "move" => self.meta_move = true,
            "remount" => self.meta_remount = true,
            "loop" => {
                self.loop_request = Some(match value {
                    Some(dev) => LoopRequest::Device(dev.to_vec()),
                    None => LoopRequest::Auto,
                });
            }
            "offset" => self.offset = Some(parse_u64(name, value)?),
            "sizelimit" => self.sizelimit = Some(parse_u64(name, value)?),
            "shared" | "slave" | "private" | "unbindable" | "rshared" | "rslave" | "rprivate"
            | "runbindable" => {
                let recursive = matches!(name, "rshared" | "rslave" | "rprivate" | "runbindable");
                let base = if recursive { &name[1..] } else { name };
                let kind = match base {
                    "shared" => PropKind::Shared,
                    "slave" => PropKind::Slave,
                    "private" => PropKind::Private,
                    "unbindable" => PropKind::Unbindable,
                    _ => unreachable!(),
                };
                self.propagation.push(PropChange { kind, recursive });
            }
            _ => return Ok(None),
        }
        Ok(Some(()))
    }

    fn apply_xmount(&mut self, rest: &str, value: Option<&[u8]>) -> Result<()> {
        match rest {
            "mkdir" => self.mkdir = Some(parse_mode(value)?),
            "subdir" => {
                self.subdir =
                    Some(value.ok_or_else(|| usage("X-mount.subdir requires a directory"))?.to_vec());
            }
            "noloop" => self.noloop = true,
            "auto-fstypes" => {
                self.auto_fstypes = Some(
                    value
                        .ok_or_else(|| usage("X-mount.auto-fstypes requires a list"))?
                        .to_vec(),
                );
            }
            "nocanonicalize" => match value {
                None => {
                    self.nocanon_source = true;
                    self.nocanon_target = true;
                }
                Some(b"source") => self.nocanon_source = true,
                Some(b"target") => self.nocanon_target = true,
                Some(other) => {
                    return Err(usage(&format!(
                        "X-mount.nocanonicalize: expected 'source' or 'target', got {}",
                        String::from_utf8_lossy(other)
                    )));
                }
            },
            // Cut permanently (§3): no peios analog — reject honestly.
            "idmap" => {
                return Err(usage(
                    "X-mount.idmap is not supported on peios — add a SID/ACE to the security descriptor instead",
                ));
            }
            "owner" | "group" | "mode" => {
                return Err(usage(&format!(
                    "X-mount.{rest} is not supported on peios — ownership is SID/SD-based, not POSIX uid/gid/mode",
                )));
            }
            other => {
                return Err(usage(&format!("unknown X-mount option: X-mount.{other}")));
            }
        }
        Ok(())
    }

    fn set_or_clr(&mut self, bit: u64, set: bool) {
        if set {
            self.attr_set |= bit;
            self.attr_clr &= !bit;
        } else {
            self.attr_clr |= bit;
            self.attr_set &= !bit;
        }
    }

    /// The combined `MOUNT_ATTR_*` mask for the `fsmount` (fresh-mount) path,
    /// where there is nothing to clear.
    pub fn fsmount_attr_flags(&self) -> u32 {
        self.attr_set as u32
    }

    /// True if this option set names any superblock-level change (Category B,
    /// `ro=fs`, or fs-specific params) — used to guard bind remounts (§6.7).
    pub fn touches_superblock(&self) -> bool {
        !self.sb_flags.is_empty() || self.sb_rdonly.is_some() || !self.fs_params.is_empty()
    }
}

// ---------------------------------------------------------------------------
// tokenizer + small value parsers
// ---------------------------------------------------------------------------

/// Split a `-o` byte string into `(key, value?)` tokens. Commas separate
/// tokens, but a `key="..."` double-quoted value protects embedded commas; the
/// `key=value` split is on the first `=`. Empty tokens are skipped.
fn tokenize(spec: &[u8]) -> Vec<(Vec<u8>, Option<Vec<u8>>)> {
    let mut out = Vec::new();
    let mut cur: Vec<u8> = Vec::new();
    let mut in_quotes = false;
    for &b in spec {
        match b {
            b'"' => in_quotes = !in_quotes, // drop the quote chars themselves
            b',' if !in_quotes => {
                push_token(&mut out, &cur);
                cur.clear();
            }
            _ => cur.push(b),
        }
    }
    push_token(&mut out, &cur);
    out
}

fn push_token(out: &mut Vec<(Vec<u8>, Option<Vec<u8>>)>, tok: &[u8]) {
    let tok = trim(tok);
    if tok.is_empty() {
        return;
    }
    match tok.iter().position(|&b| b == b'=') {
        Some(i) => out.push((tok[..i].to_vec(), Some(tok[i + 1..].to_vec()))),
        None => out.push((tok.to_vec(), None)),
    }
}

fn trim(b: &[u8]) -> &[u8] {
    let start = b.iter().position(|c| !c.is_ascii_whitespace()).unwrap_or(b.len());
    let end = b.iter().rposition(|c| !c.is_ascii_whitespace()).map_or(start, |i| i + 1);
    &b[start..end]
}

fn parse_policy(v: &[u8]) -> Result<PolicyKind> {
    match v {
        b"deny-missing" => Ok(PolicyKind::DenyMissing),
        b"synth-ephemeral" => Ok(PolicyKind::SynthEphemeral),
        b"synth-persist" => Ok(PolicyKind::SynthPersist),
        b"unmanaged" => Err(usage("policy=unmanaged is not user-settable")),
        other => Err(usage(&format!(
            "unknown mount policy: {} (expected deny-missing|synth-ephemeral|synth-persist)",
            String::from_utf8_lossy(other)
        ))),
    }
}

/// Parse a numeric option value, accepting an optional IEC/SI size suffix
/// (K/M/G/T, optionally with `iB`/`B`). util-linux uses `strtosize`.
fn parse_u64(name: &str, value: Option<&[u8]>) -> Result<u64> {
    let v = value.ok_or_else(|| usage(&format!("{name}= requires a value")))?;
    let s = std::str::from_utf8(v).map_err(|_| usage(&format!("{name}: not a number")))?;
    let s = s.trim();
    let (num, mult) = split_suffix(s);
    let base: u64 = num
        .parse()
        .map_err(|_| usage(&format!("{name}: not a number: {s}")))?;
    base.checked_mul(mult)
        .ok_or_else(|| usage(&format!("{name}: value overflow: {s}")))
}

fn split_suffix(s: &str) -> (&str, u64) {
    let units = [
        ("KiB", 1u64 << 10), ("MiB", 1 << 20), ("GiB", 1 << 30), ("TiB", 1u64 << 40),
        ("KB", 1000), ("MB", 1_000_000), ("GB", 1_000_000_000), ("TB", 1_000_000_000_000),
        ("K", 1 << 10), ("M", 1 << 20), ("G", 1 << 30), ("T", 1u64 << 40),
    ];
    for (suf, mult) in units {
        if let Some(stripped) = s.strip_suffix(suf) {
            return (stripped.trim(), mult);
        }
    }
    (s, 1)
}

fn parse_mode(value: Option<&[u8]>) -> Result<u32> {
    match value {
        None => Ok(0o755),
        Some(v) => {
            let s = std::str::from_utf8(v).map_err(|_| usage("mkdir mode: not a number"))?;
            u32::from_str_radix(s.trim_start_matches("0o").trim(), 8)
                .map_err(|_| usage(&format!("mkdir mode: invalid octal: {s}")))
        }
    }
}

fn usage(msg: &str) -> MountError {
    MountError::Usage(msg.to_string())
}

#[cfg(test)]
mod tests;
