// SID rendering. The design doc commits to "label + raw" by default,
// with --raw / --label as overrides. Labels resolve via
// `libp_sd::WellKnownSid`; unknown SIDs render raw-only.

use libp_sd::WellKnownSid;
use libp_token::uapi::Sid;

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

/// Format a SID for the human output stream.
pub fn render(sid: &Sid, style: SidStyle) -> String {
    let raw = sid.to_string();
    let label = WellKnownSid::from_sid(sid).map(|w| w.label());
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
    match WellKnownSid::from_sid(sid) {
        Some(w) => serde_json::json!({ "sid": raw, "label": w.label() }),
        None => serde_json::json!({ "sid": raw }),
    }
}
