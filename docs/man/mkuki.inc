.SH IMAGE LAYOUT
The stub is a PE/COFF executable. New read\-only initialized\-data sections
are appended in the order \fB.cmdline\fR, \fB.linux\fR, \fB.initrd\fR. Header
space and section offsets are adjusted when additional section entries require
more header room. Malformed stub headers are rejected.
.PP
The command line is literal or read from a file. Trailing carriage returns and
newlines are trimmed, embedded NUL bytes are rejected, and a final NUL is
added. Kernel and initramfs payloads must be nonempty.
.PP
Directory\-based kernel selection requires exactly one regular matching image
beneath a release directory. Missing or ambiguous matches fail that build
instead of selecting an arbitrary kernel. The bundled stub's provenance is
available through the stub\-info operation.
.SH NOTES
The assembled output is written through an exclusively created sibling
temporary file, then renamed into place. Staging\-name collisions do not open
an existing path for writing. Failure cleanup removes the temporary file
without publishing a partial image.
.PP
Watch mode observes concrete inputs through parent\-directory watches so
atomic replacement is visible. Kernel\-directory selection watches the whole
selected tree and resolves the candidate on each rebuild. Output locations
that would retrigger the watch are refused. A failed rebuild is logged while
the loop continues.
.PP
The command does not sign the result. A completed image still requires
separate signing and boot qualification appropriate to the deployment.
