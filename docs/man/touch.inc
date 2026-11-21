.SH SECURITY
Requested creation security applies to newly created objects. The shared
creation options select owner, group, integrity label, inheritance protection,
or a complete SDDL descriptor. The file is created carrying the descriptor;
see mkdir(1) for how an integrity label is set. A dangling symbolic link at the
name is an error: touch does not create the file at the link's target. A
creation request does not replace the security of an object that already
exists, including one created by another program after touch found the name
free.
.SH CREATION DATE
A file's Created At is its creation date as people and copy tools use it,
apart from the birth time the filesystem records, which nobody can set.
.B \-\-created\-at
sets it, reading the date as
.B \-d
does, and
.B \-\-no\-created\-at
unsets it, so that the creation date reads as the birth time again. Either
option alone leaves the access and modification times unchanged; give
.BR \-d ,
.BR \-t ,
.BR \-r ,
.BR \-a ,
.B \-m
or
.B \-\-time
as well to change them too. A file that touch creates gets its Created At
after it is created. A filesystem that keeps no special attributes cannot
record one, and the file is reported as failed.
