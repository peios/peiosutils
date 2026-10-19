// Batch operations: `reg export` (dump a subtree) and `reg apply` (apply a
// batch atomically). See docs/reg-spec.md §6.
//
// JSON is the canonical, exact format; the text form is the human-facing
// default for `export`. `apply` auto-detects (leading `{`/`[` ⇒ JSON). The
// text *parser* is deferred until the §6.1 escaping grammar is finalised
// (apply of text input returns a clear error pointing there); text *export*
// is available now for human review.
//
// The document, its export and its apply are registry_docs's, which other programs
// take too; this is the command line around them.

pub mod apply;
pub mod export;

pub use registry_docs::{Document, KeyEntry, ValueEntry};

impl From<registry_docs::Error> for crate::error::Error {
    fn from(e: registry_docs::Error) -> Self {
        match e {
            registry_docs::Error::Registry { op, path, source } => {
                Self::from_peios(op, &path, source)
            }
            registry_docs::Error::Invalid(why) => Self::InvalidSpec(why),
        }
    }
}
