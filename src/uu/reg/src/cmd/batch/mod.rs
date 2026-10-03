// Batch operations: `reg export` (dump a subtree) and `reg apply` (apply a
// batch atomically). See docs/reg-spec.md §6.
//
// JSON is the canonical, exact format; the text form is the human-facing
// default for `export`. `apply` auto-detects (leading `{`/`[` ⇒ JSON). The
// text *parser* is deferred until the §6.1 escaping grammar is finalised
// (apply of text input returns a clear error pointing there); text *export*
// is available now for human review.
//
// The document, its export and its apply are libreg's, which other programs
// take too; this is the command line around them.

pub mod apply;
pub mod export;

pub use libreg::{Document, KeyEntry, ValueEntry};

impl From<libreg::Error> for crate::error::Error {
    fn from(e: libreg::Error) -> Self {
        match e {
            libreg::Error::Registry { op, path, source } => Self::from_peios(op, &path, source),
            libreg::Error::Invalid(why) => Self::InvalidSpec(why),
        }
    }
}
