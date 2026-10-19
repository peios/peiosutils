.SH SECURITY
Queries and mutations use the caller's authority. Owner, group, DACL, audit
SACL, and integrity\-label operations can require different access. An
access\-check simulation does not perform or reserve the actual operation.
.PP
\fBshow\fR requests \fBOWNER | GROUP | DACL | LABEL\fR. It does not request
the full SACL. \fB\-\-all\fR adds display detail without broadening that
selection. \fB\-\-sddl\fR formats only the selected view. Human and ordinary
JSON summaries omit some ACE payload details, including callback conditions.
Do not treat matching summaries as complete descriptor equality or replay
\fBshow\fR output as a full backup.
.PP
\fBset\fR submits supplied components. A valid existing file descriptor is
updated under the kernel's component\-merging rules; missing or invalid
security descriptors follow the privileged replacement path. The operation is
not an unconditional replacement of every descriptor byte.
.PP
Inheritance protection keeps a child's stored rules from ordinary inheritance
refresh. Permitting inheritance is not a live subscription to later parent
changes. \fBpropagate\fR refreshes descendants explicitly, leaves protected
rules intact, and reports individual failures while continuing.
.PP
Recursive operations act on symbolic\-link objects and do not traverse them. A
nonrecursive path follows the final link unless physical\-link handling is
selected. \fBremove\fR removes both allow and deny DACL entries for the named
principals; removing a deny entry can increase access.
.SH NOTES
Principal operands accept raw security identifiers, recognized aliases,
friendly principal labels, and the supported self/owner selectors. Name
resolution and the kernel access check are separate operations.
.PP
Permission specifications accept named permissions, compact letters, and
numeric masks. Rule order affects access evaluation. Audit specifications also
select success, failure, or both outcomes.
