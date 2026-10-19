---
title: true and false
type: reference
description: Two commands that do nothing — one always succeeds, the other always fails.
related:
  - peios/output-and-evaluation/test
  - peios/output-and-evaluation/overview
---

`true` and `false` do nothing at all. Their only product is an **exit status**: `true` always succeeds, `false` always fails.

```
true
false
```

```
$ true ; echo $?
0
$ false ; echo $?
1
```

`true` exits `0`. `false` exits `1`. They take no arguments that matter, produce no output, and have no effect on anything.

## What they are for

A command that is *only* an exit status is useful as a fixed value in places that expect a command:

- **An always-true or always-false loop.** `while true` is the standard way to write a loop that runs until something inside it breaks out.
- **A deliberate placeholder.** Setting a configurable command to `true` makes that step do nothing and "succeed"; `false` makes it a step that always fails.
- **A known result in a test.** When a script or a condition needs a guaranteed pass or fail to check against, `true` and `false` provide it.

They are the simplest commands there are — and the [overview](~peios/output-and-evaluation/overview)'s note on exit status is the whole idea behind them.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Returned by `true`, always. |
| `1` | Returned by `false`, always. |
