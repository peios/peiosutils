---
title: stat
type: reference
description: Display the detailed status of a file or a file system — type, size, timestamps, and raw inode metadata — with an optional custom output format.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/overview
  - peios/security-descriptors/overview
---

`stat` displays the detailed status of a file. Where [`ls`](~peios/listing-and-paths/ls) gives you a line per file, `stat` gives you everything the system records about *one* file: its type, its size, its timestamps, and the low-level metadata stored in its inode.

```
stat [options] file...
```

`stat` is a diagnostic tool. It dumps what a file's inode physically contains, verbatim. That makes it the right command for "tell me exactly what is recorded about this file" — and it means some of what it prints needs a word of explanation, below.

## The default output

With no options, `stat` prints a labelled, multi-line block per file:

```
$ stat notes.txt
  File: notes.txt
  Size: 219             Blocks: 8          IO Block: 4096   regular file
Device: 8,2     Inode: 1572931     Links: 1
Access: (0644/-rw-r--r--)  Uid: ( 1000/    jack)   Gid: ( 1000/    jack)
Access: 2026-05-15 11:03:42.000000000 +0000
Modify: 2026-05-15 11:03:42.000000000 +0000
Change: 2026-05-15 11:03:18.000000000 +0000
 Birth: 2026-05-15 11:03:18.000000000 +0000
```

Reading it line by line:

- **File** — the name, and for a symbolic link, what it points to.
- **Size / Blocks / IO Block / type** — the size in bytes, the number of allocated disk blocks, the preferred I/O block size, and the file type in words.
- **Device / Inode / Links** — the device the file lives on, its inode number (its unique index within that device), and the number of hard links to it.
- **Access / Uid / Gid** — a numeric owner id, a group id, and a mode value. See the caveat below — these are not the access policy.
- **Access / Modify / Change / Birth** — four timestamps: when the file was last read, last written, last had its metadata changed, and first created.

## The Uid, Gid, and mode fields are decorative

The `Access: (0644/-rw-r--r--)  Uid: (…)  Gid: (…)` line needs care.

A file's inode carries a numeric owner id, a numeric group id, and a mode value, and `stat` reports them because `stat` reports the inode verbatim. But **the Peios access model does not consult these fields.** What a principal may do to a file is decided by the file's **security descriptor** — see [Security descriptors](~peios/security-descriptors/overview). The numbers on the `Uid`/`Gid`/mode line are inert metadata: they are stored, they are reported, and they have no effect on any access decision.

Do not read that line as a permission summary. A file showing `0644` is not "world-readable" in any meaningful sense — whether anyone can read it depends entirely on its security descriptor. `stat` shows these fields for completeness as a low-level diagnostic; it does not claim they mean anything.

To see the parts of a file's security that *do* count, use [`ls -l`](~peios/listing-and-paths/ls), which shows the owner SID and the mode column built from the security descriptor.

## Custom output formats

Two options replace the default block with output you control.

| Option | Effect |
|---|---|
| `-c FORMAT`, `--format=FORMAT` | Print `FORMAT` for each file, substituting `%` directives. A newline is added after each file. |
| `--printf=FORMAT` | Like `--format`, but interpret backslash escapes (`\n`, `\t`) in `FORMAT` and add no trailing newline. Include `\n` yourself if you want one. |

```
$ stat --format='%n is %s bytes' notes.txt
notes.txt is 219 bytes
```

### File directives

Used in the format string when stating files (the default mode):

| Directive | Substitutes |
|---|---|
| `%n` | File name. |
| `%N` | Quoted file name; for a symlink, with the target shown. |
| `%F` | File type in words (`regular file`, `directory`, …). |
| `%s` | Total size, in bytes. |
| `%b` | Number of allocated blocks. |
| `%B` | Size in bytes of each block counted by `%b`. |
| `%o` | Preferred I/O transfer block size. |
| `%i` | Inode number. |
| `%h` | Number of hard links. |
| `%d` / `%D` | Device number, in decimal / in hexadecimal. |
| `%t` / `%T` | For a device file, the major / minor device type, in hexadecimal. |
| `%m` | Mount point of the file system the file is on. |
| `%f` | Raw mode value, in hexadecimal. |
| `%x` / `%X` | Last access time — human-readable / seconds since the epoch. |
| `%y` / `%Y` | Last modification time — human-readable / seconds since the epoch. |
| `%z` / `%Z` | Last status-change time — human-readable / seconds since the epoch. |
| `%w` / `%W` | File creation (birth) time — human-readable / seconds since the epoch. `-` or `0` if unknown. |

The directives `%a` and `%A` (the mode in octal and in symbolic form) and `%u`, `%U`, `%g`, `%G` (the owner and group, numeric and by name) report the decorative inode fields described above. They are available for completeness; they are not the file's access policy.

The `%C` directive (security context) is inactive — it produces no meaningful value on a standard Peios system.

### File-system directives

With `-f`, `stat` reports on the *file system* a file lives on rather than the file itself, and the format string uses a different directive set:

| Directive | Substitutes |
|---|---|
| `%n` | File name. |
| `%i` | File-system ID, in hexadecimal. |
| `%t` / `%T` | File-system type — in hexadecimal / in words. |
| `%l` | Maximum length of a file name. |
| `%s` | Block size, for fast transfers. |
| `%S` | Fundamental block size, used for the block counts. |
| `%b` | Total data blocks in the file system. |
| `%f` | Free blocks. |
| `%a` | Free blocks available to an ordinary principal. |
| `%c` | Total inodes (file nodes). |
| `%d` | Free inodes. |

## Options

| Option | Effect |
|---|---|
| `-f`, `--file-system` | Report on the file system containing each file, instead of the file. |
| `-L`, `--dereference` | Follow symbolic links — report on the link's target rather than the link itself. |
| `-t`, `--terse` | Print the information on a single line, as bare values with no labels. Useful for scripts. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every file was stated successfully. |
| `1` | A file could not be stated, or an option or format directive was invalid. |
