.SH PRESERVATION
New destination objects inherit security from their destination directory.
Copying over an existing object does not promise to replace its descriptor.
Ordinary copies attempt to preserve executable status as a best-effort operation.
An explicit preservation list starts with no implicit attributes.
.PP
.B --preserve
without a value requests access and modification times, and the creation
date.
.B -p
requests those, executable status and the read\-only flag, all as required
operations.
.B --preserve-all
and
.B -a
request every supported attribute.
Preservation options combine in command-line order; later additions or removals
change the selection.
.TP
.B owner
The source owner SID.
.TP
.B dacl, sacl
The full discretionary or audit access-control list, including inherited entries.
The audit list includes the integrity label.
.TP
.B daclni, saclni
The corresponding source list with inherited entries removed.
Full-list selection takes precedence over the inherited-entry-stripping variant.
.TP
.B timestamps
Access and modification times, and the creation date: the copy's Created At
becomes the source's creation date, which is its Created At if set and
otherwise its birth time. Not change time. A destination on a filesystem that
keeps no special attributes keeps no creation date, and the copy's is its own
birth time.
.TP
.B links
Hard-link relationships among source entries selected by this invocation.
Links to names outside that set are not recreated.
.TP
.B exec
If any source execute bit is set, set all three destination execute bits;
otherwise clear them. Symbolic links are skipped.
This does not preserve each Unix mode bit independently.
.TP
.B security
Extended attributes in the security.* namespace, except security.peios.sd and
security.peios.sattr, which the descriptor attributes,
.B timestamps
and
.B readonly
carry.
.TP
.B readonly
The read\-only flag, set on the copy after everything else. A read\-only
source whose copy cannot be made read\-only fails to preserve.
.TP
.B xattrs, xattr
Extended attributes outside security.*.
Destination-only attributes are not removed.
.PP
.B --sd
selects owner, dacl, and sacl.
.B --sd-explicit
selects owner, daclni, and saclni, allowing destination inheritance to supply
inherited access rules.
Neither selects the descriptor's group component.
There is no group preservation attribute.
These options, including the archive selection, do not guarantee a byte-identical
descriptor or preservation of every descriptor control bit.
.PP
Required preservation failures fail the copy operation.
Descriptor writes remain subject to access and validation checks.
Setting an integrity label above the caller's level requires the corresponding
relabel authority; backup or restore authority alone is not a blanket exemption.
Unsupported source extended-attribute enumeration is accepted without copying
attributes. Failure to set an attribute read from the source remains an error.
.SH READ-ONLY FILES
A read\-only regular file cannot be opened for writing or deleted by anyone
until its flag is cleared, so cp cannot copy over one as it stands. Without
.BR \-f ,
cp refuses such a destination and leaves it. With
.BR \-f ,
cp clears its read\-only flag and copies over it, keeping the file and its
security descriptor; the copy is read\-only again only if
.B readonly
is preserved and the source is read\-only. Clearing the flag takes the right
to change the file's attributes. A copy that then fails leaves the destination
writable. See
.BR attrib (1).
.SH COPY LIMITS
Copying is not transactional. Failure can leave partial output.
A required attribute failure on the regular-file path can truncate the
destination. Keep the source and any known-good backup until verification.
.PP
Recursive copying refuses to descend through a symbolic link encountered inside
the destination tree in place of a directory. A destination operand itself can
name a symbolic link to a directory.
.PP
A backup is refused when creating it would destroy the source, comparing file
identity rather than only path spelling.
