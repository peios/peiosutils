---
title: expr
type: reference
description: Evaluate an arithmetic, string, or comparison expression and print the result.
related:
  - peios/output-and-evaluation/test
  - peios/output-and-evaluation/factor
  - peios/output-and-evaluation/overview
---

`expr` evaluates a single expression and prints its value.

```
expr expression
```

```
$ expr 6 + 7
13
$ expr length "Peios"
5
```

Each part of the expression — every number, operator, and keyword — is a separate argument. That is why the spaces matter: `expr 6+7` is one argument and not an expression, while `expr 6 + 7` is three.

## Arithmetic

| Operator | Result |
|---|---|
| `ARG1 + ARG2` | Sum. |
| `ARG1 - ARG2` | Difference. |
| `ARG1 * ARG2` | Product. |
| `ARG1 / ARG2` | Integer quotient. |
| `ARG1 % ARG2` | Remainder. |

## Comparison

A comparison yields `1` for true and `0` for false. It is arithmetic when both sides are numbers, and lexical otherwise.

| Operator | True when… |
|---|---|
| `ARG1 = ARG2` | the two are equal. |
| `ARG1 != ARG2` | they are unequal. |
| `ARG1 < ARG2` | `ARG1` is less. |
| `ARG1 <= ARG2` | `ARG1` is less or equal. |
| `ARG1 > ARG2` | `ARG1` is greater. |
| `ARG1 >= ARG2` | `ARG1` is greater or equal. |

## Logic

| Operator | Result |
|---|---|
| `ARG1 \| ARG2` | `ARG1` if it is neither null nor `0`; otherwise `ARG2`. |
| `ARG1 & ARG2` | `ARG1` if neither argument is null or `0`; otherwise `0`. |

## String operations

| Form | Result |
|---|---|
| `length STRING` | The number of characters in `STRING`. |
| `substr STRING POS LENGTH` | `LENGTH` characters of `STRING`, counting `POS` from 1. |
| `index STRING CHARS` | The position of the first of any of `CHARS` in `STRING`, or `0`. |
| `STRING : REGEXP` | Match `REGEXP` anchored at the start of `STRING`. Returns the matched length, or the text captured by `\(…\)`. |
| `match STRING REGEXP` | The same as `STRING : REGEXP`. |

Many of these operators — `*`, `<`, `|`, `(` — are also meaningful to a shell, so quote or escape them so they reach `expr` intact.

## Exit status

`expr`'s exit status reports on the *value* it computed:

| Code | Meaning |
|---|---|
| `0` | The result was neither null nor `0`. |
| `1` | The result was null or `0`. |
| `2` | The expression was syntactically invalid. |
| `3` | An error occurred during evaluation — such as division by zero. |
