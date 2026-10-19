.SH SECURITY
A newly created \fBnohup.out\fR receives owner\-only native security by
default. The creation mode is also restricted. An explicit SDDL request
selects another descriptor for a newly created output. Existing output files
are appended to without replacing their security descriptors.
.PP
Input and output already redirected by the caller retain those selections.
Ignoring the hangup signal does not provide service supervision or immunity to
other termination mechanisms.
