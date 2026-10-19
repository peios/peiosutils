.SH FILES
Installed event documentation is read from \fB/usr/share/evman\fR by default.
Catalog definitions come from the packages that own the events and fields.
.SH NOTES
Lookup reads definitions rather than the event stream or event store. An event
card describes field presence and can narrow a field's general meaning through
explanatory text. A field card describes its type, value vocabulary, and
carrying events. Required, optional, and conditional field presence are
distinct.
.PP
Prefix lookup explores names beneath a prefix. Keyword search requires all
requested terms to match. Supported derived\-name rules can resolve suffix
variants and standard attributes through their underlying definition.
.PP
Catalog validation checks relationships among definitions, including
references and ownership. It is not a claim that emitted runtime records have
been validated or that a named event occurred.
