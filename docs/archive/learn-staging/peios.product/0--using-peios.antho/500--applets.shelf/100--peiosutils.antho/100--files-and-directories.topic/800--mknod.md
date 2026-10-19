---
title: mknod
type: reference
description: Create special files — device nodes and named pipes.
related:
  - peios/files-and-directories/mkfifo
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`mknod` creates **special files** — files that are not data but an interface to something else: a device, or a named pipe.

```
mknod [options] name type [major minor]
```

```
$ mknod /dev/mydisk b 8 0
$ mknod backlog p
```

## The type argument

The `type` says what kind of special file to create:

| Type | Creates |
|---|---|
| `b` | A **block device** — a device addressed in fixed-size blocks, such as a disk. |
| `c` or `u` | A **character device** — a device addressed as a stream of bytes, such as a terminal. |
| `p` | A **named pipe** (FIFO). |

For a block or character device (`b`, `c`, `u`) you must also give a **major** and a **minor** number. The major number selects which driver handles the device; the minor number tells that driver which specific device is meant. For a pipe (`p`), the major and minor numbers are omitted — a pipe has no driver behind it.

A major or minor number is read as hexadecimal if it begins with `0x`, as octal if it begins with `0`, and as decimal otherwise.

`mknod NAME p` and [`mkfifo NAME`](~peios/files-and-directories/mkfifo) do the same thing; `mkfifo` exists as the clearer way to ask for just a pipe.

## Setting the new file's security

A special file needs a [security descriptor](~peios/security-descriptors/overview) like any other file, and by default a new one inherits its descriptor from the directory it is created in.

`mknod` accepts the shared **creation flags** — `--owner`, `--group`, `--label`, `--no-inherit`, and `--sddl` — to set that descriptor explicitly. They behave exactly as they do for `mkdir`; the full reference is on the [`mkdir`](~peios/files-and-directories/mkdir) page.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The special file was created. |
| `1` | It could not be created — a missing or invalid type, missing device numbers, or a name that already exists. |
