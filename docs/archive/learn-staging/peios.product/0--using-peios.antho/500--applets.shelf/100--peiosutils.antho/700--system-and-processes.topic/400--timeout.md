---
title: timeout
type: reference
description: Run a command and kill it if it is still running after a time limit.
related:
  - peios/system-and-processes/kill
  - peios/system-and-processes/sleep
  - peios/system-and-processes/overview
---

`timeout` runs a command with a time limit. If the command finishes in time, nothing special happens. If it is still running when the limit is reached, `timeout` kills it.

```
timeout [options] duration command...
```

```
$ timeout 30s slow-fetch https://example.com
```

It is the guard against a command that might hang — a network fetch, a script that could loop forever.

## The duration

`duration` is a number with an optional unit suffix: `s` for seconds (the default), `m` for minutes, `h` for hours, `d` for days. A duration of `0` disables the limit.

## How the command is stopped

When the limit is reached, `timeout` sends the command a `TERM` signal — a request to shut down cleanly. A well-behaved command stops there. A command that ignores `TERM` keeps running, which is what `-k` is for.

| Option | Effect |
|---|---|
| `-s`, `--signal=SIGNAL` | Send `SIGNAL` instead of `TERM` when the limit is reached. |
| `-k`, `--kill-after=DURATION` | If the command is *still* running `DURATION` after the first signal, send it a `KILL` signal — which cannot be ignored. |
| `--preserve-status` | Exit with the command's own status even when it timed out. |
| `--foreground` | Allow the command to keep reading from the terminal; do not time out the command's own children. |
| `-v`, `--verbose` | Report to standard error whenever a signal is sent. |

## Exit status

`timeout` passes through the command's exit status when the command finishes on its own. Otherwise:

| Code | Meaning |
|---|---|
| (command's own) | The command finished within the limit. |
| `124` | The command timed out. |
| `125` | `timeout` itself failed. |
| `126` | The command was found but could not be run. |
| `127` | The command was not found. |
| `137` | The command was ended by a `KILL` signal. |
