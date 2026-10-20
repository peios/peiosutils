---
title: Inspecting Storage Devices
type: how-to
description: >-
  Read the block-device hierarchy before selecting storage for another
  operation.
related:
  - peios/files-and-volumes/mount
  - peios/files-and-volumes/umount
  - peios/files-and-paths/inspecting-files-and-paths
---

`lsblk` shows block devices and their relationships. It helps distinguish a
whole disk, a partition, and a layered device before an operation uses one of
those objects.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
lsblk -o NAME,SIZE,TYPE,MOUNTPOINTS
```

## Check the result

The output shows available devices, sizes, types, and known mount points. The
device set depends on the running system.

## Missing information is not a negative finding

Columns come from several sources, including kernel device information,
filesystem probing, mount state, and device-node security. A source can be
unavailable or unreadable even when the device exists.

An empty filesystem identity does not prove that the device is blank. Probe
access can be denied, and missing optional libraries can limit the information
available.

The owner and mode columns describe the device node, not the files stored
inside its filesystem. Peios node security is not equivalent to a set of
traditional mode bits.

For scripts, choose explicit columns and a structured output mode. The human
tree is intended for inspection. `lsblk(1)` defines column sources and the
representation of incomplete information.
