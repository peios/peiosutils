// part ~ (peiosutils) manage disk partition tables.
//
// See the module docs in `src/part.rs` for the command surface, the layering,
// and why it writes GPT itself while probing foreign tables with libblkid.

uucore::bin!(pu_part);
