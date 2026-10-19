---
title: uname
type: reference
description: Print system information — the operating system, the kernel, and the machine.
related:
  - peios/system-and-processes/arch
  - peios/system-and-processes/hostname
  - peios/system-and-processes/overview
---

`uname` prints information about the system you are running on.

```
uname [options]
```

```
$ uname
Peios
$ uname -a
Peios host-01 1.4.0 #1 SMP 2026-05-10 x86_64 Peios
```

With no option, `uname` prints the operating-system name — `Peios`.

## What each option prints

| Option | Prints |
|---|---|
| `-s` | The kernel name. |
| `-o` | The operating-system name — `Peios`. |
| `-n` | The node name — the name this system is known by on the network. |
| `-r` | The kernel release. |
| `-v` | The kernel version. |
| `-m` | The machine's hardware name (its architecture). |
| `-p` | The processor type. |
| `-i` | The hardware platform. |
| `-a` | Everything — equivalent to `-mnrsvo`. |

Give several options and `uname` prints those fields, in a fixed order, on one line.

## The operating system is Peios

`uname -o`, and the operating-system field of `uname -a`, report `Peios`. This is the field a script should check when it wants to confirm what system it is on.

For the machine architecture alone, [`arch`](~peios/system-and-processes/arch) is the shorter command — it prints exactly what `uname -m` prints.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The requested information was printed. |
| `1` | The system information could not be read. |
