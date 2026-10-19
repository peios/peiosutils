---
title: hostid
type: reference
description: Print the host's numeric identifier.
related:
  - peios/system-and-processes/hostname
  - peios/system-and-processes/overview
---

`hostid` prints the host's numeric identifier, in hexadecimal.

```
hostid
```

```
$ hostid
007f0100
```

The host id is a number associated with the system. It takes no options and prints one value.

Where [`hostname`](~peios/system-and-processes/hostname) gives the system's *name* — meant to be read by people — `hostid` gives a numeric value, used by software that wants a machine identifier in a fixed numeric form.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The host id was printed. |
