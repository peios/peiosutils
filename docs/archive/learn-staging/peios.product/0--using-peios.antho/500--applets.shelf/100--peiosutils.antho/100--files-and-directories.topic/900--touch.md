---
title: touch
type: reference
description: Create empty files, or update the timestamps of existing ones.
related:
  - peios/files-and-directories/mkdir
  - peios/files-and-directories/overview
---

`touch` does one of two things, depending on whether the file already exists:

- if the file **does not exist**, `touch` creates it, empty;
- if the file **does exist**, `touch` updates its timestamps to the current time.

```
touch [options] [file...]
```

```
$ touch notes.txt        # create notes.txt if absent, else bump its times
```

The "create an empty file" behaviour is the everyday use. The "update timestamps" behaviour matters to tools that decide what work to do by comparing file times — `touch` is how you tell such a tool that a file should be considered freshly changed.

## Which timestamps, and to what

By default `touch` sets both the access time and the modification time to the current moment. These options narrow or redirect that.

| Option | Effect |
|---|---|
| `-a` | Change only the access time. |
| `-m` | Change only the modification time. |
| `--time=WORD` | Choose which time to change by name: `access`, `atime`, or `use` for the access time; `modify` or `mtime` for the modification time. |
| `-d`, `--date=STRING` | Use the time described by `STRING` instead of the current time. |
| `-t STAMP` | Use the time given as `[[CC]YY]MMDDhhmm[.ss]` instead of the current time. |
| `-r`, `--reference=FILE` | Use the timestamps of `FILE` instead of the current time. |
| `-h`, `--no-dereference` | If the named file is a symbolic link, change the link's own timestamps rather than those of the file it points to. |

## Not creating a file

To use `touch` purely to update timestamps, and never to create anything:

| Option | Effect |
|---|---|
| `-c`, `--no-create` | Do not create a file that does not exist. A named file that is absent is silently skipped. |

## Setting a created file's security

When `touch` *creates* a file, that new file needs a [security descriptor](~peios/security-descriptors/overview), and by default it inherits one from the directory it is created in.

`touch` accepts the shared **creation flags** — `--owner`, `--group`, `--label`, `--no-inherit`, and `--sddl` — to set that descriptor explicitly. They behave exactly as they do for `mkdir`; the full reference is on the [`mkdir`](~peios/files-and-directories/mkdir) page.

These flags apply **only on the create path**. If `touch` is updating the timestamps of a file that already exists, there is no new descriptor to set, and the creation flags have nothing to do — they do not alter an existing file's security. Pair them with `-c` and they never take effect at all, since `-c` means nothing is ever created.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every file was created or updated as requested. |
| `1` | A file could not be created or updated. |
