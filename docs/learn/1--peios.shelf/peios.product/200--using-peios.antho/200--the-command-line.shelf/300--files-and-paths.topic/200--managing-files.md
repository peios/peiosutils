---
title: Managing Files and Directories
type: how-to
description: Copy, move, create, link, and remove files and directories with the Peios command-line utilities, choosing the security each result receives.
related:
  - peios/files-and-paths/files-names-and-security
  - peios/permissions/sd
  - peios/files-and-paths/inspecting-files-and-paths
---

Use these commands to copy, move, create, link, and remove files in a
terminal. Each section shows the common case and the Peios behavior that
affects it. The man page for each command lists every option.

## Before you start

Use a terminal on a machine with the command-line utilities installed. To try
the examples, work in an empty directory so that nothing you need is
overwritten. The examples name files such as `report.txt`; create them first,
or substitute your own.

## Copy files

Use `cp` to make a new file or directory tree from existing data:

```sh
cp -i report.txt report-backup.txt
```

`-i` asks before overwriting a file that already exists. To copy a directory
and everything in it, use `cp -R notes notes-copy`. If the destination already
exists as a directory, `cp` copies the source into it.

A copy is a new object, so it inherits security from its destination
directory, and copying to another location can change who can read the data.
To keep particular attributes, ask for them:

- `--preserve=timestamps` keeps the access and modification times.
- `--sd-explicit` carries the source's explicit access rules, and lets the
  destination directory add its inherited rules.
- `--sd` copies the source's owner and its full access and audit lists.

Neither security option copies the security descriptor's group, so a copy is
not a complete security backup. `cp(1)` describes every preservation option
and the access each one needs.

## Move and rename files

Use `mv` to change a file's name or location:

```sh
mv -i draft.txt report.txt
```

Within one filesystem, a move is a rename: the file keeps its security
descriptor, even in a directory with different inherited rules. Across
filesystems, `mv` copies the file and then removes the source, and the copy
inherits security from its destination. Requesting particular attributes also
makes `mv` copy, even within one filesystem. See `mv(1)`.

## Create files and directories

Create a directory, with any missing parents:

```sh
mkdir -p reports/2026/drafts
```

`touch build.marker` creates an empty file, or updates the timestamps of a
file that exists without changing its contents. `mkfifo` creates a named pipe
that passes a stream from one program to another, and `mknod` creates device
nodes, which needs the relevant authority. `mktemp -d` creates a directory
with a unique name and prints it, so another program can't claim the name
first.

A new object inherits security from its parent directory. `mkdir`, `touch`,
`mkfifo`, and `mknod` can request an owner, group, integrity label, or
complete security descriptor for the object they create. `mkfifo` and `mknod`
also accept a `-m` mode option, but it isn't how security is set on Peios.
Check the result with `sd show` before you share the location, because
choosing an owner doesn't on its own give that owner the access you meant. See
`mkdir(1)` for the security options.

## Link files

Create a symbolic link, a file that stores a path to another file:

```sh
ln -s report.txt latest
```

A relative path in a symbolic link is resolved from the directory that
contains the link. Without `-s`, `ln` creates a hard link: another name for
the same object, sharing its contents and security. Hard links can't usually
cross filesystems. `link` and `unlink` add or remove exactly one hard link,
without the other choices `ln` and `rm` offer.

## Remove files and directories

Remove a file, with a confirmation prompt:

```sh
rm -i disposable.txt
```

Removal has no undo. `rm` removes a directory and its contents only when you
ask for recursion. A prompt doesn't decide whether you can delete the file:
the file's security descriptor does. `rmdir` removes only empty directories,
which protects you from removing contents by mistake.

`shred` overwrites a file's data before removing it, but snapshots, backups,
journals, and the storage device can keep copies that it doesn't reach. When
data must become unrecoverable, use a method designed for the storage, such as
encryption with a key you can destroy.

## Change a file's length

`truncate -s 1K size-sample.bin` sets a file's length. Shrinking a file
discards its end, and growing it again doesn't bring that data back. Work on a
copy when you're unsure.

## Check the result

List the result and its security:

```sh
ls -l
sd show ./report-backup.txt
```

Check the security descriptor of anything you copied or created somewhere
other people can reach. A recursive command that failed part way through can
leave some files changed.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `cp` | Copy files and directory trees | `cp(1)` |
| `mv` | Move or rename files | `mv(1)` |
| `mkdir` | Create directories | `mkdir(1)` |
| `touch` | Create a file or update its timestamps | `touch(1)` |
| `mkfifo` | Create a named pipe | `mkfifo(1)` |
| `mknod` | Create a device node | `mknod(1)` |
| `mktemp` | Create a uniquely named file or directory | `mktemp(1)` |
| `ln` | Create symbolic or hard links | `ln(1)` |
| `link`, `unlink` | Add or remove one hard link | `link(1)`, `unlink(1)` |
| `rm` | Remove files and directory trees | `rm(1)` |
| `rmdir` | Remove empty directories | `rmdir(1)` |
| `shred` | Overwrite a file's data | `shred(1)` |
| `truncate` | Change a file's length | `truncate(1)` |
