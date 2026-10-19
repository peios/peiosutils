---
title: nice
type: reference
description: Run a command at an adjusted scheduling priority.
related:
  - peios/system-and-processes/overview
---

`nice` runs a command at an adjusted **scheduling priority** — making it more, or less, willing to give the processor up to other work.

```
nice [options] [command [arg]...]
```

```
$ nice -n 15 big-batch-job
```

With no command, `nice` prints the current niceness.

## Niceness

A process has a *niceness* — a number from **-20** to **19**:

- A **high** niceness (toward 19) means the process is "nicer" to others — it yields the processor readily. Good for background work that should not get in the way.
- A **low** niceness (toward -20) means the process is favoured — it gets the processor more often.

By default `nice` adds **10** to the niceness, making the command run in the background more gracefully.

| Option | Effect |
|---|---|
| `-n`, `--adjustment=N` | Add `N` to the niceness instead of 10. `N` may be negative. |

Raising a command's niceness — running it *lower* priority — is always allowed. Lowering the niceness, to claim a *higher* priority than normal, is a privileged request and may be refused; when it is, `nice` warns and runs the command at the priority it was permitted.

## Exit status

When `nice` runs a command, it exits with **that command's** status. The exception is a failure in `nice` itself:

| Code | Meaning |
|---|---|
| (command's own) | The command ran; this is its exit status. |
| `0` | No command was given; the niceness was printed. |
| `125` | `nice` itself failed — for example, a bad adjustment value. |
| `126` | The command was found but could not be run. |
| `127` | The command was not found. |
