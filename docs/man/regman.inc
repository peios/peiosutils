.SH FILES
Installed documentation fragments are read from \fB/usr/share/regman\fR by
default. The optional lookup index defaults to \fB/var/cache/regman/index\fR.
The index is an accelerator; invalid index data falls back to fragment
scanning. Index publication uses an exclusively created staging file.
.SH NOTES
The command reads documentation, not live registry state. A documented default
is not a query of the configured value. Missing documentation does not prove
the absence of a key.
.PP
Exact records take precedence over matching wildcard key families. Paths and
queries use the implementation's folded comparison rules. Wildcard matching
retains those rules without recursive backtracking.
.PP
Value\-name extraction falls back to the final name if the apparent key prefix
does not form valid string boundaries. Overlapping definitions are reported
rather than silently selected as one record.
