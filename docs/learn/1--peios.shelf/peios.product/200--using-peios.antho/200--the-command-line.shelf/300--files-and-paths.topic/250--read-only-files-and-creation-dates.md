---
title: Read-Only Files and Creation Dates
type: how-to
description: Protect a file from changes and deletion with the read-only flag, and see or set the creation date a file keeps apart from its birth time.
related:
  - peios/files-and-paths/managing-files
  - peios/files-and-paths/inspecting-files-and-paths
  - peios/permissions/sd
---

Every file on Peios can carry two special attributes besides its security
descriptor: a read-only flag, and a creation date that copies can carry over.
Windows programs set and check both, and Peios keeps them the same way.

The examples use a sample file named `report.txt`. Create it first, or
substitute your own.

## Make a file read-only

Mark a file read-only:

```sh
attrib +r report.txt
```

No program can then open the file for writing, truncate it, replace it, or
delete it, whoever runs the program, the file's owner included. Clear the flag
to change the file again:

```sh
attrib -r report.txt
```

Run `attrib` with only file names to see which are read-only. `R` marks a
read-only file and `-` a writable one:

```sh
attrib report.txt notes.md
```

Read-only guards against accidental change, not against other people. It
grants and denies no one access, and anyone allowed to write the file's
attributes can clear it. Use [`sd`](~peios/permissions/sd) to decide who can
use a file. A program that already had the file open for writing keeps that
access, and you can still rename the file. A directory keeps the flag but
isn't protected by it.

## Delete a read-only file

`rm` asks before deleting a read-only file:

```sh
rm report.txt
```

Answer `y` to delete it. `rm -f` deletes it without asking. In a script,
where `rm` can't ask, it refuses and leaves the file unless you give `-f`, so
a read-only file survives a cleanup that didn't mean to remove it. Deleting a
read-only file needs the right to change its attributes as well as the right
to delete it.

## See and set a creation date

A file has two creation dates. Its birth time is when the filesystem created
it, and nobody can change it. Its Created At is the date people and copy tools
mean: a copy can carry its original's date, and you can set one. Until a file
has a Created At, its creation date is its birth time.

`stat` shows both:

```sh
stat report.txt
```

`Birth` is the birth time. `Created` is the creation date, marked `(birth)`
when it is only the birth time. `Attributes` shows `read-only` for a read-only
file. For one value, use `stat -c %k report.txt`, or `%K` for seconds since
the Epoch.

Set a file's Created At with `touch`, writing the date in any form `touch -d`
accepts:

```sh
touch --created-at='2026-03-14 09:12' report.txt
```

This changes only the creation date. Add `-d`, `-t`, `-r`, `-a`, or `-m` to
change the access and modification times as well. Remove the date again, so
that the creation date is the birth time:

```sh
touch --no-created-at report.txt
```

## Copy and move files

A plain `cp` makes a new file, with today's creation date and no read-only
flag. `cp -p` and `cp -a` keep the original's creation date and, if it is
read-only, make the copy read-only too:

```sh
cp -p report.txt report-copy.txt
```

To keep the creation date without the flag, use `--preserve` without a value,
or remove the flag from the selection with `--no-preserve=readonly`. Moving a
file keeps both. A filesystem that has no special attributes, such as a USB
drive formatted for another system, can't keep a creation date or the flag.

## Commands on this page

| Command | Use it to | Manual |
|---|---|---|
| `attrib` | Show, set, or clear the read-only flag | `attrib(1)` |
| `touch` | Set or unset a file's Created At | `touch(1)` |
| `stat` | Show a file's creation date and attributes | `stat(1)` |
| `cp`, `mv` | Copy or move a file, with its creation date and flag | `cp(1)`, `mv(1)` |
| `rm` | Delete a read-only file | `rm(1)` |
