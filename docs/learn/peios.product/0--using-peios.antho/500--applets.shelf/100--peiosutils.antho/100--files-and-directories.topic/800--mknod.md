---
title: Understanding Device Nodes
type: how-to
description: >-
  Distinguish a device node from a regular file and a named pipe.
related:
  - peios/files-and-directories/mkfifo
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`mknod` creates special filesystem entries. A device node routes file
operations to a driver; a named pipe connects readers and writers.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   mknod messages p
   ```

2. Run the next command:

   ```sh
   ls -l messages
   ```

## Check the result

The listing identifies `messages` as a pipe. It has no regular-file message
contents.

## Device numbers select a driver interface

Block and character nodes require a major and minor number. Those numbers
identify a driver and its device instance; choosing arbitrary numbers does not
create hardware or discover a missing disk.

Creating a device node requires the relevant authority. Use the device
management mechanism for normal device discovery rather than guessing a node
under `/dev`.

Peios uses creation flags for requested security instead of the `-m` mode
option. For a pipe alone, `mkfifo` expresses the intention more clearly. See
`mknod(1)` for node types and number syntax.
