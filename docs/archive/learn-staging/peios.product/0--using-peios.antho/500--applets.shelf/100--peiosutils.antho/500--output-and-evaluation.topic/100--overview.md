---
title: Output and evaluation
type: reference
description: The commands that produce text, work with the environment, convert numbers, and evaluate expressions and conditions.
related:
  - peios/output-and-evaluation/echo
  - peios/output-and-evaluation/test
---

This topic gathers the small, sharp commands that scripts are built from: the ones that **print** something, that read or set the **environment**, that **evaluate** an expression or a condition, and that simply **succeed or fail**.

## The commands

**Producing text**

| Command | Purpose |
|---|---|
| [`echo`](~peios/output-and-evaluation/echo) | Print its arguments as a line of text. |
| [`printf`](~peios/output-and-evaluation/printf) | Print text from a format string, with precise control over layout. |
| [`yes`](~peios/output-and-evaluation/yes) | Print a line over and over, without stopping. |
| [`seq`](~peios/output-and-evaluation/seq) | Print a sequence of numbers. |
| [`tee`](~peios/output-and-evaluation/tee) | Copy standard input to standard output and to each named file. |

**The environment**

| Command | Purpose |
|---|---|
| [`env`](~peios/output-and-evaluation/env) | Run a command with a modified environment — or print the current one. |
| [`printenv`](~peios/output-and-evaluation/printenv) | Print environment variables. |

**Numbers**

| Command | Purpose |
|---|---|
| [`numfmt`](~peios/output-and-evaluation/numfmt) | Convert numbers to and from human-readable forms (`1.5G`, `1000000`). |
| [`factor`](~peios/output-and-evaluation/factor) | Print the prime factors of a number. |

**Evaluating**

| Command | Purpose |
|---|---|
| [`expr`](~peios/output-and-evaluation/expr) | Evaluate an arithmetic, string, or comparison expression. |
| [`test`](~peios/output-and-evaluation/test) | Evaluate a condition — a file check or a comparison — as a true/false result. |

**Exit status**

| Command | Purpose |
|---|---|
| [`true` and `false`](~peios/output-and-evaluation/true-and-false) | Do nothing, and succeed — or do nothing, and fail. |

## A note on exit status

Several commands here exist to be used for their **exit status** rather than their output. Every command, when it finishes, leaves behind a number: `0` means success, anything else means a kind of failure. A shell's `if`, `&&`, and `||` all act on that number.

[`test`](~peios/output-and-evaluation/test) is the clearest case — it prints nothing and exists *only* to set an exit status from a condition. [`true` and `false`](~peios/output-and-evaluation/true-and-false) are the most minimal: fixed success and fixed failure. Where this topic's pages give an exit-status table, that number is often the whole point of the command.

## Where to start

For everyday printing, [`echo`](~peios/output-and-evaluation/echo); for anything that needs columns, padding, or precise formatting, [`printf`](~peios/output-and-evaluation/printf).
