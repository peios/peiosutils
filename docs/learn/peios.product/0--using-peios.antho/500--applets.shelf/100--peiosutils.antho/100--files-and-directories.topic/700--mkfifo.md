---
title: Connecting Programs with a Named Pipe
type: how-to
description: >-
  Create a filesystem name through which two programs exchange a byte
  stream.
related:
  - peios/files-and-directories/mknod
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

A named pipe provides a first-in-first-out (FIFO) stream between programs. It
does not store a regular file's persistent contents. `mkfifo` creates its
name.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   mkfifo events
   ```

2. Create the example input:

   ```sh
   printf 'ready\n' > events &
   ```

3. Read the selected file contents:

   ```sh
   cat events
   ```

4. Wait for the background writer to finish:

   ```sh
   wait
   ```

## Check the result

The reader prints `ready`. After the writer closes, the reader reaches end of
input.

## Start both ends of the stream

Opening a pipe can wait for the other end. The example starts the writer in
the background so that the reader can open the same pipe.

The pipe's security controls who can open it. Peios creation flags can request
explicit security; the default inherits from the parent directory. The `-m`
mode option is not this command's security interface.

Remove the pipe name with `rm events` when it is not needed for the workflow.
Removing the name does not erase a stored message history because no such
history is held in the pipe file.

See `mkfifo(1)` for the complete options and status contract.
