---
title: whoami
type: reference
description: Print the name of the user a process is running as.
related:
  - peios/system-and-processes/id
  - peios/system-and-processes/logname
  - peios/identity/sids
---

`whoami` prints the name of the user the calling process is running as.

```
whoami
```

```
$ whoami
jack
```

It takes no arguments. It is the same answer as `id -un`, which is all it has ever been.

## Where the name comes from

The name is resolved from the process's effective user ID, which is a projection of the [token's](~peios/tokens/overview) user SID. The lookup goes to the authority — there is no `/etc/passwd` to read — and comes back as the principal's canonical name. [Resolving names](~peios/authentication/resolving-names) is the full picture.

The name is the principal's; the *number* it was looked up by is not what grants anything. If you want the identity access is actually decided against, `id -Z` prints the SID.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The name was printed. |
| `1` | The user ID could not be resolved to a name. |

A failure here usually means the authority could not be reached rather than that the account is missing — identity does not resolve before the authority is running, which is the correct answer rather than a gap.
