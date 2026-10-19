.SH NOTES
Operands are resolved against live mount state. An exact mount point is
selected directly. A source mounted more than once is ambiguous unless
all\-targets behavior is requested. Recursive expansion operates deepest
first, including stacked mounts, and stops on its first failure.
.PP
Lazy detachment removes the attachment while outstanding references can
remain. It does not prove final cleanup or safe device removal. Read\-only
fallback changes state instead of completing a detach.
.PP
Loop cleanup verifies that the loop device still backs the removed mount
before clearing it, avoiding an unrelated reused device. Mount\-created
automatically cleared loops usually need no explicit detach request.
.PP
Multiple top\-level operands can produce mixed success. Recursive and
all\-targets expansion failures are not an atomic rollback of completed
unmounts.
