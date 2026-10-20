.SH COMMANDS
.TP
list [DEVICE]
List all disks or inspect one whole disk. An unreadable table is reported as
unknown, not as an empty disk.
.TP
verify DEVICE
Validate the GPT structure, ranges, and checksums.
.TP
create DEVICE --yes [--force]
Write a fresh empty GPT. This replaces the previous partition layout.
.TP
add DEVICE [--size SIZE] [--type TYPE] [--name NAME] --yes [--force]
Add an entry. Size defaults to max, the largest free run. A bare number is a
sector count; K, M, G, and T use powers of 1024. Placement is aligned to 1 MiB.
Type defaults to linux; esp, linux, swap, and msdata are aliases, and a raw
GUID is accepted. The name is a GPT partition name, not a filesystem label.
.TP
del DEVICE INDEX --yes [--force]
Delete the one-based partition entry. This does not preserve or relocate data.
.SH SAFETY
Writes require confirmation, a whole-disk target, and no mounted filesystem
on the disk. --force permits replacement of a foreign table; it does not
bypass the whole-disk or mounted-device guards. A write is not an undoable
filesystem transaction. Verify the table and kernel partition discovery
before using a new entry. Partitioning does not format a filesystem.
.SH EXIT STATUS
0 indicates success; 1 indicates usage or operation failure; 2 indicates device
I/O failure; 3 indicates refusal by a safety guard or foreign-table rule.
