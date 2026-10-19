---
title: Displaying a Time in UTC
type: how-to
description: >-
  Print an unambiguous timestamp without changing the system clock.
related:
  - peios/system-and-processes/uptime
  - peios/system-and-processes/overview
---

`date` displays a time using a chosen representation. A timestamp with a
stated time zone is easier to exchange than a local clock reading alone.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
date -u '+%Y-%m-%dT%H:%M:%SZ'
```

## Check the result

The output contains the current date and time in Coordinated Universal Time
(UTC), with a final `Z` indicating that zone.

## Separate display from setting the clock

Formatting a time does not change the system clock. Parsing a supplied date
and setting the clock are also different operations. Setting requires the
relevant authority and can affect scheduled work and timestamp comparisons.

The process environment can select a display time zone. Keep that choice
explicit in scripts that exchange timestamps. See `date(1)` for parsing,
format fields, and setting behavior.
