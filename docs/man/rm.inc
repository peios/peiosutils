.SH SECURITY
The write\-protected prompt is based on a write\-access check. Deletion has
its own access requirements, so the prompt is neither deletion authority nor
proof that deletion is forbidden.
.PP
Root preservation checks filesystem identity, including alternate paths to the
root. Recursive traversal uses directory handles, does not follow encountered
symbolic links, and rejects replacement of the original operand detected
during removal. These safeguards do not protect a mistakenly selected
non\-root directory.
.SH READ-ONLY FILES
A read\-only regular file cannot be deleted by anyone until its flag is
cleared. rm asks before deleting one, even where it would not otherwise ask,
and deletes it on agreement, setting the flag aside in the same step. With
.BR \-f ,
it does not ask. Unable to ask, because standard input is not a terminal, rm
refuses and leaves the file. Deleting a read\-only file takes the right to
change its attributes as well as the right to delete it. See
.BR attrib (1).
.SH DELETING WHEN CLOSED
With
.BR \-\-when\-closed ,
rm marks each regular file and directory for deletion instead of removing it
at once. A file that no other program has open goes at once. One that another
program has open keeps its name until the last program using it closes it,
and rm says so; meanwhile it cannot be opened again. A directory goes when its
last user closes it if it is empty then. One that still holds entries marked
for deletion and open elsewhere is left in place, reported as an error.
Symbolic links and other files are removed at once.
.PP
Neither way deletes a file that another program has opened without allowing
it to be deleted; rm reports the file as in use.
.SH NOTES
Unlinking removes a name. Other hard links, open files, snapshots, backups,
and storage remnants can retain data. Removal is not secure erasure.
