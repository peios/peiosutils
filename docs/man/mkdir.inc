.SH SECURITY
A new directory normally inherits a security descriptor from its parent.
Creation options can select owner, group, integrity, inheritance protection,
or an explicit descriptor in security\-descriptor definition language (SDDL).
The complete SDDL option cannot be combined with individual adjustments.
.PP
A requested descriptor is supplied to the create itself, so the directory
never exists with the descriptor it would have inherited instead. A symbolic
link at the name is not followed. An integrity label is the exception: it is
set through the new directory's handle just after the create, because
supplying it at creation would require the privilege that a full audit list
does. Until then the directory has the label it inherited. If the label cannot
be set, the command removes the directory and reports the error. Under
parent\-creation mode, an already existing directory is accepted without
applying creation security to it.
.PP
\fB\-\-no\-inherit\fR constructs owner\-only access rather than inheriting the
parent's rules. \fB\-\-label\fR accepts untrusted, low, medium, medium\-plus,
high, system, and protected. A SID argument accepts a raw SID or a recognized
well\-known alias.
