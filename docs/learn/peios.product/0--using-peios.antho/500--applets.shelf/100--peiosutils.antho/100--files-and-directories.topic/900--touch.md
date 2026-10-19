---
title: Creating a Timestamp Marker
type: how-to
description: >-
  Create a marker file and distinguish creation from changing existing
  metadata.
related:
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`touch` creates a missing file or updates timestamps on an existing one. It
does not clear an existing file's contents.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Follow these steps in the same shell:

1. Run the next command:

   ```sh
   touch build.marker
   ```

2. Run the next command:

   ```sh
   stat -c '%n: %s bytes' build.marker
   ```

## Check the result

A newly created marker has size zero. Its timestamp records when the command
ran.

## Choose which time matters

Access and modification timestamps represent different events. The default
updates both; options select one or use a supplied reference time.

A creation-only security choice applies when an object is created. It is not a
replacement for editing an existing file's descriptor. Use `sd` for that task.

Timestamps can be set deliberately, so they are not proof of origin or an
audit history. See `touch(1)` for date parsing and reference-file behavior.
