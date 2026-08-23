// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! The identity a Peios process is really running under.
//!
//! POSIX asks `getuid()` and gets a number. Under KACS that number is a
//! *projection* of the token's user SID — real, stable, and what every Linux
//! program sees, but not what decides access. The two can disagree in ways the
//! projection has no room to express: a filtered `Limited` token carries the
//! same user SID and the same group numbers as the elevated token it came
//! from, differing only in `SE_GROUP_*` attributes POSIX cannot represent. So
//! `id` on a limited token would otherwise print output byte-identical to the
//! elevated case — not a missing field, but an identical answer to two
//! different questions.
//!
//! This module renders the half the projection drops, into the field GNU
//! already reserves for it: `context=`, which GNU `id` fills with the SELinux
//! context when SELinux is enabled. Peios' security context is the token, so
//! that is what goes there.
//!
//! # Format
//!
//! `<user-sid>:<integrity>` — for example
//! `S-1-5-21-1004336348-1177238915-682003330-1001:High`.
//!
//! Colon-separated because that is the shape a reader expects of a security
//! context, and SIDs contain no colons, so the split is unambiguous. Integrity
//! is part of it rather than a separate field because the SID alone does not
//! separate an elevated token from its filtered counterpart — both carry the
//! same user SID, and the integrity level is precisely what differs.
//!
//! # Absence is normal, and must never be an error
//!
//! Every entry point returns `None` rather than an error when the token cannot
//! be read. These utilities run during builds and on non-Peios kernels where
//! the KACS syscalls do not exist at all. A missing context must omit a field,
//! never fail a command — `id` running in a build container has to keep
//! working.

use peios::security::{IntegrityLevel, Sid};
use peios::token::{Token, TokenAccess};

/// The name of an integrity level, or its raw RID if it is not one of the five
/// named ones. KACS permits any value in the field; only these are named.
fn integrity_name(level: IntegrityLevel) -> String {
    let rid = level.rid();
    for (named, name) in [
        (IntegrityLevel::UNTRUSTED, "Untrusted"),
        (IntegrityLevel::LOW, "Low"),
        (IntegrityLevel::MEDIUM, "Medium"),
        (IntegrityLevel::HIGH, "High"),
        (IntegrityLevel::SYSTEM, "System"),
    ] {
        if named.rid() == rid {
            return name.to_string();
        }
    }
    rid.to_string()
}

/// Open the calling process's token for reading.
///
/// `real` selects the **primary** token rather than the effective one, which
/// is what `id -r` asks for: under KACS the real/effective split is the
/// impersonation boundary, not the setuid one (setuid is cosmetic here).
fn open(real: bool) -> Option<Token> {
    Token::open_self(real, TokenAccess::QUERY).ok()
}

/// The calling process's security context, or `None` when there is no token to
/// read.
pub fn context(real: bool) -> Option<String> {
    let token = open(real)?;
    let sid = token.user().ok()?;
    // Integrity is a separate query and can fail on its own. The SID is still
    // worth printing without it, so a failure here degrades the field rather
    // than dropping it.
    match token.integrity() {
        Ok(level) => Some(format!("{sid}:{}", integrity_name(level))),
        Err(_) => Some(sid.to_string()),
    }
}

/// The token's user SID alone — the authoritative answer to "who is this
/// process", as against the uid, which is its projection.
pub fn user_sid(real: bool) -> Option<Sid> {
    open(real)?.user().ok()
}
