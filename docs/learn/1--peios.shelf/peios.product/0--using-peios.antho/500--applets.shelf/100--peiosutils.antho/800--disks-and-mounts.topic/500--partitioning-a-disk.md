---
title: Partitioning a Disk
type: how-to
description: Inspect a disk, plan its GPT layout, and distinguish partition changes from formatting or installation.
---

`part` manages GPT partition tables. Begin with its read-only inventory and
verification commands, and identify the whole disk by more than its transient
device name. An unreadable inventory entry is not an empty disk.

Plan the EFI and root partitions, or the data layout required by the intended
consumer, before writing. Partition changes can destroy access to existing
filesystems. Preserve needed data and unmount the target's filesystems first.
The whole-disk and mounted-device guards remain relevant even when replacing
an unfamiliar table is explicitly allowed.

Creating a table, adding an entry, and deleting an entry are separate changes.
The tool does not resize filesystems, relocate data, install a bootloader, or
supply an undo transaction. A newly added partition is not yet a formatted
or secured filesystem.

After a successful write, verify the table and confirm that the kernel sees
the intended partition devices before formatting. A valid table does not
prove an existing filesystem fits an edited entry. Use the installer for its
supported whole-disk workflow rather than reconstructing that workflow from
isolated partition commands.

`part(1)` owns size units, type aliases, confirmation flags, alignment, table
recognition, and exit status. In particular, a bare size is a sector count;
use an explicit unit when expressing a byte-sized plan.
