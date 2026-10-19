---
title: numfmt
type: reference
description: Convert numbers between plain digits and human-readable forms.
related:
  - peios/output-and-evaluation/overview
---

`numfmt` converts numbers between plain digits and **human-readable** forms — turning `1500000` into `1.5M`, or `1.5M` back into `1500000`.

```
numfmt [options] [number...]
```

```
$ numfmt --to=si 1500000
1.5M
$ numfmt --from=si 1.5M
1500000
```

`numfmt` takes numbers as arguments, or — with none — reads them from standard input, which lets it rescale a column of numbers flowing through a pipe.

## The direction of conversion

| Option | Effect |
|---|---|
| `--from=UNIT` | Parse input numbers that carry a `UNIT` suffix, scaling them up to plain digits. |
| `--to=UNIT` | Scale output numbers down and add a `UNIT` suffix. |

A `UNIT` is one of:

| Unit | Suffixes mean |
|---|---|
| `none` | No suffixes — a suffix is an error. The default. |
| `si` | Powers of 1000: `1K` = 1000, `1M` = 1000000. |
| `iec` | Powers of 1024: `1K` = 1024, `1M` = 1048576. |
| `iec-i` | Powers of 1024 with a two-letter suffix: `1Ki` = 1024, `1Mi` = 1048576. |
| `auto` | Accept any of the above on input, reading `Ki`/`Mi` as powers of 1024 and bare `K`/`M` as powers of 1000. |

## Working on fields

By default `numfmt` converts the whole input line. To convert numbers sitting in a *column* of wider text, name the field:

| Option | Effect |
|---|---|
| `--field=FIELDS` | Convert only the numbers in these fields. `FIELDS` uses the same range syntax as `cut` — `3`, `2-5`, `2-`. |
| `-d`, `--delimiter=X` | Use `X` to separate fields instead of whitespace. |
| `--header[=N]` | Pass the first `N` lines through unconverted, as a header. |

## Shaping the output

| Option | Effect |
|---|---|
| `--format=FORMAT` | Format the number with a `printf`-style floating-point `FORMAT`, controlling width, padding, and precision. |
| `--padding=N` | Pad the output to `N` columns — positive right-aligns, negative left-aligns. |
| `--grouping` | Group digits according to the locale (for example, `1,000,000`). |
| `--round=METHOD` | Choose the rounding method used when scaling. |
| `--suffix=SUFFIX` | Append `SUFFIX` to each result, and accept it on input. |
| `--from-unit=N` / `--to-unit=N` | Set the unit size the input or output is counted in. |

## Other options

| Option | Effect |
|---|---|
| `--invalid=MODE` | Choose what to do with input that is not a valid number: `abort`, `fail`, `warn`, or `ignore`. |
| `-z`, `--zero-terminated` | Treat the NUL character as the line delimiter. |
| `--debug` | Print warnings about questionable input. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every number was converted. |
| `2` | An input was not a valid number, or an option was misused. |
