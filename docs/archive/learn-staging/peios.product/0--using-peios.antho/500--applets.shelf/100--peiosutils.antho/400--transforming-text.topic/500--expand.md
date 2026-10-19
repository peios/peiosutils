---
title: expand
type: reference
description: Convert tab characters in a file into spaces.
related:
  - peios/transforming-text/unexpand
  - peios/transforming-text/overview
---

`expand` converts tab characters into spaces, so that text lines up the same way no matter what tab width something is viewed with.

```
expand [options] [file...]
```

```
$ expand source.txt > source-spaces.txt
```

A tab is not a fixed number of spaces — it jumps to the next tab stop, and where those stops are is a display setting. `expand` removes the ambiguity: it replaces each tab with exactly the number of spaces needed to reach the same column, baking the layout in. With no file, it reads standard input.

[`unexpand`](~peios/transforming-text/unexpand) is the reverse operation.

## Options

| Option | Effect |
|---|---|
| `-t`, `--tabs=N` | Set tab stops every `N` columns instead of the default 8. |
| `-t`, `--tabs=LIST` | Given a comma-separated list, place tab stops at those explicit column positions. |
| `-i`, `--initial` | Convert only the tabs that come before the first non-blank character on each line — leave tabs *within* the text alone. Useful for re-indenting code without disturbing aligned tabs inside it. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read, or a tab specification was invalid. |
