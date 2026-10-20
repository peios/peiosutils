---
title: Reading System Information
type: how-to
description: Read the time, the system's identifiers and host name, the processors available, uptime and load, and terminal settings, and know what each value doesn't tell you.
related:
  - peios/time/overview
  - peios/scripts-and-the-system/running-and-controlling-commands
---

Use these commands to read facts about the running system: the time, what is
running, how much processing capacity is available, and the terminal you are
using. Each value describes one moment, so don't rely on it staying the same.
The man page for each command lists every option.

## Read the time

Print the current time in Coordinated Universal Time (UTC), in a form that
other programs can read unambiguously:

```sh
date -u '+%Y-%m-%dT%H:%M:%SZ'
```

Printing a time doesn't change the clock. Setting the clock requires the
relevant authority, and can affect scheduled jobs. To manage the clock and
time zone, see [Keeping the Time](~peios/time/overview).

## Identify the system

`uname -s` prints the kernel's name and `uname -o` the operating system's.
`arch` prints the machine architecture. These values don't tell you which
version of each installed component you have, or whether a particular program
will run: use the package information and each program's own checks for that.

`hostname` prints the machine's host name. Another machine can't necessarily
find this one by that name; that depends on the network and its name service.
Make a lasting change through the [network
configuration](~peios/network-configuration/configuring-profiles), because a
configuration service can restore its own value over a name you set directly.

`hostid` prints a short identifier kept for compatibility. It can repeat
across machines, so never use it to identify a machine securely.

## Check capacity and load

`nproc` prints the number of processors available to the current program,
which can be fewer than the machine has. Memory and storage can limit parallel
work too, so use it as a starting point when choosing how many jobs to run.

`uptime` shows how long the system has been running and its load: a measure of
work that is running or waiting, not a percentage of processor use. For
signed-in sessions, use [Inspecting and Ending
Sessions](~peios/local-accounts/logonse) instead.

## Check the terminal

`tty` prints the terminal connected to standard input, and fails if input
comes from a pipe or a file. A script can use that status to tell whether a
person is typing. `stty -a` shows the terminal's settings, such as whether
typed characters are echoed. If a script changes those settings, save them
first and restore them even when the script fails.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `date` | Print or set the time | `date(1)` |
| `uname` | Print system identifiers | `uname(1)` |
| `arch` | Print the machine architecture | `arch(1)` |
| `hostname` | Print the host name | `hostname(1)` |
| `hostid` | Print the host identifier | `hostid(1)` |
| `nproc` | Print the processors available | `nproc(1)` |
| `uptime` | Print how long the system has run, and its load | `uptime(1)` |
| `tty` | Print the terminal connected to input | `tty(1)` |
| `stty` | Read or change terminal settings | `stty(1)` |
