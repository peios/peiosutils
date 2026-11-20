.SH SECURITY
Requested creation security applies to newly created objects. The shared
creation options select owner, group, integrity label, inheritance protection,
or a complete SDDL descriptor. A special node cannot be created carrying a
descriptor, so the FIFO is created first and the descriptor applied to it
afterward, without following a symbolic link at the name. Until then the
FIFO has the descriptor it inherited. If the descriptor cannot be applied,
the command removes the FIFO and reports the error. A creation request does
not replace the security of an object that already exists.
.SH NOTES
The \fB\-m\fR mode interface is not supported. Use the native security
creation options instead.
