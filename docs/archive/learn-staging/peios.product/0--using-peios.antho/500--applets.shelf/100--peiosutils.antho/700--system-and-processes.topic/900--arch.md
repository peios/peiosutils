---
title: arch
type: reference
description: Print the machine's hardware architecture.
related:
  - peios/system-and-processes/uname
  - peios/system-and-processes/overview
---

`arch` prints the machine's hardware architecture — the kind of processor the system runs on.

```
arch
```

```
$ arch
x86_64
```

That is the whole command. It takes no options and prints a single name.

`arch` prints exactly what [`uname -m`](~peios/system-and-processes/uname) prints. It exists as a short, obvious name for the one question "what architecture is this?" — convenient in a script that picks an architecture-specific path.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The architecture was printed. |
| `1` | The architecture could not be determined. |
