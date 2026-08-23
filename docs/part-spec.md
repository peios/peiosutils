# peiosutils `part` — Partitioning Tool Specification

Status: **v1 implemented.**

`part` creates and edits GPT partition tables. It is shipped as a normal
`peiosutils` applet and is the tool `peios-install --whole-disk` delegates to.

It fills the slot a Windows user would reach for `diskpart` to fill, but not its
shape. `diskpart` is a stateful interactive shell that also formats volumes,
assigns drive letters and manages dynamic disks. `part` is one-shot commands
over a named device, covering the partition table and nothing else — the house
style for `sd`, `reg` and `stratafs`, and the right shape for its main caller,
which is a script. What `diskpart` also bundles already has homes elsewhere:
`mke2fs` and `mkfs.vfat` format, `mount` mounts, and Peios has no drive letters.

**Out of scope, deliberately:** MBR↔GPT conversion, resize/extend/shrink,
dynamic disks, RAID, and writing any label other than GPT.

## 1. Security and consistency rules

- Every destructive operation requires `--yes`. There is no interactive
  confirmation, because the caller is usually a script.
- Replacing a partition table `part` did not create additionally requires
  `--force`. The two flags mean different things: `--yes` is "I mean this
  destructive operation", `--force` is "and I know it destroys a table that was
  already there".
- `part` refuses a device that is not a whole disk. Writing a GPT *inside* a
  partition produces a table that looks valid to anything reading that partition
  directly and is invisible to everything else.
- `part` refuses a disk any of whose partitions is mounted.
- Both guards are evaluated in-process. Neither shells out to an external
  command, because a guard written that way fails **open**: a missing command
  makes `sh` exit 127, `if` reads that as false, and the check silently passes.
  That is not hypothetical — `peios-install` shipped exactly that bug for eight
  revisions (PEI-191).
- The tool runs entirely with the caller's authority and does not require,
  acquire, or emulate privilege. In practice the device nodes under `/dev` grant
  only `LocalSystem`, so `part` needs SYSTEM authority for the same reason
  `mke2fs` does.
- A damaged table is reported, never silently repaired. `part` is not a recovery
  tool, and rewriting a table nobody asked it to touch is how data is lost.

## 2. Commands

```
part list                       # every disk on the system
part list   <disk>              # one disk in detail
part verify <disk>
part create <disk> --yes [--force]
part add    <disk> [--size SIZE] [--type TYPE] [--name NAME] --yes [--force]
part del    <disk> <index> --yes [--force]
```

`list` and `verify` are read-only. `create`, `add` and `del` write.

### `part list` with no device

Lists every whole disk, with its partitions beneath it:

```
DEVICE             SIZE  CONTENTS
/dev/vda           8.0G  gpt, 2 partitions
  vda1             512M  esp    EFI system partition
  vda2             7.5G  linux  Peios root
/dev/vdb           8.0G  no partition table
```

Three rules govern this view:

- **Structure comes from sysfs, not from any partition table.** A disk carrying
  a label `part` cannot manage still lists its partitions, because they are
  whatever the *kernel* found. "What is on this machine" should not depend on
  whether `part` approves of the answer.
- **Partition names are read, never derived.** `sd`/`vd` number directly
  (`vda1`) while `nvme`/`mmcblk` interpose a `p` (`nvme0n1p1`); a rule guessed
  from the disk name gets one of those wrong.
- **A disk that cannot be read is still listed**, with `?` for its contents. For
  an inventory command that is the only defensible behaviour — the disk you
  cannot read is precisely the one you want to be told about. Probe failures
  degrade one line, never the listing.

Zero-length devices are omitted: the kernel always presents a set of empty
`loop` and `sr` slots, and listing a dozen of them buries the disks that exist.

### Sizes

`--size` accepts `K`, `M`, `G`, `T` (powers of 1024), an explicit sector count
with `s`, or `max` for the largest free run. **A bare number is sectors, not
bytes** — `--size 2048` is 1 MiB at 512-byte sectors, not 2 KiB. Sizes that are
not a whole number of sectors round up.

### Types

`--type` accepts an alias or a raw GUID in either case.

| alias | GUID | meaning |
|---|---|---|
| `esp` | `C12A7328-F81F-11D2-BA4B-00A0C93EC93B` | EFI system partition |
| `linux` | `0FC63DAF-8483-4772-8E79-3D69D8477DE4` | Linux filesystem data |
| `swap` | `0657FD6D-A4AB-43C4-84E5-0933C84B4F4F` | Linux swap |
| `msdata` | `EBD0A0A2-B9E5-4433-87C0-68B6B72699C7` | Microsoft basic data |

The alias list is deliberately short. A table of every type GUID in circulation
would be a catalogue to maintain, and a raw GUID is always accepted, so nothing
is out of reach for want of an alias.

### Names

Up to 36 UTF-16 code units. A longer name is **rejected, not truncated**: a
partition name is how a person identifies the thing they are about to format,
and quietly shortening it makes the label on screen disagree with the label on
disk. Characters outside the BMP cost two units each.

## 3. What `part` reports about a disk it cannot manage

"No GPT" is ambiguous between a blank disk and an MBR disk holding somebody's
data, and those deserve opposite behaviour. `part` therefore reports what it
*found*:

