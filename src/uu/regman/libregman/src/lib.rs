// libregman ~ (peiosutils) — reading the Peios registry manual.
//
// The manual is documentation fragments under /usr/share/regman, one
// `*.regman` file per provider, each a run of records documenting a registry
// key or value: its type, default, valid range, when a change applies, and
// what it means. This is the reading of them: parsing a fragment, finding
// the records for a key or a value, through the index when there is one.
// `regman` shows them on a terminal; Registry Editor and Event Viewer show
// them in windows. It never touches the live registry.
//
// To find what documents a value, fold its path and name with `fold::fold`
// and ask `query::resolve_exact_default`; for a key and the values it
// documents, `query::resolve_key_default`.

pub mod corpus;
pub mod error;
pub mod fold;
pub mod fragment;
pub mod index;
pub mod pattern;
pub mod query;
pub mod scan;
