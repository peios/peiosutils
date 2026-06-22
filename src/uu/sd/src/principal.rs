// Principal parsing — input grammar for SIDs.
//
// Acceptance order:
//   1. `@self`                  — running token's user SID (resolved at parse)
//   2. `@owner`                 — PRINCIPAL_SELF (S-1-5-10), substituted by
//                                  kernel at access-check time to the SD's owner
//   3. Well-known label         — `Everyone`, `LocalSystem`, ...
//   4. SDDL two-letter alias    — `WD`, `SY`, `BA`, ...
//   5. Raw SID                  — `S-1-5-32-544`
//
// Domain-relative aliases (DA/DG/DU/...) parse-error with a hint about the
// missing identity backend; same message the SDDL parser uses.
//
// Migrated to the `peios` crate: SIDs are now `peios::security::Sid` (an owned,
// `Copy` inline buffer that derefs to `SidRef`). Construction is via
// `str::parse::<Sid>()` (which handles "S-1-…" literals and SDDL aliases) or
// `Sid::build` for raw authority/sub-authority assembly; the old
// `WellKnownSid::to_sid` table is re-expressed as canonical SID strings.

use crate::error::{Error, Result};
use peios::security::Sid;
use peios::token::{Token, TokenAccess};

/// MS-DTYP §2.4.2.4 — `PRINCIPAL_SELF` (S-1-5-10). The kernel substitutes
/// the SD's owner SID at access-check time when it sees this in an ACE.
fn principal_self() -> Sid {
    Sid::build(5, &[10]).expect("S-1-5-10 always encodes")
}

/// Well-known short labels (Peios canonical) and SDDL two-letter aliases, as
/// canonical SID strings. (The new `peios::security::WellKnown` enum lacks the
/// integrity-label and `BUILTIN\Users` variants, so a string table — fed to
/// `str::parse::<Sid>()` — is used uniformly.)
const LABELED_SIDS: &[(&str, &str)] = &[
    ("Null", "S-1-0-0"),
    ("Everyone", "S-1-1-0"),
    ("World", "S-1-1-0"),
    ("Anonymous", "S-1-5-7"),
    ("AuthenticatedUsers", "S-1-5-11"),
    ("Authenticated", "S-1-5-11"),
    ("LocalSystem", "S-1-5-18"),
    ("System", "S-1-5-18"),
    ("LocalService", "S-1-5-19"),
    ("NetworkService", "S-1-5-20"),
    ("Administrators", "S-1-5-32-544"),
    ("Users", "S-1-5-32-545"),
    ("UntrustedIl", "S-1-16-0"),
    ("LowIl", "S-1-16-4096"),
    ("MediumIl", "S-1-16-8192"),
    ("MediumPlusIl", "S-1-16-8448"),
    ("HighIl", "S-1-16-12288"),
    ("SystemIl", "S-1-16-16384"),
    ("ProtectedProcessIl", "S-1-16-20480"),
];

const SDDL_ALIASES: &[(&str, &str)] = &[
    ("WD", "S-1-1-0"),
    ("AN", "S-1-5-7"),
    ("AU", "S-1-5-11"),
    ("SY", "S-1-5-18"),
    ("LS", "S-1-5-19"),
    ("NS", "S-1-5-20"),
    ("BA", "S-1-5-32-544"),
    ("BU", "S-1-5-32-545"),
    ("LW", "S-1-16-4096"),
    ("ME", "S-1-16-8192"),
    ("MP", "S-1-16-8448"),
    ("HI", "S-1-16-12288"),
    ("SI", "S-1-16-16384"),
];

const DOMAIN_RELATIVE: &[&str] = &[
    "DA", "DG", "DU", "DD", "DC", "LA", "LG", "SA", "EA", "RO", "CA", "PA", "CN", "RS", "RU",
];

fn sid_from_str(s: &str) -> Result<Sid> {
    s.parse::<Sid>()
        .map_err(|e| Error::Invalid(format!("SID `{s}` did not parse: {e}")))
}

