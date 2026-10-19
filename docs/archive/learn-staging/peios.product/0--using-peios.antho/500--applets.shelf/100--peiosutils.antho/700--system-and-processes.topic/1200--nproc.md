---
title: nproc
type: reference
description: Print how many processor cores are available.
related:
  - peios/system-and-processes/overview
---

`nproc` prints the number of processor cores available.

```
nproc [options]
```

```
$ nproc
8
```

By default it prints the number of cores available **to the current process** — which can be fewer than the machine has, if the process has been restricted to a subset. It is most often used to decide how many parallel jobs to run.

## Options

| Option | Effect |
|---|---|
| `--all` | Print the number of cores the *whole system* has, ignoring any restriction on the current process. |
| `--ignore=N` | Subtract `N` from the count — for leaving some cores free rather than using every one. |

The environment variables `OMP_NUM_THREADS` and `OMP_THREAD_LIMIT`, if set, also bound the number `nproc` reports.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The count was printed. |
| `1` | An option value was invalid. |
