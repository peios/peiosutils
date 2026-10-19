---
title: lsblk
type: reference
description: List block devices as a tree, flat list, JSON or key=value pairs — with filesystem identity, topology, and SD-derived owner and mode columns.
related:
  - peios/mount-policies/overview
  - peios/mount-policies/mount
  - peios/mount-policies/umount
  - peios/listing-and-paths/ls
---

`lsblk` lists the block devices on the system and their relationships — disks, their partitions, and the device-mapper and loop devices layered on top. It is the device-oriented companion to [`mount`](~peios/mount-policies/mount): where `mount` with no arguments shows what is *mounted*, `lsblk` shows what is *available to mount* and what filesystem sits on each device.

```
lsblk [options] [DEVICE...]
```

By default it prints a tree, one row per device with children indented beneath their parent. Given one or more `DEVICE` operands (a name like `sda`, a full path like `/dev/sda`, or anything resolving to a device node), it re-roots the output at those devices.

## Where the data comes from

Each column is sourced from one of four places, and any that cannot be read degrades to an empty cell rather than aborting the run:

- **sysfs** (`/sys/block`) — the device tree, sizes, and the topology/hardware columns. Peios leaves `/sys/block` unpatched, so this is the standard no-udev path.
- **libblkid** — filesystem identity: `FSTYPE`, `FSVER`, `UUID`, `LABEL`, and the partition-table columns (`PTTYPE`, `PTUUID`, `PARTTYPE`, `PARTLABEL`, `PARTUUID`). libblkid is opened at runtime, and it reads the device itself, so these columns are empty for anyone who may not open it: as shipped, a disk's node lets only SYSTEM and Administrators open it.
- **The device node's security descriptor** — `OWNER` and `MODE`, read the same way [`ls -l`](~peios/listing-and-paths/ls) reads them.
- **The `/dev/disk/by-*` symlink farm** — `ID-LINK`. Populated by the device manager once it has run; there is deliberately no `/run/udev/data` parser, so the column reads the links themselves and is empty in the initramfs, where no device manager runs.

## Output modes

The mode selects how the rows are formatted; it does not change which columns are shown.

| Flag | Mode |
|---|---|
| *(default)* | Indented tree. |
| `-l`, `--list` | Flat list — the same columns, no tree glyphs. |
| `-J`, `--json` | JSON. Flag columns render as bare booleans and `MOUNTPOINTS` as an array; children nest under a `children` key. |
| `-P`, `--pairs` | `KEY="value"` pairs, one device per line. |
| `-r`, `--raw` | Raw, space-separated. Values that could contain a space or control character are hex-escaped (`\xNN`) so fields stay parseable. |
| `-T`, `--tree[=COLUMN]` | Force tree output even alongside `-l`, optionally attaching the tree glyphs to `COLUMN` instead of `NAME`. |

## Columns

With no column flag, `lsblk` prints the default set: `NAME`, `MAJ:MIN`, `RM`, `SIZE`, `RO`, `TYPE`, `MOUNTPOINTS`. Three flags swap in a preset, and `-o` names an explicit list (which wins over all of them):

| Flag | Column set |
|---|---|
| `-o`, `--output LIST` | Exactly the comma-separated columns named (case-insensitive; an unknown name is a usage error). |
| `-O`, `--output-all` | Every available column. |
| `-f`, `--fs` | Filesystem view: `NAME`, `FSTYPE`, `FSVER`, `LABEL`, `UUID`, `MOUNTPOINTS`. |
| `-m`, `--perms` | Permissions view: `NAME`, `SIZE`, `OWNER`, `MODE`. |

The available columns are `NAME`, `KNAME`, `PATH`, `MAJ:MIN`, `FSTYPE`, `FSVER`, `LABEL`, `UUID`, `PTUUID`, `PTTYPE`, `PARTTYPE`, `PARTLABEL`, `PARTUUID`, `MOUNTPOINT`, `MOUNTPOINTS`, `SIZE`, `RO`, `RM`, `HOTPLUG`, `TYPE`, `OWNER`, `MODE`, `MODEL`, `VENDOR`, `REV`, `SERIAL`, `TRAN`, `HCTL`, `ALIGNMENT`, `MIN-IO`, `OPT-IO`, `PHY-SEC`, `LOG-SEC`, `STATE`, `ROTA`, `SCHED`, `PKNAME`, and `ID-LINK`.

### The `OWNER` and `MODE` columns

`OWNER` and `MODE` describe the *device node*, not the filesystem on it, and they mirror [`ls -l`](~peios/listing-and-paths/ls) exactly — there is no `GROUP` column and no POSIX permission bits, because access to a device node is governed by its security descriptor, not by a mode. `OWNER` is the owner SID (`S-1-…`); `MODE` is a three-character `[type][x][+]`: the device-type character (`b` for a block device, `c` for a character device), an `x` slot (never set for a block device), and a `+` when the DACL is inheritance-protected. When the SD cannot be read — for instance on a non-Peios host with no KACS syscalls — `OWNER` shows `?` and `MODE` degrades honestly.

## Filtering, sorting and shaping

| Flag | Effect |
|---|---|
| `-a`, `--all` | Include empty (zero-size) devices, which are hidden by default. |
| `-d`, `--nodeps` | Do not print a device's holders or slaves (drop the children). |
| `-I`, `--include LIST` | Show only devices with these major numbers (comma-separated). |
| `-e`, `--exclude LIST` | Exclude devices by major number. The default is `1` (RAM disks); an explicit `-e` replaces that default, and `-a` clears exclusions entirely. |
| `-s`, `--inverse` | Print dependencies in inverse order (holders above the devices they depend on). |
| `-M`, `--merge` | Collapse a subtree shared by several parents (a multipath device) to a single occurrence. |
| `-E`, `--dedup COLUMN` | Drop rows whose `COLUMN` value duplicates an earlier one. |
| `-x`, `--sort COLUMN` | Sort siblings by `COLUMN` (numeric columns sort by value, not text). |

## Formatting

| Flag | Effect |
|---|---|
| `-b`, `--bytes` | Print `SIZE` as an exact byte count instead of a human-readable value. |
| `-p`, `--paths` | Print full `/dev` paths in the `NAME` column. |
| `-n`, `--noheadings` | Omit the header row. |
| `-i`, `--ascii` | Draw the tree with ASCII characters instead of box-drawing glyphs. |
| `-y`, `--shell` | Render column keys shell-safe (`MAJ:MIN` becomes `MAJ_MIN`). |
| `-w`, `--width NUM` | Truncate each table row to `NUM` columns wide. |
| `--sysroot DIR` | Read sysfs, the mount table and `/dev` from `DIR` instead of `/` (chiefly for testing against a fixture). |
| `-h`, `--help` / `-V`, `--version` | Standard. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | Incorrect invocation (an unknown column, a malformed major-number or width value) or a failed run (for example, `/sys/block` is unreadable). |

`lsblk` uses this single non-zero code, matching util-linux. A per-device read failure is not fatal: it degrades the affected columns to empty or `?` and the run continues. A broken output pipe (piping into `head`, for instance) is treated as success.
