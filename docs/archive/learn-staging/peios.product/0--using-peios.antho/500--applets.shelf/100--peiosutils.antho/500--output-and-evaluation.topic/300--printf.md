---
title: printf
type: reference
description: Print formatted text from a format string and a list of arguments.
related:
  - peios/output-and-evaluation/echo
  - peios/output-and-evaluation/overview
---

`printf` prints text built from a **format string**. The format string is printed literally, except that escape sequences become characters and substitution fields are replaced by the arguments that follow.

```
printf format [argument...]
```

```
$ printf 'the letter %X comes before %X\n' 10 11
the letter A comes before B
```

Where [`echo`](~peios/output-and-evaluation/echo) prints arguments as-is, `printf` gives you control: padding, column widths, number bases, fixed decimal places. It is the command for output that has to *line up* or *be exact*.

`printf` writes only what the format string says — there is no automatic trailing newline. Include `\n` yourself where you want one.

## Escape sequences

The format string interprets these backslash sequences:

| Sequence | Character |
|---|---|
| `\\` | Backslash. |
| `\n` `\t` `\r` | Newline, tab, carriage return. |
| `\b` `\f` `\v` `\a` | Backspace, form feed, vertical tab, alert. |
| `\e` | Escape. |
| `\NNN` | The byte with octal value `NNN`. |
| `\xHH` | The byte with hexadecimal value `HH`. |
| `\uHHHH`, `\UHHHHHHHH` | A Unicode character by hexadecimal code point. |
| `%%` | A literal `%`. |

## Substitution fields

A field begins with `%` and is replaced by the next argument, formatted accordingly.

| Field | Formats the argument as… |
|---|---|
| `%s` | a string. |
| `%b` | a string, with backslash escapes in it interpreted. |
| `%q` | a string, quoted so it is safe to reuse as shell input. |
| `%c` | a single character. |
| `%d`, `%i` | a signed integer. |
| `%u` | an unsigned integer. |
| `%x`, `%X` | an unsigned integer in hexadecimal (lower- or upper-case). |
| `%o` | an unsigned integer in octal. |
| `%f`, `%F` | a decimal floating-point number. |
| `%e`, `%E` | a number in scientific notation. |
| `%g`, `%G` | whichever of decimal or scientific notation is shorter. |

## Width and precision

Between the `%` and the field letter you may put two numbers — `%[width][.precision]field`:

- **width** — the minimum number of columns. Output narrower than this is padded with leading spaces; a negative width pads on the right instead.
- **precision** — after a `.`; its meaning depends on the field. For a string it is a *maximum length*; for an integer, a minimum digit count (zero-padded); for a float, the number of decimal places.

```
$ printf '%-10s %5.2f\n' apples 3.1
apples         3.10
```

## Reusing the format string

If more arguments are supplied than the format string has fields, `printf` **repeats the format string** until the arguments run out. If there are too few, the leftover fields default to an empty string (or `0` for a number).

```
$ printf '%s: %d\n' alpha 1 beta 2 gamma 3
alpha: 1
beta: 2
gamma: 3
```

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A missing operand, a bad format string, or a write failure. |
