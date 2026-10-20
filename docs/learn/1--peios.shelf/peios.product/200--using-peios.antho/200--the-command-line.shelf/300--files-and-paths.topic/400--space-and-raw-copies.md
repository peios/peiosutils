---
title: Measuring Space and Copying Raw Data
type: how-to
description: Check filesystem capacity with df, measure a directory tree with du, and copy an exact range of bytes with dd without overwriting the wrong destination.
related:
  - peios/files-and-volumes/diagnosing-storage-pressure
  - peios/files-and-paths/managing-files
---

Use `df` and `du` to find out how much space is available and what is using
it, and `dd` to copy an exact number of bytes. These commands measure and copy
storage rather than files, so read their results with the storage model in
mind.

The examples use sample file names such as `source.bin`. Create those files
first, or substitute your own.

## Check available space

Show the space on the filesystem that holds the current directory:

```sh
df -h .
```

`df` reports space for the whole filesystem. Reserved space, metadata,
snapshots, and files that are still open after removal all affect its totals,
so free space isn't the size minus the files you can see.

## Measure a directory tree

Total the space used by a directory and everything in it:

```sh
du -sh measurements
```

`du` adds up the files it can reach. It can report allocated size, the storage
blocks a file occupies, or apparent size, the file's logical length; the two
differ for sparse and compressed files. A directory that `du` can't read makes
its total incomplete, so check for errors.

`df` and `du` answer different questions, and a difference between them is a
reason to look at what each one counts, not proof that either is wrong. For a
full procedure, see [Diagnosing Storage
Pressure](~peios/files-and-volumes/diagnosing-storage-pressure).

## Copy an exact range of bytes

Copy the first 4 bytes of a file into another file:

```sh
dd if=source.bin of=prefix.bin bs=4 count=1 status=none
```

`if=` names the input and `of=` names the output. `dd` overwrites the output
without asking, so check `of=` before you run it, especially if it names a
device: a mistake there affects a whole disk, not one file. Counts are in
units of the block size `bs=` unless you choose byte units. `dd` finishing
doesn't mean the data has reached the storage device; use its synchronization
options or `sync` when that matters.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `df` | Show filesystem capacity | `df(1)` |
| `du` | Measure a directory tree | `du(1)` |
| `dd` | Copy a byte stream in blocks | `dd(1)` |
