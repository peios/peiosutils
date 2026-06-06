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
// missing identity backend; same message libp-sd's SDDL parser uses.

use crate::error::{Error, Result};
use libp_sd::WellKnownSid;
use libp_token::Token;
use libp_sd::Sid;

/// MS-DTYP §2.4.2.4 — `PRINCIPAL_SELF` (S-1-5-10). The kernel substitutes
/// the SD's owner SID at access-check time when it sees this in an ACE.
fn principal_self() -> Sid {
    Sid::new(1, 5, vec![10])
}

/// Well-known short labels (Peios canonical) and SDDL two-letter aliases.
const LABELED_SIDS: &[(&str, WellKnownSid)] = &[
    ("Null", WellKnownSid::Null),
    ("Everyone", WellKnownSid::Everyone),
    ("World", WellKnownSid::Everyone),
    ("Anonymous", WellKnownSid::Anonymous),
    ("AuthenticatedUsers", WellKnownSid::AuthenticatedUsers),
    ("Authenticated", WellKnownSid::AuthenticatedUsers),
    ("LocalSystem", WellKnownSid::LocalSystem),
    ("System", WellKnownSid::LocalSystem),
    ("LocalService", WellKnownSid::LocalService),
    ("NetworkService", WellKnownSid::NetworkService),
    ("Administrators", WellKnownSid::BuiltinAdministrators),
    ("Users", WellKnownSid::BuiltinUsers),
    ("UntrustedIl", WellKnownSid::UntrustedIl),
    ("LowIl", WellKnownSid::LowIl),
    ("MediumIl", WellKnownSid::MediumIl),
    ("MediumPlusIl", WellKnownSid::MediumPlusIl),
    ("HighIl", WellKnownSid::HighIl),
    ("SystemIl", WellKnownSid::SystemIl),
    ("ProtectedProcessIl", WellKnownSid::ProtectedProcessIl),
];

const SDDL_ALIASES: &[(&str, WellKnownSid)] = &[
    ("WD", WellKnownSid::Everyone),
    ("AN", WellKnownSid::Anonymous),
    ("AU", WellKnownSid::AuthenticatedUsers),
    ("SY", WellKnownSid::LocalSystem),
    ("LS", WellKnownSid::LocalService),
    ("NS", WellKnownSid::NetworkService),
    ("BA", WellKnownSid::BuiltinAdministrators),
    ("BU", WellKnownSid::BuiltinUsers),
    ("LW", WellKnownSid::LowIl),
    ("ME", WellKnownSid::MediumIl),
    ("MP", WellKnownSid::MediumPlusIl),
    ("HI", WellKnownSid::HighIl),
    ("SI", WellKnownSid::SystemIl),
];

const DOMAIN_RELATIVE: &[&str] = &[
    "DA", "DG", "DU", "DD", "DC", "LA", "LG", "SA", "EA", "RO", "CA", "PA", "CN", "RS", "RU",
];

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
            return Ok(sid.to_sid());
        }
    }
    for &(code, sid) in SDDL_ALIASES {
        if code == t {
            return Ok(sid.to_sid());
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
    let mut parts = s.split('-');
    let prefix = parts.next();
    if !matches!(prefix, Some("S") | Some("s")) {
        return Err(Error::Usage(format!("not a raw SID: `{s}`")));
    }
    let revision: u8 = parts
        .next()
        .ok_or_else(|| Error::Usage(format!("SID `{s}` missing revision")))?
        .parse()
        .map_err(|_| Error::Usage(format!("SID `{s}` revision is not a number")))?;
    let authority_str = parts
        .next()
        .ok_or_else(|| Error::Usage(format!("SID `{s}` missing authority")))?;
    let authority: u64 = if let Some(hex) = authority_str.strip_prefix("0x") {
        u64::from_str_radix(hex, 16)
            .map_err(|_| Error::Usage(format!("SID `{s}` authority is not a number")))?
    } else {
        authority_str
            .parse()
            .map_err(|_| Error::Usage(format!("SID `{s}` authority is not a number")))?
    };
    let subs: std::result::Result<Vec<u32>, _> = parts.map(str::parse).collect();
    let subs = subs.map_err(|_| Error::Usage(format!("SID `{s}` has a non-numeric subauthority")))?;
    Ok(Sid::new(revision, authority, subs))
}

/// Open the caller's own token and read its user SID.
fn resolve_self() -> Result<Sid> {
    use libp_token::SelfOpenFlags;
    use libp_token::uapi::KACS_TOKEN_QUERY;
    let tok = Token::open_self(
        SelfOpenFlags {
            real_token: false,
            ..Default::default()
        },
        KACS_TOKEN_QUERY,
    )
    .map_err(|e| Error::Invalid(format!("open self token: {e}")))?;
    let user = tok
        .user_sid()
        .map_err(|e| Error::Invalid(format!("read user SID: {e}")))?;
    Ok(user)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_known_labels() {
        assert_eq!(parse("Everyone").unwrap(), WellKnownSid::Everyone.to_sid());
        assert_eq!(parse("System").unwrap(), WellKnownSid::LocalSystem.to_sid());
        assert_eq!(
            parse("Administrators").unwrap(),
            WellKnownSid::BuiltinAdministrators.to_sid()
        );
    }

    #[test]
    fn parses_sddl_aliases() {
        assert_eq!(parse("SY").unwrap(), WellKnownSid::LocalSystem.to_sid());
        assert_eq!(
            parse("BA").unwrap(),
            WellKnownSid::BuiltinAdministrators.to_sid()
        );
    }

    #[test]
    fn parses_raw_sid() {
        let sid = parse("S-1-5-32-544").unwrap();
        assert_eq!(sid, WellKnownSid::BuiltinAdministrators.to_sid());
    }

    #[test]
    fn principal_self_is_s_1_5_10() {
        assert_eq!(parse("@owner").unwrap(), Sid::new(1, 5, vec![10]));
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