| found | `list` says | `create`/`add`/`del` |
|---|---|---|
| a healthy GPT | the table | proceed |
| a real MBR | "an MBR (dos) partition table, which part cannot manage" | refuse without `--force` |
| Apple/BSD/Sun/SGI label | names the label | refuse without `--force` |
| protective MBR, invalid header | "the table may be damaged" | refuse without `--force` |
| a filesystem written straight to the disk | "a &lt;fstype&gt; filesystem … with no partition table" | refuse without `--force` |
| nothing | "no partition table" | `create` proceeds |

Foreign labels are identified with **libblkid**, which is also where `mount` and
`lsblk` get filesystem identity (`uucore::blkid`). Recognising other people's
formats is a catalogue that grows with every util-linux release — exactly the
kind of thing to reuse rather than reimplement.

libblkid is **not** trusted for whether a GPT is healthy. It answers
`PTTYPE=gpt` on the strength of a protective MBR alone, so a disk whose header
is corrupt still looks like GPT to it. That judgement comes from `part`'s own
parse.

## 4. Exit codes

| code | meaning |
|---|---|
| 0 | success |
| 1 | usage error, or the operation failed |
| 2 | I/O against the device |
| 3 | refused by a safety guard, including a foreign or damaged table |

3 is separated from 1 on purpose. "This disk is not what you said it was" is
`part` working correctly, and a caller such as `peios-install` should stop and
say so rather than treat it as a malfunction to retry.

## 5. On-disk layout

```
  LBA 0                     protective MBR
  LBA 1                     primary header
  LBA 2 .. 2+E-1            primary entry array      (E = ceil(16384/sector))
  first_usable = 2+E        ─┐
                             ├─ partitions
  last_usable  = L-E-1      ─┘
  LBA L-E .. L-1            backup entry array
  LBA L                     backup header            (L = disk_sectors-1)
```

`E` is **32** at 512-byte sectors and **4** at 4096, so `first_usable` is 34 or
6. Nothing in the implementation hardcodes 34.

Partitions are aligned to **1 MiB** (2048 sectors at 512 bytes, 256 at 4096).
The logical sector size is read from `queue/logical_block_size`, falling back to
`BLKSSZGET`; it is never assumed, because assuming 512 on a 4Kn disk silently
misplaces every structure.

Both checksums are **CRC-32/ISO-HDLC**, the algorithm `cksum -a crc32b`
computes. The header CRC is taken over the first `header_size` bytes with its
own field zeroed; the entry CRC covers the entire array including unused
entries, which is why the array is zero-filled rather than merely allocated.

### Write ordering

Entries are written before the headers that vouch for them, and the disk is
synced before the kernel is asked to re-read:

```
protective MBR → primary entries → backup entries
               → primary header → backup header → fsync → BLKRRPART
```

If power is lost midway the disk holds either the old table or an unreadable
one, never a valid header confidently pointing at entries that were never
written.

`BLKRRPART` is what makes the new partitions visible: without it they exist on
disk and nowhere else, and the caller's next `mkfs` fails on a path that does
not exist. `/dev` is devtmpfs, so the kernel's own device model materialises the
nodes — no udev is involved, which matters because Peios ships none. If a
partition is still open the ioctl returns `EBUSY`; `part` reports that the table
*was* written but the kernel's view is stale, which is more useful than an
errno.

## 6. Why GPT is written here rather than bound from libfdisk

The rule applied throughout peiosutils is to reuse other people's **data** and
own our own **logic**.

libblkid is a data problem: 61 superblock probes, 12,036 lines of format
knowledge that grows every release, silently wrong rather than loudly wrong when
stale. It is reused.

libfdisk is 20,047 lines, of which ~6,400 are MBR/BSD/Sun/SGI labels Peios will
never use (UEFI-only, no bootloader), 1,865 an sfdisk script parser and 1,127 an
interactive prompt layer. The part we want — create, list, align — is small, the
format has not changed in substance since 2000, and its two fiddly primitives
were already workspace dependencies (`crc-fast`, used by `cksum`, and `rand`).
Binding it would mean 400–600 lines of compile-time-unchecked FFI plus a shared
library on every install, to avoid writing code smaller than the binding.

## 7. Testing

Because a wrong partition table is unrecoverable, correctness is established
against external ground truth rather than self-agreement:

- **Both directions against sgdisk.** `sgdisk -v` and `sgdisk -p` must accept
  and correctly describe a `part`-written table; `part list` and `part verify`
  must agree with `sgdisk -p` on an sgdisk-written one. The seven structural
  header fields are compared byte-for-byte.
- **`blkid`** must independently report `PTTYPE=gpt`.
- **Unit tests** pin the mixed-endian GUID encoding against published type
  GUIDs, the CRC-32 variant against its published check value, the inclusive
  `ending_lba` convention, alignment, free-extent selection, and both sector
  sizes.
- **Guards are asserted to fire**, not merely to exist — the failure being
  guarded against is a check that silently never ran.

All host-side testing runs against **regular files**. `Device` treats a file as
a first-class target, which is not a test affordance but the only way to develop
a disk-destroying tool safely: no test ever needs to name a device node.
