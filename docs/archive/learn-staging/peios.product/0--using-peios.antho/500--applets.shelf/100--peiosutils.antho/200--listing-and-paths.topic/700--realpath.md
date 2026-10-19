---
title: realpath
type: reference
description: Resolve a path to its canonical, absolute form — every symbolic link followed and every . and .. component removed.
related:
  - peios/listing-and-paths/readlink
  - peios/listing-and-paths/overview
---

By default, `realpath` prints an absolute path with symbolic links resolved and `.` and `..` components removed. Options below change which components must exist, how links are handled, and whether output is absolute or relative.

```
realpath [options] file...
```

```
$ realpath ../work/./notes.txt
/data/projects/jack/notes.txt
```

Use `realpath` in scripts to turn a relative path into an absolute path or remove redundant components. Choose the existence and link-handling options for the job, and check the exit status before using the result.

## How much must exist

By default `realpath` requires every component of the path *except the last* to exist. These options change that requirement:

| Option | Existence requirement |
|---|---|
| `-E`, `--canonicalize` (default) | Every component except the last must exist. |
| `-e`, `--canonicalize-existing` | Every component must exist — including the final one. |
| `-m`, `--canonicalize-missing` | No component need exist. The path is canonicalised purely as text where it cannot be walked. |

## How symbolic links are handled

| Option | Effect |
|---|---|
| `-P`, `--physical` | Resolve each symbolic link as it is encountered. This is the default. |
| `-L`, `--logical` | Resolve `..` components *before* resolving the symbolic links they follow. |
| `-s`, `--strip`, `--no-symlinks` | Do not resolve symbolic links at all — only remove `.` and `..` components. The result is canonical as text, but a link in the path is left as-is. |

## Output options

| Option | Effect |
|---|---|
| `--relative-to=DIR` | Print the result as a path relative to `DIR` instead of as an absolute path. |
| `--relative-base=DIR` | Print an absolute path, unless the result lies inside `DIR`, in which case print it relative to `DIR`. |
| `-z`, `--zero` | End each output line with a NUL character instead of a newline. |
| `-q`, `--quiet` | Do not print a warning when a path is invalid. See Exit status below for the quiet-mode compatibility warning and script workaround. |

## `realpath` and `readlink`

[`readlink`](~peios/listing-and-paths/readlink) with `-f`/`-e`/`-m` does the same canonicalising job. The difference is focus: `readlink` is primarily for reading a single link's target, with canonicalising as an extra; `realpath` is built for canonicalising and carries the richer option set — relative output, the symlink-handling modes, the strip-only mode. Use `realpath` when canonicalising is the actual goal.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every path was resolved and printed. |
| `1` | A path could not be resolved under the chosen options or its output could not be written. |

**Compatibility warning:** builds using the [uncorrected implementation](https://github.com/peios/peiosutils/blob/3344d4690476fd66bfaec99b1ae92190bbcba06f/src/uu/realpath/src/realpath.rs#L108-L129) can return `0` for invalid operands when `-q` is used. Until your build includes the quiet-status correction, omit `-q` when a script must detect those failures. Successful operands are still printed, so nonempty output alone does not show that every operand succeeded.
