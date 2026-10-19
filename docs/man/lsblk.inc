.SH NOTES
Device topology comes from sysfs, mount relationships from live mount state,
and filesystem identity from runtime probing through libblkid. Device\-node
security supplies the native owner and mode summaries. Persistent link names
come from the \fB/dev/disk/by\-*\fR links, not a udev state database.
.PP
An optional per\-device read failure leaves the affected column empty or
unknown while enumeration continues. Empty identity fields do not prove that a
device contains no filesystem. Access denial or unavailable probing can
prevent identification.
.PP
OWNER and MODE describe the device node, not the filesystem contents. The mode
summary is type, executable marker, and inheritance\-protection marker; it is
not a conventional permission\-bit string. JSON preserves boolean flags and
mount\-point arrays instead of requiring text parsing.
