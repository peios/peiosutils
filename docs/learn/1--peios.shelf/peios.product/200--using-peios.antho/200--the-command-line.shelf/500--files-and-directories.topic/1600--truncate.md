---
title: Changing a File Length
type: how-to
description: >-
  Grow a disposable file without confusing logical size with allocated
  storage.
related:
  - peios/files-and-directories/overview
---

`truncate` changes a file's length. Growing can introduce a sparse region;
shrinking removes bytes beyond the selected length.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   truncate -s 1K size-sample.bin
   ```

2. Run the next command:

   ```sh
   stat -c '%s' size-sample.bin
   ```

## Check the result

The size is 1024 bytes. The amount of allocated storage can be smaller.

## Protect data before shrinking

Shrinking discards the file's tail. Growing it again does not restore the old
bytes. Work on a copy when testing a resize operation.

The size syntax can express absolute lengths or adjustments. Check the unit
convention rather than assuming every suffix means decimal bytes.
`truncate(1)` documents those forms and no-create behavior.
