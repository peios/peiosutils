---
title: nohup
type: reference
description: Run a command so it keeps running after the terminal closes — and how the nohup.out it creates is secured.
related:
  - peios/security-descriptors/overview
  - peios/files-and-directories/mkdir
  - peios/system-and-processes/overview
---

`nohup` runs a command so that it **survives the terminal closing**. Normally, when a terminal session ends, the system sends a hangup signal to the commands running under it, and they stop. A command started with `nohup` ignores that signal and keeps running.

```
nohup [options] command [arg]...
```

```
$ nohup long-build &
```

It is how you start something that needs to outlast your session — a long build, a background job — without it being killed the moment you disconnect.

## Where the output goes

A command run with `nohup` is meant to outlive the terminal, so `nohup` cannot leave its streams attached to one. It redirects any stream that is still connected to a terminal:

- **Standard input**, if it is a terminal, is replaced with an empty source — the command reads nothing.
- **Standard output**, if it is a terminal, is appended to a file named `nohup.out` in the current directory. If that file cannot be opened there, `nohup` falls back to `nohup.out` in your home directory.
- **Standard error**, if it is a terminal, is sent to wherever standard output now goes.

Streams that you have already redirected yourself — to a file, or a pipe — are left as they are.

## How nohup.out is secured

When `nohup` has to **create** `nohup.out`, that file is a new file and needs a [security descriptor](~peios/security-descriptors/overview). `nohup` does not let it inherit one from the directory. It creates `nohup.out` with a deliberate, locked descriptor: **full access for the owner, and no one else**.

The reasoning is that `nohup.out` captures whatever the command writes, which the person running it has not chosen to share. A file that appears as a side effect should not be more open than its creator intended — so `nohup` gives it the safe default of owner-only, rather than whatever the surrounding directory would have handed down.

The POSIX mode bits agree with the descriptor: the file is created with mode `0600` in the same `open` call that creates it, so there is no moment at which it exists more openly.

| Option | Effect |
|---|---|
| `--sddl=SDDL` | Create `nohup.out` with the security descriptor given as an SDDL string, instead of the owner-only default. |

`--sddl` only matters when `nohup` actually creates the file. If `nohup.out` already exists, `nohup` appends to it and leaves its existing security alone.

## Exit status

When `nohup` runs a command, it exits with **that command's** status once it finishes. The exception is a failure in `nohup` itself:

| Code | Meaning |
|---|---|
| (command's own) | The command ran; this is its exit status. |
| `125` | `nohup` could not set things up — it could not detach from the terminal, or could not open `nohup.out` anywhere. The command was not run. |
