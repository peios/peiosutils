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
.SH NOTES
Unlinking removes a name. Other hard links, open files, snapshots, backups,
and storage remnants can retain data. Removal is not secure erasure.
