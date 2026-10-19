---
title: System and processes
type: reference
description: The commands that report on the system, run other commands in a managed way, signal processes, and work with time, terminals, and storage.
related:
  - peios/system-and-processes/uname
  - peios/system-and-processes/kill
---

This topic gathers the commands that deal with the **running system** rather than with files: reading the time, asking what hardware and system you are on, launching another command under some constraint, signalling a process, working with the terminal.

## The commands

**Time**

| Command | Purpose |
|---|---|
| [`date`](~peios/system-and-processes/date) | Print — or set — the system date and time. |
| [`sleep`](~peios/system-and-processes/sleep) | Pause for a given length of time. |
| [`uptime`](~peios/system-and-processes/uptime) | Show how long the system has been running. |

**Running a command under a constraint**

| Command | Purpose |
|---|---|
| [`timeout`](~peios/system-and-processes/timeout) | Run a command, and kill it if it runs too long. |
| [`nohup`](~peios/system-and-processes/nohup) | Run a command so it survives the terminal closing. |
| [`nice`](~peios/system-and-processes/nice) | Run a command at an adjusted scheduling priority. |
| [`stdbuf`](~peios/system-and-processes/stdbuf) | Run a command with altered stream buffering. |
| [`chroot`](~peios/system-and-processes/chroot) | Run a command with a different directory as its root. |

**Signalling processes**

| Command | Purpose |
|---|---|
| [`kill`](~peios/system-and-processes/kill) | Send a signal to a process. |

**System identity and capacity**

| Command | Purpose |
|---|---|
| [`uname`](~peios/system-and-processes/uname) | Print system information — the OS, the kernel, the machine. |
| [`arch`](~peios/system-and-processes/arch) | Print the machine's hardware architecture. |
| [`hostname`](~peios/system-and-processes/hostname) | Display — or set — the system's host name. |
| [`hostid`](~peios/system-and-processes/hostid) | Print the host's numeric identifier. |
| [`nproc`](~peios/system-and-processes/nproc) | Print how many processor cores are available. |

**Who you are**

| Command | Purpose |
|---|---|
| [`whoami`](~peios/system-and-processes/whoami) | Print the name of the user a process is running as. |
| [`id`](~peios/system-and-processes/id) | Print the user and group identity of a process, or of a named user. |
| [`groups`](~peios/system-and-processes/groups) | Print the groups a user belongs to. |
| [`logname`](~peios/system-and-processes/logname) | Print the login name of the user a process is running as. |

These report the **projection** of a token onto POSIX user and group numbers, which is what Linux programs see. The numbers grant nothing on their own — [`id`](~peios/system-and-processes/id) explains what they can and cannot say, and [`token`](~peios/system-and-processes/token) shows the identity underneath them.

**Tokens and logon sessions**

| Command | Purpose |
|---|---|
| [`token`](~peios/system-and-processes/token) | Inspect and manage a process's token. |
| [`logonse`](~peios/system-and-processes/logonse) | Inspect and manage logon sessions and their processes. |

**Storage and terminal**

| Command | Purpose |
|---|---|
| [`sync`](~peios/system-and-processes/sync) | Flush cached writes out to persistent storage. |
| [`tty`](~peios/system-and-processes/tty) | Print the name of the terminal on standard input. |
| [`stty`](~peios/system-and-processes/stty) | Display or change terminal settings. |

## A note on changing the system

Most commands here only *report*. A few can *change* the running system — `date` can set the clock, `hostname` can set the host name. Changing system-wide state is a privileged operation: it succeeds only for a caller whose token holds the right to do it, and is refused otherwise. The pages for those commands say so where it applies, and [Privileges](~peios/privileges/overview) is the full picture.

## Where to start

[`uname`](~peios/system-and-processes/uname) answers "what am I running on?". [`kill`](~peios/system-and-processes/kill) is the one to read carefully — sending the wrong signal to the wrong process is a quick way to lose work.
