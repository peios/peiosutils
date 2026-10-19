---
title: sleep
type: reference
description: Pause for a given length of time.
related:
  - peios/system-and-processes/timeout
  - peios/system-and-processes/overview
---

`sleep` does nothing for a given length of time, then exits.

```
sleep number[suffix]...
```

```
$ sleep 5          # pause for five seconds
$ sleep 1.5h       # pause for an hour and a half
```

It is used to space things out — a pause between the steps of a script, a delay before a retry.

## Specifying the duration

`number` may be a whole number or a fraction. A `suffix` gives the unit:

| Suffix | Unit |
|---|---|
| `s` | Seconds. This is the default if no suffix is given. |
| `m` | Minutes. |
| `h` | Hours. |
| `d` | Days. |

Given more than one argument, `sleep` waits for the **sum** of them — `sleep 1m 30s` pauses for ninety seconds.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The full time elapsed. |
| `1` | A bad argument, or `sleep` was interrupted before the time was up. |
