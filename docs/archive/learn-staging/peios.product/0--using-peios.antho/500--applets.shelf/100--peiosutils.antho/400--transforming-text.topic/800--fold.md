---
title: fold
type: reference
description: Hard-wrap long lines at a fixed width.
related:
  - peios/transforming-text/fmt
  - peios/transforming-text/overview
---

`fold` breaks long lines so that none exceeds a fixed width. Every line longer than the limit is cut into pieces.

```
fold [options] [file...]
```

```
$ fold -w 80 wide.txt
```

By default `fold` wraps at 80 columns, and it cuts at exactly that column — even in the middle of a word. That hard cut is the difference between `fold` and [`fmt`](~peios/transforming-text/fmt): `fmt` reflows paragraphs and breaks between words; `fold` just enforces a width, mechanically. Use `fold` when you need a guarantee that no line is wider than `N`; use `fmt` when you want the result to read well.

With no file, `fold` reads standard input.

## Options

| Option | Effect |
|---|---|
| `-w`, `--width=WIDTH` | Wrap at `WIDTH` columns instead of 80. |
| `-s`, `--spaces` | Break at a space within the width where one exists, rather than mid-word — a gentler wrap. |
| `-b`, `--bytes` | Count bytes rather than display columns. Control characters such as tab and newline then count as ordinary bytes. |
| `-c`, `--characters` | Count character positions rather than display columns. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read, or the width was invalid. |
