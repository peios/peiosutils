.SH ADDRESSING
Key paths accept slash or backslash separators and compare without case. A
leading separator is optional. The value name is a separate operand; \fB@\fR
denotes the empty\-name default value. Output separator selection does not
change parsing.
.PP
Typed value prefixes override inference. Use \fBsz:\fR when a numeric\-looking
value must remain text, including leading zeros. A successful set reports the
resolved type. Malformed hexadecimal input is rejected as an invalid
specification, including non\-ASCII characters that cannot form byte pairs;
accepted separators do not make malformed digits valid.
.SH LAYERS
Reads resolve the effective value. Deleting a layer's entry can reveal a lower
layer. A tombstone masks lower values, and a hidden path masks key visibility.
The selected write layer defaults to base.
.PP
Security\-descriptor mutations affect the key object directly and are not
undone by layer removal. A winning\-layer query is not a complete export of
every shadowed lower entry.
.SH BATCH DOCUMENTS
Apply accepts the supported JSON batch representation. Text export is a review
format; text\-format apply is not implemented. A batch must stay within one
hive so its changes can be committed in one transaction.
.PP
The JSON object contains a keys array. Key entries carry a path, optional
values, and optional descriptor. Values carry name, type, and data or the
supported hexadecimal representation. A descriptor supplies selected SDDL
components; absent components are not a request to erase all other parts.
Unknown key\-entry members and invalid descriptors are rejected.
.PP
JSON export preserves the represented values but does not emit descriptors. It
is not an exact security backup. Opaque backup and restore use the kernel
snapshot mechanism and its required privileges.
.SH SECURITY
Operations use the caller's authority and report the kernel's denial. There is
no blanket identity check in the utility that grants an operation. Reading or
changing an audit SACL and privileged backup/restore operations can require
privileges beyond ordinary key access.
.PP
Destructive operations prompt when standard input is a terminal and can
proceed without that prompt when input is redirected. Confirmation is not an
authorization boundary.
.SH NOTES
Watch notifications mean that matching state changed. Reread the relevant
configuration instead of treating a notification as a complete replacement
state. Expected\-sequence writes detect a concurrent change rather than
silently overwriting a newer value.
