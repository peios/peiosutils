---
title: more
type: reference
description: View a file one screen at a time, pausing between screens.
related:
  - peios/viewing-and-joining-text/cat
  - peios/viewing-and-joining-text/overview
---

`more` displays a file one screenful at a time. Where [`cat`](~peios/viewing-and-joining-text/cat) prints a whole file at once — and a long file scrolls straight past — `more` shows the first screen, then waits for you before showing the next.

```
more [options] file...
```

```
$ more long-report.txt
```

## Moving through the file

While `more` is paused, it is waiting for a keypress:

| Key | Action |
|---|---|
| Space | Show the next screenful. |
| Return | Show the next line. |
| `q` | Quit. |
| `/` | Search forward for a string. |
| `h` | Show the built-in help. |

`more` moves *forward* through a file. It is the simple, always-available pager — enough for reading something through once.

## Options

| Option | Effect |
|---|---|
| `-n`, `--lines=NUM` | Show `NUM` lines per screenful instead of filling the terminal. `--number=NUM` is the same. |
| `-F`, `--from-line=NUM` | Begin displaying at line `NUM`. |
| `-P`, `--pattern=STRING` | Search for `STRING` and begin displaying at the first match. |
| `-e`, `--exit-on-eof` | Exit automatically at the end of the file, rather than waiting. |
| `-s`, `--squeeze` | Collapse runs of blank lines into one. |
| `-u`, `--plain` | Suppress underlining in the displayed text. |
| `-p`, `--print-over` | Clear the screen and print the next page, instead of scrolling. |
| `-c`, `--clean-print` | Redraw each page in place, cleaning line ends, instead of scrolling. |
| `-d`, `--silent` | When an unrecognised key is pressed, show a short hint instead of ringing the terminal bell. |
| `-l`, `--logical` | Do not pause when a line contains a form-feed character. |
| `-f`, `--no-pause` | Count logical lines rather than screen lines when deciding where to pause. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The file was displayed. |
| `1` | A file could not be opened. |
