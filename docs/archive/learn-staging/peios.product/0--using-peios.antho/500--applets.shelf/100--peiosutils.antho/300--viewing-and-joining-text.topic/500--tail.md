---
title: tail
type: reference
description: Print the last lines of a file — and keep printing as the file grows.
related:
  - peios/viewing-and-joining-text/head
  - peios/viewing-and-joining-text/overview
---

`tail` prints the **end** of a file — by default, the last 10 lines.

```
tail [options] [file...]
```

```
$ tail server.log
```

It is the counterpart of [`head`](~peios/viewing-and-joining-text/head): the quick way to see the most recent lines of a file. With no file, `tail` reads standard input.

## Choosing how much

| Option | Effect |
|---|---|
| `-n`, `--lines=NUM` | Print the last `NUM` lines instead of 10. |
| `-c`, `--bytes=NUM` | Print the last `NUM` bytes instead of counting lines. |

With `-n`, if `NUM` is given a leading `+`, the meaning flips: `tail` prints **everything from line `NUM` onward**.

```
$ tail -n +20 report.txt    # from line 20 to the end
```

## Following a growing file

`tail`'s most useful trick is `-f`. Instead of printing the end and exiting, it stays running and prints new lines as they are appended — the standard way to watch a log file live.

| Option | Effect |
|---|---|
| `-f`, `--follow` | Keep the file open and print new data as it is added. |
| `-F` | Follow the file *by name*, and keep retrying if it is missing. Equivalent to `--follow=name --retry`. This survives log rotation — when the file is replaced, `-F` picks up the new one. |
| `--retry` | Keep trying to open a file that is not yet accessible. |
| `--pid=PID` | While following, stop once process `PID` exits. |
| `-s`, `--sleep-interval=N` | Wait `N` seconds between checks of the file. |

Following by name picks up a *file* that replaces the watched one. If what appears under the name is a symbolic link, `tail` reports that the file "has been replaced with an untailable symbolic link" and stops reading it, rather than following the link to wherever it points.

Press Ctrl-C to stop a `tail -f`.

## Headers for multiple files

As with `head`, when given more than one file `tail` prints a header before each.

| Option | Effect |
|---|---|
| `-q`, `--quiet` | Never print the file-name headers. |
| `-v`, `--verbose` | Always print the header, even for a single file. |

## Other options

| Option | Effect |
|---|---|
| `-z`, `--zero-terminated` | Treat the NUL character as the line delimiter instead of newline. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read. |
