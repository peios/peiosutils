.SH SECURITY
Traditional mode, uid, and gid fields are compatibility metadata. Peios access
checks use tokens and security descriptors. These fields do not represent
every right, audit rule, integrity rule, or token restriction.
.SH CREATION DATE AND ATTRIBUTES
The
.B Birth
line is the time the filesystem created the file, which nobody can set. The
.B Created
line is the file's creation date as people and copy tools use it: its
Created At if one is set, and otherwise its birth time, marked
.BR (birth) .
A copy made with
.B cp \-p
can carry its original's creation date, and
.B touch \-\-created\-at
sets one. The
.B Attributes
line shows
.B read\-only
for a file that no one can open for writing or delete until the flag is
cleared, and \- otherwise. A filesystem that keeps no special attributes shows
the birth time and \-.
.PP
The sequences
.BR %k ,
.B %K
and
.B %j
print the creation date, the creation date in seconds since the Epoch, and
the attributes. With the
.B #
flag,
.B %k
marks a date that is only the birth time.
