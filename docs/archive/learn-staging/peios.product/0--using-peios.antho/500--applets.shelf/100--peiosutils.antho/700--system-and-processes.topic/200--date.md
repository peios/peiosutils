---
title: date
type: reference
description: Print or set the system date and time, with a flexible output format.
related:
  - peios/system-and-processes/uptime
  - peios/privileges/overview
  - peios/system-and-processes/overview
---

`date` prints the current date and time. With the right option, it can also *set* the system clock, display some other moment, or format a date however you need.

```
date [options] [+format]
```

```
$ date
Sun 17 May 2026 14:32:08 BST
$ date +%Y-%m-%d
2026-05-17
```

## Formatting the output

A `+format` argument controls the output. The format string is printed literally, except for `%` sequences, each of which is replaced by a piece of the date. The common ones:

| Sequence | Is replaced by |
|---|---|
| `%Y` `%m` `%d` | Year, month, day. |
| `%H` `%M` `%S` | Hour, minute, second. |
| `%F` | The full date — same as `%Y-%m-%d`. |
| `%T` | The time — same as `%H:%M:%S`. |
| `%A` `%a` | Weekday name, full and abbreviated. |
| `%B` `%b` | Month name, full and abbreviated. |
| `%j` | Day of the year. |
| `%s` | Seconds since 1970-01-01 UTC. |
| `%z` `%Z` | Numeric and named time zone. |
| `%%` | A literal `%`. |

The full set is long; `date --help` lists every sequence with an example.

## Displaying a different moment

| Option | Effect |
|---|---|
| `-d`, `--date=STRING` | Show the time described by `STRING` — `"yesterday"`, `"next Friday"`, `"@1615432800"` — instead of now. |
| `-r`, `--reference=FILE` | Show `FILE`'s last modification time. |
| `-f`, `--file=DATEFILE` | Like `-d`, but once for each line of `DATEFILE`. |
| `-u`, `--universal` | Work in Coordinated Universal Time (UTC) rather than the local zone. |

## Standard formats

| Option | Output |
|---|---|
| `-I`, `--iso-8601[=FMT]` | ISO 8601 — `FMT` is `date`, `hours`, `minutes`, `seconds`, or `ns`. |
| `-R`, `--rfc-email` | The format used in email headers. |
| `--rfc-3339=FMT` | RFC 3339 — `FMT` is `date`, `seconds`, or `ns`. |

## Setting the clock

| Option | Effect |
|---|---|
| `-s`, `--set=STRING` | Set the system clock to the time described by `STRING`. |

Setting the clock changes state for the whole system, so it is a **privileged** operation — it succeeds only for a caller whose token carries the right to set the time, and is refused otherwise. See [Privileges](~peios/privileges/overview).

## Exit status

| Code | Meaning |
|---|---|
| `0` | The date was printed or set. |
| `1` | An invalid date or format, or the clock could not be set. |
