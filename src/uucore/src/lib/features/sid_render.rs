// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Turning a SID into something worth reading.
//!
//! This is the **one** place peiosutils renders a SID for display. It used to
//! be five — `ls` through `uucore::sd_control`, `token` and `sd` each with
//! their own `SidStyle` and their own label table, `sd` again for the reverse
//! direction, and `revstrm` with a hand-rolled byte formatter. They had
//! already drifted: the same principal printed as `Local System` from one
//! command and `LocalSystem` from another, `Administrators` from one and
//! `BUILTIN\Administrators` from another, and the branch `ls` was on had no
//! labels at all — so `ls -l` showed `S-1-5-18` for a SID two other tables
//! could already name.
//!
//! # Two sources of a name, in this order
//!
//! **Well-known SIDs come from the table below.** They are fixed by
//! specification rather than held in any store, so there is nothing an
//! authority could add — and answering locally means `ls -l` over a large tree
//! does not ask the authority about `Everyone` several thousand times, nor
//! change what it prints depending on whether the authority happens to be
//! running.
//!
//! **Everything else goes to the resolver**, if one is installed — see
//! [`set_resolver`]. That is the seam for real principal names
//! (`S-1-5-21-…-1000` → `jack`), which only the authority can answer.
//!
//! # Why the resolver is a seam rather than an implementation
//!
//! SID → name is not a POSIX question, so NSS cannot carry it: `libnss_peios`
//! answers `getpwuid`, and libc has no SID key. The lookup that *can* answer
//! is PSD-012 chapter 6 `Lookup` with `KeyType::Sid`, on `/run/ident.sock` —
//! which means speaking the ident wire format, which lives in `libauthd`.
//! peiosutils does not depend on it today. Until it can, this module names
//! what it knows statically and leaves the rest as SIDs, which is honest
//! rather than degraded.
//!
//! Nor can a client shortcut via SID → uid → `getpwuid`: the projection is the
//! authority's to know, not something a caller may compute.
//!
//! # A note on spelling
//!
//! These labels follow the user-facing documentation (*Well-known
//! principals*). authd's own table spells two of them differently —
//! `SYSTEM` for `S-1-5-18`, and `Administrators` rather than
//! `BUILTIN\Administrators` — because those strings are also policy-record key
//! names and a backslash is the registry's path separator. That constraint is
//! about key names, not display, so the two are allowed to differ. Do not
//! "fix" one to match the other without reading both.

use std::sync::OnceLock;

use peios::security::SidRef;

/// How a SID should be rendered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SidStyle {
    /// `Label (S-1-…)` when a name is known, else `S-1-…`.
    #[default]
    Both,
    /// Always the raw `S-1-…` form.
    Raw,
    /// The name if one is known; the raw form otherwise.
    Label,
}

/// Well-known SIDs, keyed on their canonical string form.
///
/// Keyed on the string rather than on `peios::security::WellKnown` because
/// the enum only goes forward (`Sid::well_known(variant)`) and does not cover
/// everything nameable here — the integrity levels and the `BUILTIN` aliases
/// past `Administrators` have no variant. Rendering formats the SID anyway,
/// so matching on the result costs nothing.
const WELL_KNOWN: &[(&str, &str)] = &[
    // Null and universal.
    ("S-1-0-0", "Nobody"),
    ("S-1-1-0", "Everyone"),
    ("S-1-2-0", "Local"),
    ("S-1-2-1", "Console Logon"),
    // Creator placeholders — rewritten at inheritance time, never matched.
    ("S-1-3-0", "Creator Owner"),
    ("S-1-3-1", "Creator Group"),
    ("S-1-3-4", "Owner Rights"),
    // Logon-type groups. Membership is a property of a logon, not of an
    // account, which is why these carry no projected gid.
    ("S-1-5-2", "Network"),
    ("S-1-5-4", "Interactive"),
    ("S-1-5-6", "Service"),
    ("S-1-5-7", "Anonymous"),
    ("S-1-5-10", "Self"),
    ("S-1-5-11", "Authenticated Users"),
    // Machine identities.
    ("S-1-5-18", "Local System"),
    ("S-1-5-19", "Local Service"),
    ("S-1-5-20", "Network Service"),
    // BUILTIN aliases.
    ("S-1-5-32-544", "BUILTIN\\Administrators"),
    ("S-1-5-32-545", "BUILTIN\\Users"),
    ("S-1-5-32-546", "BUILTIN\\Guests"),
    ("S-1-5-32-551", "BUILTIN\\Backup Operators"),
    // Integrity levels. Not principals, but they appear as SIDs in a SACL's
    // mandatory label and a reader wants them named.
    ("S-1-16-0", "Untrusted IL"),
    ("S-1-16-4096", "Low IL"),
    ("S-1-16-8192", "Medium IL"),
    ("S-1-16-8448", "Medium-Plus IL"),
    ("S-1-16-12288", "High IL"),
    ("S-1-16-16384", "System IL"),
    ("S-1-16-20480", "Protected-Process IL"),
];

