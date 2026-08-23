// SID rendering for `token`.
//
// The style enum, the well-known table and the renderer all live in
// `uucore::sid_render` — one place, so `ls`, `sd`, `token` and `revstrm`
// cannot disagree about what a principal is called. They had: this file used
// to carry its own 13-entry table saying `Local System` and `Administrators`
// where `sd`'s said `LocalSystem` and `BUILTIN\Administrators`.
//
// What remains here is the part that is genuinely token's own — the JSON
// shape, which needs serde_json and so cannot sit in uucore.

pub use uucore::sid_render::{SidStyle, render};

use peios::security::Sid;

/// JSON object form: `{ "sid": ..., "label": ... }`. `label` is omitted when
/// no name is available for the SID.
pub fn render_json(sid: &Sid) -> serde_json::Value {
    let raw = sid.to_string();
    match uucore::sid_render::name_for(sid, &raw) {
        Some(name) => serde_json::json!({ "sid": raw, "label": name }),
        None => serde_json::json!({ "sid": raw }),
    }
}
