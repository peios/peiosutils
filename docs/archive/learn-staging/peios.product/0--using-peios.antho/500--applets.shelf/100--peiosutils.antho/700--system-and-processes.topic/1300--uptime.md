---
title: uptime
type: reference
description: Show how long the system has been running.
related:
  - peios/system-and-processes/date
  - peios/system-and-processes/overview
---

`uptime` shows how long the system has been running, along with a few related figures.

```
uptime [options]
```

```
$ uptime
 14:32:08  up 6 days,  3:21,  load average: 0.18, 0.24, 0.21
```

The default line packs in three things:

- the **current time**;
- how long the system has been **up**;
- the **load average** — a rough measure of how busy the system has been over the last 1, 5, and 15 minutes.

If you know `uptime` from other systems you may be expecting a count of logged-in users between the uptime and the load average. Peios does not print one. That count comes from a login database (`utmp`) that Peios does not keep, so the field is left out rather than always reporting `0 users`.

To inspect Peios logon sessions and their processes, use [`logonse`](~peios/system-and-processes/logonse). Logon sessions record authentication events, including service and batch logons; their count is not a count of logged-in users.

## Options

| Option | Effect |
|---|---|
| `-p`, `--pretty` | Show just the uptime, in a readable phrase — `up 6 days, 3 hours, 21 minutes`. |
| `-s`, `--since` | Show the date and time the system started, rather than how long ago that was. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The uptime was printed. |
| `1` | The boot time could not be read. |
