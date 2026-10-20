.SH NOTES
This command consumes KMES directly and does not persist events or query
eventd. Attaching requires enabled SeSecurityPrivilege. Snapshot mode drains
available records in CPU order without waiting; it is not a global atomic
snapshot or historical storage query. Follow mode uses one reader per CPU.
.PP
Repeated type patterns are ORed, as are repeated origin selections. Type
and origin dimensions must both match when both are supplied. Filters apply
after draining and do not reduce the kernel event production rate.
.PP
Output headers contain UTC time of day without a calendar date, CPU number,
per-ring sequence, origin class, and event type. Sequences are per ring,
not a cross-CPU total ordering. Display filtering can also cause gaps between
printed sequences. Ring overwrite is reported separately with a lost count.
.SH FILES
The event catalog is loaded from /usr/share/evman, or EVMAN_DIR when set.
Nested payload fields are looked up through dotted catalog paths. SID/GUID
fields, masks, enums, integrity levels, errors, times, durations, and byte
counts use declared types. Unknown values remain numeric; unsupported shapes
retain their wire representation. Without definitions, field-name conventions
provide fallback formatting. Undecodable payloads receive a hex preview.
.SH CAVEATS
Compact payload output is bounded and can be truncated. Pretty printing can
increase drain cost. Output failures terminate the command; per-ring errors
are diagnosed and can end draining without a failure status. A successful
exit alone does not establish complete capture. Inspect stderr and loss
markers. Redirected files use the caller's destination policy.
