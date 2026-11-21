.SH READ-ONLY
A read\-only regular file cannot be opened for writing, truncated, replaced,
or deleted, by any process, its owner and administrators included, until the
flag is cleared. A program that already had the file open for writing keeps
that access. Renaming the file, and changing its security descriptor, are
not affected. A directory keeps the flag but is not protected by it.
.PP
Read\-only is not a permission: it grants and denies no one anything. Anyone
with the right to write the file's attributes can clear it. Use
.BR sd (1)
to decide who can use a file.
.SH EXIT STATUS
0 if every file was shown or changed, 1 if any failed. A file on a filesystem
that keeps no special attributes fails.
