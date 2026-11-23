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
.SH NOT INDEXED
A file's indexing preference, the
.B user.pgss.indexing
extended attribute of PGSS Indexing, says how far a search indexer may record
it:
.B all
for names, properties and contents,
.B no\-content
for names and properties only, and
.B none
for nothing at all. A directory's preference holds for everything beneath it,
until a nearer preference says otherwise, so
.B all
on a subdirectory loosens its parent's.
.PP
.B +i
sets
.BR no\-content ,
as Windows'
.B attrib +i
does, and leaves
.B none
as it is.
.B \-i
removes the preference, so the file takes its directory's.
.B \-\-indexing
sets any of the three. The listing shows
.B I
for a file whose own preference is anything but
.BR all ;
it does not show a preference a file takes from a directory above.
.PP
A preference narrows what an indexer records; it does not hide a file from
listings or grant or deny anyone access.
.SH EXIT STATUS
0 if every file was shown or changed, 1 if any failed. A file on a filesystem
that keeps no special attributes fails, and so does setting an indexing
preference on one that keeps no extended attributes.
