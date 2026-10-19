.SH SECURITY
A new directory normally inherits a security descriptor from its parent.
Creation options can select owner, group, integrity, inheritance protection,
or an explicit descriptor in security\-descriptor definition language (SDDL).
The complete SDDL option cannot be combined with individual adjustments.
.PP
Creation precedes applying a requested descriptor. On application failure, the
command attempts to remove the created directory and reports the error. This
sequence is not an atomic create\-with\-security transaction. Under
parent\-creation mode, an already existing directory is accepted without
applying creation security to it.
.PP
\fB\-\-no\-inherit\fR constructs owner\-only access rather than inheriting the
parent's rules. \fB\-\-label\fR accepts untrusted, low, medium, medium\-plus,
high, system, and protected. A SID argument accepts a raw SID or a recognized
well\-known alias.
