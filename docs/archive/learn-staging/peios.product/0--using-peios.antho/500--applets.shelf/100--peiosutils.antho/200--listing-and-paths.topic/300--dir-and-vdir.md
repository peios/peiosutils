---
title: dir and vdir
type: reference
description: Two commands that list directory contents with a fixed output style — dir always lists in columns, vdir always lists in long format.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/overview
---

`dir` and `vdir` list the contents of a directory. They are [`ls`](~peios/listing-and-paths/ls) with one decision already made for you: the output style is fixed instead of adapting to where the output is going.

```
dir [options] [file...]
vdir [options] [file...]
```

## What they fix

`ls` chooses its layout based on whether it is writing to a terminal — columns for a terminal, one name per line for a pipe or file. `dir` and `vdir` each pick one layout and always use it:

| Command | Layout | Equivalent `ls` |
|---|---|---|
| `dir` | Columns, regardless of where output goes. | `ls -C` |
| `vdir` | Long format, regardless of where output goes. | `ls -l` |

Both also default to a quoting style that escapes unusual characters in names without wrapping every name in quotes.

That is the *only* difference. The fixed style is just a default — pass an explicit format option (`-1`, `-m`, `-l`, `-C`, …) and it overrides the built-in choice, so `dir -l` produces a long listing and `vdir -C` produces columns.

## Everything else is `ls`

`dir` and `vdir` accept every option `ls` accepts and behave identically in every other respect — sorting, filtering, the long-format columns, colour, indicators, sizes, timestamps. `vdir`'s long format is the same Peios long format documented for `ls`, with the `[type][executable][protected]` mode column and the owner SID.

For the full option set and the meaning of every long-format column, see [`ls`](~peios/listing-and-paths/ls). This page exists only to explain how `dir` and `vdir` differ from it — and the answer is "they pin the output style."

## Exit status

The same as `ls`:

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A minor problem — a named file could not be accessed. |
| `2` | A serious problem — a directory could not be read, or an option was invalid. |
