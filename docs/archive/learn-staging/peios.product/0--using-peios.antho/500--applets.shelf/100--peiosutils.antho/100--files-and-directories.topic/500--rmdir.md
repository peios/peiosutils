---
title: rmdir
type: reference
description: Remove directories, but only when they are already empty.
related:
  - peios/files-and-directories/rm
  - peios/files-and-directories/overview
---

`rmdir` removes directories — but only **empty** ones. If a directory still has anything in it, `rmdir` leaves it alone and reports the failure.

```
rmdir [options] directory...
```

```
$ rmdir old-cache/
```

That refusal to touch a non-empty directory is the whole point of `rmdir`: it is the safe way to remove a directory you *believe* is empty. If it is not empty, you find out instead of losing the contents. To remove a directory together with everything inside it, use [`rm -r`](~peios/files-and-directories/rm).

## Options

| Option | Effect |
|---|---|
| `-p`, `--parents` | Remove the directory and then its ancestors, as long as each becomes empty in turn. `rmdir -p a/b/c` removes `a/b/c`, then `a/b`, then `a`. |
| `--ignore-fail-on-non-empty` | Do not treat "directory not empty" as a failure. Other failures are still reported. Useful when clearing out whatever directories happen to be empty and leaving the rest. |
| `-v`, `--verbose` | Print a line for each directory as it is removed. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every directory was removed. |
| `1` | A directory could not be removed — it was not empty, did not exist, or removal was refused. |
