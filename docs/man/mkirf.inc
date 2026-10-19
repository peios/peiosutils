.SH SOURCE LAYOUT
The source root must contain an executable regular init, directly or through a
supported link within the tree. Existing generated sequence files are refused.
Supported archive entries include regular files, directories, symbolic links,
FIFOs, and device nodes; sockets are refused.
.PP
Packaged hooks are read directly under \fBusr/libexec/prelude/hooks.d\fR and
operator hooks under \fBlcl/libexec/prelude/hooks.d\fR. The operator copy wins
on equal filenames. The legacy hooks directory is accepted with a migration
warning. Generated sequence versions 1 and 2 are placed under
\fBsystem/prelude\fR in the image, not written into the source tree.
.PP
Hook declarations use the supported comment metadata block. Missing required
capabilities, cycles, malformed declarations, and conflicting
provider/contributor use are errors. An absent optional after capability is
not the same as an unsatisfied requires declaration. Unannotated hooks are
warned about and scheduled last in name order.
.SH REPRODUCIBILITY
Entries are sorted in byte order. Ownership, timestamps, and archive inode
numbers are normalized. Directories use 0755, links 0777, FIFOs and devices
0644, and regular files 0755 or 0644 according to executability.
.PP
The default compressed member uses zstd level 3. Gzip uses level 6 with zero
timestamp and no embedded filename. Reproducibility assumes identical source
contents, options, and implementation.
.PP
The reserved \fB++\fR directory contains named early segments. Each child is a
directory whose contents map to the early archive root. Segment labels do not
become archive prefixes. Duplicate entries are refused. An uncompressed
archive combines the early data before the compressed main archive; early
payload alignment is preserved.
.SH NOTES
Watch mode performs an initial build, then rebuilds after a quiet interval.
Build failures are reported without ending the watch loop. Output inside the
watched source is refused to prevent feedback. Exclusion globs are relative to
the source; an excluded directory prunes its subtree.
.PP
Output is assembled in a sibling staging file and renamed into place.
Successful assembly establishes archive construction, not runtime boot
qualification or signing.
