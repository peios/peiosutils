---
title: head
type: reference
description: Print the first lines or bytes of a file.
related:
  - peios/viewing-and-joining-text/tail
  - peios/viewing-and-joining-text/overview
---

`head` prints the **beginning** of a file — by default, the first 10 lines.

```
head [options] [file...]
```

```
$ head config.toml
```

It is the quick way to glance at the top of a file without printing the whole thing — a header, the first few records, the start of a log. With no file, `head` reads standard input.

## Choosing how much

| Option | Effect |
|---|---|
| `-n`, `--lines=NUM` | Print the first `NUM` lines instead of 10. |
| `-c`, `--bytes=NUM` | Print the first `NUM` bytes instead of counting lines. |

`NUM` accepts a unit suffix (`K`, `M`, …) when counting bytes.

### Counting from the end

If `NUM` is given a leading `-`, the meaning flips: `head` prints **everything except the last `NUM`**.

```
$ head -n -5 report.txt    # all but the final 5 lines
```

## Headers for multiple files

When given more than one file, `head` prints a header line before each one so you can tell them apart. Two options override that:

| Option | Effect |
|---|---|
| `-q`, `--quiet` | Never print the file-name headers. |
| `-v`, `--verbose` | Always print the header, even for a single file. |

The header is printed only once the file has actually been opened. A file that exists but cannot be read produces just the error message, with no `==> name <==` line before it.

## Other options

| Option | Effect |
|---|---|
| `-z`, `--zero-terminated` | Treat the NUL character as the line delimiter instead of newline. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every file was printed. |
| `1` | A file could not be read. |