/// The well-known label for a SID already rendered to its string form.
pub fn label_for(raw: &str) -> Option<&'static str> {
    WELL_KNOWN
        .iter()
        .find(|(sid, _)| *sid == raw)
        .map(|(_, label)| *label)
}

type Resolver = Box<dyn Fn(&SidRef) -> Option<String> + Send + Sync>;

static RESOLVER: OnceLock<Resolver> = OnceLock::new();

/// Install the principal-name resolver, once per process.
///
/// Returns `false` if one was already installed, in which case the existing
/// resolver stands. Nothing installs one yet — see the module docs.
///
/// A resolver is consulted only for SIDs the well-known table does not cover,
/// and may return `None` freely: an unresolvable SID renders raw, which is
/// what every caller here did before a resolver existed.
pub fn set_resolver(resolver: Resolver) -> bool {
    RESOLVER.set(resolver).is_ok()
}

/// The best name available for `sid`: well-known first, then the resolver.
pub fn name_for(sid: &SidRef, raw: &str) -> Option<String> {
    if let Some(label) = label_for(raw) {
        return Some(label.to_string());
    }
    RESOLVER.get().and_then(|resolve| resolve(sid))
}

/// Render a SID for human output.
pub fn render(sid: &SidRef, style: SidStyle) -> String {
    let raw = sid.to_string();
    if style == SidStyle::Raw {
        return raw;
    }
    match (style, name_for(sid, &raw)) {
        (SidStyle::Label, Some(name)) => name,
        (SidStyle::Both, Some(name)) => format!("{name} ({raw})"),
        // No name, or Raw handled above: the SID is the answer.
        _ => raw,
    }
}

/// Render a SID held as raw bytes, for decoders reading a wire format.
///
/// `None` when the bytes are not a valid SID, which a caller reading
/// attacker-shaped or corrupt input must handle rather than assume away.
pub fn render_bytes(bytes: &[u8], style: SidStyle) -> Option<String> {
    SidRef::from_bytes(bytes).map(|sid| render(sid, style))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn well_known_labels_are_unique_and_sorted_by_nothing_in_particular() {
        // Duplicated keys would make `label_for` order-dependent, which is
        // exactly the drift this module exists to end.
        let mut keys: Vec<&str> = WELL_KNOWN.iter().map(|(k, _)| *k).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "duplicate SID key in WELL_KNOWN");
    }

    #[test]
    fn label_lookup_hits_and_misses() {
        assert_eq!(label_for("S-1-5-18"), Some("Local System"));
        assert_eq!(label_for("S-1-16-12288"), Some("High IL"));
        assert_eq!(label_for("S-1-5-21-1-2-3-1000"), None);
    }

    #[test]
    fn render_bytes_rejects_a_malformed_sid() {
        assert_eq!(render_bytes(&[0u8; 3], SidStyle::Both), None);
    }
}