/// Parse a principal string to a SID.
pub fn parse(s: &str) -> Result<Sid> {
    let t = s.trim();
    if t.is_empty() {
        return Err(Error::Usage("empty principal".into()));
    }

    if t == "@self" {
        return resolve_self();
    }
    if t == "@owner" {
        return Ok(principal_self());
    }
    if let Some(rest) = t.strip_prefix("user:") {
        return Err(Error::Usage(format!(
            "principal `user:{rest}` requires an identity backend; none configured"
        )));
    }
    if let Some(rest) = t.strip_prefix("group:") {
        return Err(Error::Usage(format!(
            "principal `group:{rest}` requires an identity backend; none configured"
        )));
    }

    for &(name, sid) in LABELED_SIDS {
        if name.eq_ignore_ascii_case(t) {
            return sid_from_str(sid);
        }
    }
    for &(code, sid) in SDDL_ALIASES {
        if code == t {
            return sid_from_str(sid);
        }
    }
    if DOMAIN_RELATIVE.contains(&t) {
        return Err(Error::Usage(format!(
            "domain-relative alias `{t}` cannot be resolved without an identity backend"
        )));
    }
    if t.starts_with("S-") || t.starts_with("s-") {
        return parse_raw_sid(t);
    }
    Err(Error::Usage(format!("unrecognised principal `{t}`")))
}

/// Parse an `S-1-...` literal.
fn parse_raw_sid(s: &str) -> Result<Sid> {
    // The new `Sid: FromStr` parses the canonical `S-R-A-…` form directly,
    // including hex (`0x…`) authorities. Preserve the old, friendlier
    // diagnostics by validating the shape first, then delegating.
    let mut parts = s.split('-');
    let prefix = parts.next();
    if !matches!(prefix, Some("S") | Some("s")) {
        return Err(Error::Usage(format!("not a raw SID: `{s}`")));
    }
    parts
        .next()
        .ok_or_else(|| Error::Usage(format!("SID `{s}` missing revision")))?
        .parse::<u8>()
        .map_err(|_| Error::Usage(format!("SID `{s}` revision is not a number")))?;
    let authority_str = parts
        .next()
        .ok_or_else(|| Error::Usage(format!("SID `{s}` missing authority")))?;
    if let Some(hex) = authority_str.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
            .map_err(|_| Error::Usage(format!("SID `{s}` authority is not a number")))?;
    } else {
        authority_str
            .parse::<u64>()
            .map_err(|_| Error::Usage(format!("SID `{s}` authority is not a number")))?;
    }
    for sub in parts {
        sub.parse::<u32>()
            .map_err(|_| Error::Usage(format!("SID `{s}` has a non-numeric subauthority")))?;
    }
    sid_from_str(s)
}

/// Open the caller's own token and read its user SID.
fn resolve_self() -> Result<Sid> {
    let tok = Token::open_self(false, TokenAccess::QUERY)
        .map_err(|e| Error::Invalid(format!("open self token: {e}")))?;
    let user = tok
        .user()
        .map_err(|e| Error::Invalid(format!("read user SID: {e}")))?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_known_labels() {
        assert_eq!(parse("Everyone").unwrap(), sid_from_str("S-1-1-0").unwrap());
        assert_eq!(parse("System").unwrap(), sid_from_str("S-1-5-18").unwrap());
        assert_eq!(
            parse("Administrators").unwrap(),
            sid_from_str("S-1-5-32-544").unwrap()
        );
    }

    #[test]
    fn parses_sddl_aliases() {
        assert_eq!(parse("SY").unwrap(), sid_from_str("S-1-5-18").unwrap());
        assert_eq!(parse("BA").unwrap(), sid_from_str("S-1-5-32-544").unwrap());
    }

    #[test]
    fn parses_raw_sid() {
        let sid = parse("S-1-5-32-544").unwrap();
        assert_eq!(sid, sid_from_str("S-1-5-32-544").unwrap());
    }

    #[test]
    fn principal_self_is_s_1_5_10() {
        assert_eq!(parse("@owner").unwrap(), Sid::build(5, &[10]).unwrap());
    }

    #[test]
    fn domain_relative_rejected() {
        assert!(parse("DA").is_err());
    }

    #[test]
    fn user_prefix_reserved() {
        assert!(parse("user:alice").is_err());
    }
}
