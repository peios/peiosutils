---
title: factor
type: reference
description: Print the prime factors of numbers.
related:
  - peios/output-and-evaluation/expr
  - peios/output-and-evaluation/overview
---

`factor` prints the prime factors of a number.

```
factor [options] [number...]
```

```
$ factor 360
360: 2 2 2 3 3 5
```

Each line of output is the original number, a colon, and its prime factors in increasing order, repeated as often as they divide in. Give several numbers and `factor` factors each; give none and it reads numbers from standard input.

## Options

| Option | Effect |
|---|---|
| `-h`, `--exponents` | Write a repeated factor in exponent form — `p^e` — instead of repeating it. |

```
$ factor -h 360
360: 2^3 3^2 5
```

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every number was factored. |
| `1` | An input was not a valid positive integer. |
