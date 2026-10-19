---
title: hostname
type: reference
description: Display or set the system's host name.
related:
  - peios/system-and-processes/uname
  - peios/privileges/overview
  - peios/system-and-processes/overview
---

`hostname` displays the system's **host name** — the name the system is known by.

```
hostname [options] [name]
```

```
$ hostname
host-01.example.com
```

## What is displayed

By default `hostname` prints the fully qualified name. These options narrow or change what is shown:

| Option | Prints |
|---|---|
| `-f`, `--fqdn` | The fully qualified domain name — the full `host.domain` form. This is the default. |
| `-s`, `--short` | The short host name only — the part before the first dot. |
| `-d`, `--domain` | The DNS domain part only. |
| `-i`, `--ip-address` | The network address (or addresses) the host name resolves to. |

## Setting the host name

Given a `name` argument, `hostname` sets the system's host name to it.

```
$ hostname host-02
```

Changing the host name changes state for the whole system, so it is a **privileged** operation — it succeeds only for a caller whose token carries the right to do it, and is refused otherwise. See [Privileges](~peios/privileges/overview).

## Exit status

| Code | Meaning |
|---|---|
| `0` | The host name was displayed or set. |
| `1` | A failure — the name could not be read, set, or resolved. |
