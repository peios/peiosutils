---
title: link and unlink
type: reference
description: The single-purpose, low-level commands for creating one hard link and for removing one file.
related:
  - peios/files-and-directories/ln
  - peios/files-and-directories/rm
  - peios/files-and-directories/overview
---

`link` and `unlink` are the bare, single-purpose versions of two operations that [`ln`](~peios/files-and-directories/ln) and [`rm`](~peios/files-and-directories/rm) also perform. Each does exactly one thing, takes exactly its operands, and has no options. They exist for scripts that want one precise operation with no surrounding behaviour — no prompting, no link kinds to choose, no recursion.

## `link`

```
link file1 file2
```

`link` creates one hard link: a new name, `file2`, for the existing file `file1`. `file1` must exist; `file2` must not.

```
$ link data.csv data-archive.csv
```

That is the whole command. It performs the link operation directly and reports whether it succeeded. For anything more — symbolic links, replacing an existing destination, linking into a directory — use [`ln`](~peios/files-and-directories/ln).

## `unlink`

```
unlink file
```

`unlink` removes one file: it deletes the directory entry `file`. It takes a single name and removes it directly.

```
$ unlink stale.lock
```

`unlink` does not prompt, does not recurse, and does not remove directories. For removing several files, removing directories, or any of the safety prompts, use [`rm`](~peios/files-and-directories/rm).

## Why they exist

`ln` and `rm` are the commands to use day to day — they are flexible and have the safety behaviours. `link` and `unlink` are deliberately rigid: a script that calls `unlink` will *only ever* remove a single file, and a reviewer can see that at a glance. The narrowness is the feature.

## Exit status

Both commands:

| Code | Meaning |
|---|---|
| `0` | The operation succeeded. |
| `1` | The operation failed — a missing or already-existing operand, or a refused request. |
