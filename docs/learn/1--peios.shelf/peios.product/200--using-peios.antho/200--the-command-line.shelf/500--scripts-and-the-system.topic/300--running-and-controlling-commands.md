---
title: Running and Controlling Commands
type: how-to
description: Pause, time-limit, and keep running commands, adjust their priority and output buffering, send signals, run a command in another root directory, and flush pending writes.
related:
  - peios/services-and-jobs/overview
  - peios/scripts-and-the-system/output-and-status
---

Use these commands to control how and how long another command runs. They suit
short jobs in a terminal or a script. A job that must keep running, restart
when it fails, or start when the machine starts belongs to the service manager
instead; see [Services and Jobs](~peios/services-and-jobs/overview). The man
page for each command lists every option.

## Wait and set time limits

`sleep 0.1` pauses for a tenth of a second. A pause doesn't prove that
anything finished, so wait for an explicit sign that a service is ready rather
than sleeping and hoping.

`timeout` stops waiting for a command after a time limit:

```sh
timeout 1 peiosutils sleep 0.1
```

When the limit expires, `timeout` sends a signal, and the command must still
respond to it. An option can force a stop after a grace period, but a command
stopped part way through can leave partial output behind. See `timeout(1)` for
its exit statuses.

## Keep a command running after you disconnect

`nohup` starts a command that keeps running when its terminal disconnects.
Output that would have gone to the terminal goes to `nohup.out` in the working
directory instead. A new `nohup.out` gets security that admits only you; an
existing one is added to and keeps its own security. `nohup` doesn't put the
command in the background (end the line with `&` for that), and it doesn't
protect the command from failing, other signals, or shutdown.

## Adjust priority and buffering

`nice -n 5` runs a command with a lower scheduling preference, so it yields to
other work. Raising a command's preference needs extra authority. Niceness
doesn't change the command's identity or what it can access.

When a pipeline's output arrives late because a program holds it in a buffer,
`stdbuf -oL` asks the program to write each line as soon as it is complete. It
works only for programs that use the standard C output functions; a program's
own option for flushing output is more reliable.

## Send signals

`peiosutils kill -l` lists the signals. `kill` sends a signal to a process: a
request to change state, which a program can handle. The usual stop signal
lets a program clean up; a forced kill doesn't. Process numbers are reused
after a process ends, so check the target first. Whether you can signal a
process is an access decision, and knowing its number grants nothing.

## Run a command in another root directory

`chroot` runs a command with a different directory as its root, so that
absolute paths are found inside that directory. The directory must contain
everything the command needs, including its libraries. A changed root only
changes where paths are looked up: it doesn't change the command's access
token, privileges, network access, or open files, so it isn't a way to contain
untrusted code. It also requires the relevant authority.

## Flush pending writes

`sync` asks the system to write pending changes to storage. How durable the
data then is still depends on the filesystem and the device. `sync` doesn't
make a half-written file complete or check that its contents are right.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `sleep` | Pause for a time | `sleep(1)` |
| `timeout` | Run a command with a time limit | `timeout(1)` |
| `nohup` | Keep a command running after a disconnect | `nohup(1)` |
| `nice` | Run a command with a different priority | `nice(1)` |
| `stdbuf` | Change a command's output buffering | `stdbuf(1)` |
| `kill` | Send a signal to a process | `kill(1)` |
| `chroot` | Run a command in another root directory | `chroot(1)` |
| `sync` | Write pending changes to storage | `sync(1)` |
