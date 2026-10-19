---
title: dirname
type: reference
description: Strip the final component off a path, leaving the directory part.
related:
  - peios/listing-and-paths/basename
  - peios/listing-and-paths/overview
---

`dirname` takes a path and prints everything *except* its final component — the directory the name lives in. It is the counterpart of [`basename`](~peios/listing-and-paths/basename).

```
dirname [options] name...
```

```
$ dirname /home/jack/projects/notes.txt
/home/jack/projects
```

Like `basename`, it is a pure text operation — `dirname` never touches the file system, so the path need not exist.

## How it handles edge cases

- If the name has no `/` in it at all, there is no directory part, so `dirname` prints `.` — the current directory.
- Trailing slashes on the name are ignored before the final component is removed.

```
$ dirname notes.txt
.
$ dirname /home/jack/projects/
/home
```

## Multiple names

`dirname` accepts any number of names and prints the directory part of each on its own line:

```
$ dirname /etc/hosts /var/log/messages report.txt
/etc
/var/log
.
```

## Options

| Option | Effect |
|---|---|
| `-z`, `--zero` | End each output line with a NUL character instead of a newline. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A usage error — no name was given. |
