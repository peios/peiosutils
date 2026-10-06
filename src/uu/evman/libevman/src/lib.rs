// libevman ~ (peiosutils) — reading the Peios event catalogue.
//
// The catalogue is every `*.evman` fragment in /usr/share/evman (PGSS §6.10):
// one per component, each a run of records defining a field, a group of
// fields that travel together, or an event type with the fields it carries.
// This is the reading of them: parsing a fragment, checking the catalogue
// against the §6.10 rules, and finding what a name means. `evman` shows it on
// a terminal; Event Viewer and policy editors show it in windows. It never
// touches the event stream.
//
// To find what a name means, read the corpus with `corpus::read`, check it
// with `lint::check`, and ask the returned `Catalogue` to `lookup` the name.

pub mod catalogue;
pub mod corpus;
pub mod error;
pub mod fragment;
pub mod lint;
pub mod py;
