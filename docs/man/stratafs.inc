.SH COMMANDS
.TP
list [MOUNT]
List stacks in the current namespace, or one exact mount, in precedence order.
.TP
resolve PATH
Report providers, participants, shadowed and masked objects, absent entries,
and write/delete routing. This is explanation, not an authorization grant.
.TP
origin PATH
Print the real provider path, or every participating directory in precedence
order, using the synthetic origin attribute.
.TP
sweep MOUNT
Inspect the create stratum recursively. Entries are gaps, overrides, or
shadowed. Directory counterparts can merge rather than replace one another.
.TP
diff PATH
Compare the create-stratum entry with the first lower default. Supports regular
files and symlinks, reports type changes, and refuses other object types.
Regular-file reads are bounded to 16 MiB; binary files are reported as different.
.SH OUTPUT
list, resolve, and sweep accept --json. Non-UTF-8 names are represented
losslessly in human output; JSON refuses them instead of replacing bytes.
Incomplete protected views fail rather than presenting a partial result.
.PP
Write routing can be in_place, copy_up, create, or an error condition. Symlink
content follows its target. Deleting an override can expose a lower object;
there are no whiteouts. Every actual mutation performs its own access checks.
.SH EXIT STATUS
0 indicates success, no sweep entries, or no diff difference. 1 indicates
sweep entries or a diff difference. 2 indicates usage, visibility, malformed
state, or operational failure.
