---
title: unexpand
type: reference
description: Convert runs of spaces in a file back into tab characters.
related:
  - peios/transforming-text/expand
  - peios/transforming-text/overview
---

`unexpand` is the reverse of [`expand`](~peios/transforming-text/expand): it converts runs of spaces back into tab characters.

```
unexpand [options] [file...]
```

```
$ unexpand spaced.txt > tabbed.txt
```

Where a run of spaces reaches a tab stop, `unexpand` replaces it with a tab. By default it only converts the **leading** spaces on each line — the indentation — and leaves spacing within the text alone. With no file, it reads standard input.

## Options

| Option | Effect |
|---|---|
| `-t`, `--tabs=N` | Set tab stops every `N` columns instead of the default 8. A comma-separated list places stops at explicit column positions. Using `-t` also enables `-a`. |
| `-a`, `--all` | Convert *all* runs of spaces that reach a tab stop, not only the leading indentation. |
| `--first-only` | Convert only the leading run of blanks on each line. Overrides `-a`. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A file could not be read, or a tab specification was invalid. |
