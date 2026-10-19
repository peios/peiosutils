---
title: logname
type: reference
description: Print the login name of the user a process is running as.
related:
  - peios/system-and-processes/whoami
  - peios/system-and-processes/id
  - peios/logon-sessions/overview
---

`logname` prints the login name the calling process runs under.

```
logname
```

```
$ logname
jack
```

It takes no arguments.

## Login name, not current name

`logname` and [`whoami`](~peios/system-and-processes/whoami) look like the same command and answer slightly different questions.

`whoami` reports the **effective** identity — whoever the process is acting as right now. `logname` reports the **login** identity: the principal the process's own token is for, regardless of any token it has since taken on. When a process is [impersonating](~peios/impersonation/overview) a client, `whoami` follows the impersonation and `logname` does not.

Most of the time nothing is impersonating and the two agree.

If you know `logname` from another Unix, note that it answers this from the token rather than from a login-record file. There is no `utmp` on Peios — nothing writes one — so the traditional source does not exist. The token is the record of who a process is, and it is a better one: it cannot drift from the identity access is actually checked against, because it *is* that identity.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The login name was printed. |
| `1` | No login name could be determined. |
