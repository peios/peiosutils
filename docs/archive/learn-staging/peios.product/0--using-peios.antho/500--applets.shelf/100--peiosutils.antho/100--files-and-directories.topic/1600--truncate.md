---
title: truncate
type: reference
description: Shrink or extend a file to an exact size.
related:
  - peios/files-and-directories/overview
---

`truncate` sets a file's size to an exact figure — shrinking it, or extending it.

```
truncate [options] file...
```

```
$ truncate -s 0 logfile        # empty the file
$ truncate -s 1G disk.img      # make a 1 GiB file
```

Shrinking a file discards everything past the new size. Extending it adds space that reads back as zero bytes; that added space is a *hole* — it costs no actual storage until something writes real data into it, which is how `truncate -s 1G` can create a "1 GiB file" instantly.

By default, `truncate` creates a file that does not yet exist (as an empty file, then sized as asked). A file created this way needs a [security descriptor](~peios/security-descriptors/overview), and it inherits one from the directory it is created in.

## Specifying the size

`-s` (`--size`) takes the size. A plain number sets the size outright. A unit suffix scales it: `K`, `M`, `G`, `T`, … where the bare letter is a power of 1024 (`K` = 1024) and the letter with `B` is a power of 1000 (`KB` = 1000).

A size may also begin with a prefix, which makes it *relative to the file's current size*:

| Prefix | Meaning |
|---|---|
| `+` | Extend by this much. |
| `-` | Reduce by this much. |
| `<` | Shrink to this size only if the file is currently larger ("at most"). |
| `>` | Extend to this size only if the file is currently smaller ("at least"). |
| `/` | Round the size *down* to a multiple of this number. |
| `%` | Round the size *up* to a multiple of this number. |

```
$ truncate -s +4K notes.txt    # 4 KiB larger than it is now
$ truncate -s '<1M' notes.txt  # cap it at 1 MiB, leave smaller files alone
```

## Options

| Option | Effect |
|---|---|
| `-s`, `--size=SIZE` | Set or adjust the size to `SIZE`, as described above. |
| `-r`, `--reference=RFILE` | Base the size on the current size of `RFILE` instead of giving a number. Combine with a relative `-s` to mean "the size of `RFILE`, adjusted". |
| `-c`, `--no-create` | Do not create a file that does not exist; skip it instead. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every file was resized. |
| `1` | A file could not be resized — a bad size, or a file that could not be opened. |
