.SH SECURITY
A same\-filesystem rename retains the original object and its descriptor. It
does not reapply inheritance from the destination directory. A default
cross\-filesystem move creates a new object whose security inherits from the
destination; numeric ownership and set\-id state are not carried as an
identity\-preservation mechanism.
.PP
Any explicit preserve\-family selection forces copying followed by source
removal even on one filesystem. Attribute selection is then applied to the new
object. Consult cp(1) for the component\-preservation limits. Default
preservation of ordinary attributes is best effort, not a complete
metadata\-backup contract.
.SH NOTES
Successful skips under an update or no\-clobber policy can leave the source in
place. Exit success does not imply that every source moved.
.PP
When making a backup would replace the source itself, the operation is refused
using file identity rather than path spelling. A cross\-filesystem directory
destination replaced during recreation is refused instead of silently copied
into.
