---
title: tsort
type: reference
description: Topologically sort items so that each comes after the things it depends on.
related:
  - peios/transforming-text/sort
  - peios/transforming-text/overview
---

`tsort` performs a **topological sort**. Given a set of "this must come before that" rules, it produces an order in which everything appears after the things it depends on.

```
tsort [file]
```

```
$ tsort dependencies.txt
```

With no file, `tsort` reads standard input.

## The input

`tsort` reads its input as a flat sequence of whitespace-separated tokens, taken **in pairs**. Each pair `A B` means "A must come before B" — an edge in a dependency graph.

```
compiler  linker
linker    installer
compiler  installer
```

That input says the linker depends on the compiler, the installer on the linker, and the installer on the compiler. `tsort` reads the pairs and prints the items in a valid order:

```
compiler
linker
installer
```

A token paired with itself (`A A`) simply introduces `A` with no dependency — a way to list an item that nothing else mentions.

## Cycles

A topological order only exists if the dependencies form no **cycle**. If A must come before B and B must come before A, there is no valid order. `tsort` detects this, reports the loop it found, and exits with a failure status — it still prints an order, but the cycle means that order cannot satisfy every rule.

## Where it is used

`tsort` answers "what order do I do these in?" — building components in dependency order, scheduling tasks, sequencing anything where some steps must precede others. It is the dependency-aware counterpart to [`sort`](~peios/transforming-text/sort), which orders by value rather than by dependency.

## Exit status

| Code | Meaning |
|---|---|
| `0` | A valid order was produced. |
| `1` | The input contained a cycle, had an odd number of tokens, or could not be read. |
