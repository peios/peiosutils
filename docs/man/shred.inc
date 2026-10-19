.SH NOTES
Overwriting a file reaches only storage exposed through that file.
Copy\-on\-write filesystems, snapshots, journal copies, backups, and device
wear leveling can retain independent copies. Additional overwrite passes do
not erase copies the filesystem does not overwrite.
.PP
The force option does not bypass Peios security\-descriptor checks. Use a
storage\-appropriate erasure process when unrecoverability is required.
