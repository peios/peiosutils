.SH NOTES
The token's authenticated logon\-session identifier and its interactive
environment scope are separate fields. \fBadjust interactivity\-scope\fR
changes the latter; its legacy \fBadjust session\fR alias does not change the
logon session's authentication identifier.
.PP
Comparison reads selected header fields, groups, and privileges. Equal and
unequal comparisons both exit successfully when the queries succeed. JSON
comparison fields contain left, right, and matches values; a missing group or
privilege is null, not an implied disabled membership.
.PP
Thread enumeration reads \fB/proc/PID/task\fR and queries each thread's
effective identity. Disappeared or inaccessible threads retain error rows,
mark the JSON result incomplete, and cause failure. Impersonation remains a
separate fact even when its user SID matches the primary token's user. Queries
are consecutive observations, not an atomic process\-wide snapshot.
.SH SECURITY
Token targets, including process, thread, and captured socket\-peer views, are
subject to the applicable access checks. Restrictions and adjustments do not
confer authority the caller lacks. Token creation and primary\-token
installation require the corresponding privileged operations.
