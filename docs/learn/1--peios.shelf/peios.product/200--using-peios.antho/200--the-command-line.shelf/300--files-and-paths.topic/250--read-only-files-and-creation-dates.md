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

## Commands on this page

| Command | Use it to | Manual |
|---|---|---|
| `attrib` | Show, set, or clear the read-only flag | `attrib(1)` |
