.SH SECURITY
A requested new\-filesystem policy is set on the detached filesystem before
attachment. Failure to set it prevents attachment with the unintended policy.
This is not whole\-command rollback: target preparation can already have
changed the namespace, and propagation can fail after attachment.
.PP
Policy selection applies only to a new real filesystem, not bind, move,
remount, propagation\-only, or listing operations. The unmanaged class is
kernel\-selected rather than a userspace policy choice. A supplied synthetic
descriptor must parse and include an owner.
.PP
A bind mount shares its superblock. Bind\-remount requests that would change
superblock state are refused; per\-mount attributes remain a distinct scope.
.SH FILES
Live state comes from \fB/proc/self/mountinfo\fR. This implementation does not
use \fB/etc/fstab\fR for operand completion or \fB/etc/mtab\fR as mount state.
.SH NOTES
A regular\-file source can receive loop backing. Matching backing with the
same offset and size can be reused. Newly allocated loop devices are marked
for automatic cleanup. Offset and size limits imply loop use and do not apply
as if a block device were an ordinary image file.
.PP
Filesystem probing rejects ambiguous signatures rather than choosing one.
Read\-only retry behavior differs from an explicit read\-write request.
Remounts apply selected changes rather than assuming unmentioned options are a
complete replacement of all previous flags.
.SH BUGS
The parsed \fB\-\-onlyonce\fR option is not consulted by the executor. Inspect
live state before a repeated operation; the flag is not idempotence.
.PP
Fake mode skips mount syscalls but can create target directories during
preparation. It is not a no\-write preview and does not validate real mount
authority. The mkdir mode is not applied as a Peios descriptor; establish and
verify target security separately.
.PP
A later propagation failure can leave the attachment in place. Inspect live
state after a failed invocation before choosing a retry or cleanup.
