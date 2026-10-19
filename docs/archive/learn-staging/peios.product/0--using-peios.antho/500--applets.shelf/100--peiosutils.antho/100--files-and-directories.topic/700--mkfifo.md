---
title: mkfifo
type: reference
description: Create named pipes (FIFOs).
related:
  - peios/files-and-directories/mknod
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`mkfifo` creates **named pipes**, also called FIFOs.

```
mkfifo [options] name...
```

```
$ mkfifo events
```

A named pipe is a file that two programs use to pass a stream of data: one writes into it, the other reads from it, and the data flows through in order — first in, first out, which is what "FIFO" stands for. Unlike an ordinary pipe between two commands, a named pipe has a name on the file system, so the two programs do not have to be started together or be related to each other.

## Setting the new pipe's security

A FIFO is a file, and a new file needs a [security descriptor](~peios/security-descriptors/overview). By default a new FIFO inherits its descriptor from the directory it is created in.

`mkfifo` accepts the shared **creation flags** — `--owner`, `--group`, `--label`, `--no-inherit`, and `--sddl` — to set that descriptor explicitly instead. They behave exactly as they do for `mkdir`; the full reference is on the [`mkdir`](~peios/files-and-directories/mkdir) page.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every FIFO was created. |
| `1` | A FIFO could not be created — for example, the name already exists. |
