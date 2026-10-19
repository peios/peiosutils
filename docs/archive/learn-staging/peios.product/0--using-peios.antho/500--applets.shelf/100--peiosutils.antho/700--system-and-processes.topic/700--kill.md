---
title: kill
type: reference
description: Send a signal to a process.
related:
  - peios/system-and-processes/timeout
  - peios/system-and-processes/overview
---

`kill` sends a **signal** to a process. Despite the name, a signal is not always fatal — it is a message, and some signals ask a process to do something other than stop.

```
kill [options] pid...
```

```
$ kill 4821                 # ask process 4821 to terminate
$ kill -KILL 4821           # force it to stop
```

A `pid` is a process identifier — the number a running process is known by.

## Signals

With no option, `kill` sends `TERM` — a polite request to shut down, which a well-behaved process honours by cleaning up and exiting. The signals you will use most:

| Signal | Meaning |
|---|---|
| `TERM` | Terminate. The default — a request to stop cleanly. |
| `KILL` | Stop immediately. Cannot be caught or ignored, so it cannot be refused — but the process gets no chance to clean up. The last resort. |
| `HUP` | Hang up. Conventionally tells a long-running service to reload its configuration. |
| `INT` | Interrupt — the signal a terminal sends on Ctrl-C. |
| `STOP` / `CONT` | Suspend the process / resume a suspended one. |

| Option | Effect |
|---|---|
| `-s`, `--signal=SIGNAL` | Send `SIGNAL` instead of `TERM`. |
| `-SIGNAL` | A shorthand — `-KILL` or `-9` both send `KILL`. |
| `-l`, `--list` | List the signal names. |
| `-L`, `--table` | List the signals as a table of numbers and names. |

Use `KILL` only when `TERM` has failed. A process killed outright cannot save its work or release what it holds.

A `-N` that is not a known signal number is an error ("invalid signal"). It is never reinterpreted as a negative process ID.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every signal was sent. |
| `1` | A signal could not be sent — a process that does not exist, or one you are not permitted to signal. |
