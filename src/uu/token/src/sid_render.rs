// SID rendering. The design doc commits to "label + raw" by default,
// with --raw / --label as overrides. Labels resolve by matching the SID
// against the `peios::security::WellKnown` set; unknown SIDs render
// raw-only.
//
// Migration note: the old `libp_sd::WellKnownSid::{from_sid, label}`
// reverse-lookup (SID bytes -> label) has no analogue in the new `peios`
// crate, whose `WellKnown` only goes forward (`Sid::well_known(variant)`).
// We rebuild the reverse map here by encoding every `WellKnown` variant
// once and matching the queried SID against it.

use peios::security::{Sid, WellKnown};

/// How a SID should be rendered to the human-readable output stream.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SidStyle {
    /// `LABEL (S-1-…)` when a label is known, else `S-1-…`.
    #[default]
    Both,
    /// Always render the raw form.
    Raw,
    /// Render the label if known; fall back to the raw form.
    Label,
}

/// The well-known SIDs we can attach a human label to, with their display
/// labels. Mirrors the old `WellKnownSid::label()` text.
const WELL_KNOWN: &[(WellKnown, &str)] = &[
    (WellKnown::Null, "Null"),
    (WellKnown::Everyone, "Everyone"),
    (WellKnown::Local, "Local"),
    (WellKnown::CreatorOwner, "Creator Owner"),
    (WellKnown::CreatorGroup, "Creator Group"),
    (WellKnown::OwnerRights, "Owner Rights"),
    (WellKnown::Anonymous, "Anonymous"),
    (WellKnown::PrincipalSelf, "Self"),
    (WellKnown::AuthenticatedUsers, "Authenticated Users"),
    (WellKnown::System, "Local System"),
    (WellKnown::LocalService, "Local Service"),
    (WellKnown::NetworkService, "Network Service"),
    (WellKnown::Administrators, "Administrators"),
];

/// Best-effort reverse lookup: the well-known label for `sid`, if any.
fn label_for(sid: &Sid) -> Option<&'static str> {
    WELL_KNOWN
        .iter()
        .find(|(wk, _)| Sid::well_known(*wk).as_bytes() == sid.as_bytes())
        .map(|(_, label)| *label)
}

/// Format a SID for the human output stream.
pub fn render(sid: &Sid, style: SidStyle) -> String {
    let raw = sid.to_string();
    let label = label_for(sid);
    match (style, label) {
        (SidStyle::Raw, _) => raw,
        (SidStyle::Label, Some(l)) => l.to_string(),
        (SidStyle::Label, None) => raw,
        (SidStyle::Both, Some(l)) => format!("{l} ({raw})"),
        (SidStyle::Both, None) => raw,
    }
}

/// JSON object form: `{ "sid": ..., "label": ... }`. `label` is omitted
/// when no well-known label is available.
pub fn render_json(sid: &Sid) -> serde_json::Value {
    let raw = sid.to_string();
    match label_for(sid) {
        Some(l) => serde_json::json!({ "sid": raw, "label": l }),
        None => serde_json::json!({ "sid": raw }),
    }
}
