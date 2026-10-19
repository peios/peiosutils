---
title: logonse
type: reference
description: The logonse command lists logon sessions and their processes, creates and destroys them, signs them out, and shows or sets a process's PSB.
related:
  - peios/logon-sessions/overview
  - peios/logon-sessions/lifecycle
  - peios/security-diagnostics/sessions
  - peios/process-mitigations/overview
---

`logonse` is the command-line tool for **logon sessions** — the kernel's records of authentication events that this topic describes. It lists the active sessions, shows which processes belong to one, creates and destroys sessions, signs them out, and (as a related low-level job) shows a process's Process Security Block (PSB) and sets its mitigation flags.

```
logonse subcommand [arguments]
```

```
$ logonse list
$ logonse show 4711
```

`logonse` is a low-level administrative and debugging tool. It requires a subcommand: `list`, `show`, `create`, `destroy`, `end`, or `psb`.

## Listing sessions

### `logonse list`

Lists every live logon session: who it is for, what kind of sign-in made it, the authentication package, when it was made, and the processes in it.

```
$ logonse list
session 999  Local System (S-1-5-18)  service
  package: Negotiate
  created: when the machine started
  pids:    [562, 571, 620, 667, 670]
session 1007  jack (S-1-5-21-…-1000)  remote-interactive
  package: lpsd
  created: 2026-10-04 10:02:41
  pids:    [2611, 2633, 2650]
```

### `logonse show`

Shows one session, the same way.

```
$ logonse show 1007
```

### Where the answers come from

The sessions come from the kernel's own list, [`/sys/kernel/security/kacs/sessions`](~peios/security-diagnostics/sessions), which only `BUILTIN\Administrators` and SYSTEM may read. It lists every session, including one that has no process — held alive only by a token file descriptor somewhere, or made and not yet used — and such a session shows `pids: none you can see`.

The processes come from walking the running processes and reading each one's token (`/proc/<pid>/token`) to find which session it belongs to. That shows only the processes you may inspect: an administrator sees every process but the protected ones, such as `peinit` and `authd`, which no administrator can inspect. Processes start and exit while the walk runs, so it is a close approximation of the moment, not a locked one.

Without Administrators, the kernel's list is refused, and `logonse` says so and shows only the sessions of processes you can inspect — your own.

## Creating and destroying sessions

### `logonse create`

Creates a new logon session for a user, described by a logon type, an authentication-package name, and the user's SID.

```
$ logonse create --logon-type interactive --auth-package Negotiate --user-sid S-1-5-21-...-1001
```

| Flag | Meaning |
|---|---|
| `--logon-type TYPE` | The kind of logon: `interactive`, `network`, `batch`, `service`, `network-cleartext`, `new-credentials`, or `remote-interactive`. |
| `--auth-package STR` | The name of the authentication package that vouched for the logon. |
| `--user-sid SID` | The user the session belongs to, as an `S-1-…` SID or an SDDL alias such as `BA`. |

On success `logonse` prints the new session's id. Creating a session is a **privileged** operation — minting authentication records is reserved for the components that legitimately do so.

> [!NOTE]
> The tool builds the kernel's binary session spec from these fields for you. The underlying [wire format](~peios/advanced-peios/wire-formats-reference/token-and-session-specs) is unchanged; only the command-line surface is typed.

### `logonse destroy`

Destroys a session — but only an **empty** one, with no tokens still referencing it.

```
$ logonse destroy 4711
```

A session with live tokens cannot be destroyed this way; its tokens must go first. See [Session lifecycle](~peios/logon-sessions/lifecycle).

## Signing a session out

### `logonse end`

Signs a session out: authd ends every process running in it, asking each to end and, after a few seconds, ending any that haven't. Once the last of them has gone, the kernel ends the session.

```
$ logonse end 1012
ended session 1012: 2 processes ended
```

You may end your own sessions, and anyone's that the machine's `SessionEndSecurity` setting allows, which as shipped is Administrators and SYSTEM. A service's session can't be ended this way: it ends when the service is stopped. Nor can the kernel's own sessions, 999 and 998. A refusal says which of these it is. See [Ending a session](~peios/logon-sessions/lifecycle).

| Flag | Meaning |
|---|---|
| `--check` | Only ask whether you may end it, and end nothing. |

If authd couldn't end every process, `logonse end` says how many still hold the session and exits with status `1`.

## A process's PSB

### `logonse psb`

With only `--pid`, `logonse psb` shows a process's **Process Security Block**: whether it is protected by PIP (Process Integrity Protection, which shields signed system processes from everyone else), which mitigations are on, and the process's GUID, which events carry.

```
$ logonse psb --pid 1
psb pid=1
  pip:         protected, Peios TCB
  mitigations: none
  guid:        3f2c9a1e-6b0d-4c8e-9a41-2d7e5f10b6c3
```

Anyone the process's descriptor lets query it may read this, which by default is everyone — even for a protected process whose everything else is closed to you.

With `--mitigations`, it requests those mitigation flags instead of showing the PSB. Read the current state with only `--pid` before considering a change, and use the [application and lifecycle guidance](~peios/process-mitigations/applying-and-lifecycle) to review the required policy and activation limits.

A request is one-way, activation-backed and all-or-nothing. An authorised caller can still be refused because the target's existing state or platform cannot satisfy a protection. The Kernel TRM documents CFIF as unavailable and CFIB activation as self-only, so a mask containing those flags is not a general-purpose way to harden another process. Do not use ALL as a blanket policy. Check the command's result, then inspect the committed state again; a requested mask alone is not proof of protection.

| Flag | Meaning |
|---|---|
| `--pid PID` | The process to act on. |
| `--mitigations MASK` | The mitigation bitmask to request, in hexadecimal or decimal. Without it, the PSB is shown. |

This subcommand is about process hardening rather than logon sessions — it lives in `logonse` because both deal with low-level per-process kernel state. For what the mitigation flags mean and how they behave, see [Process mitigations](~peios/process-mitigations/overview).

## Output options

| Flag | Effect |
|---|---|
| `--json` | Emit JSON instead of human-readable output. Accepted by every subcommand. |

## Exit status

| Code | Meaning |
|---|---|
| `0` | The operation succeeded. |
| `1` | A usage error, or the operation failed. |
