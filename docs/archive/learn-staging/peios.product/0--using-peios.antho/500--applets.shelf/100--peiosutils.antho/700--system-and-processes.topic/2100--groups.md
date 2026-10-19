---
title: groups
type: reference
description: Print the groups a user belongs to.
related:
  - peios/system-and-processes/id
  - peios/authentication/resolving-names
  - peios/system-and-processes/token
---

`groups` prints the groups a user belongs to.

```
groups [USERNAME]...
```

```
$ groups
Authenticated Users Everyone Administrators Local

$ groups jack
jack : Authenticated Users Administrators
```

With no argument it reports the calling process. Given names, it reports on those.

## Those two answers are both correct

The example above is not a mistake, and it is the thing worth knowing about this command on Peios.

**`groups`** reads the calling process's credential — the projection of its [token's](~peios/tokens/overview) group set. That set includes the groups the authority *staples on* when the token is minted: `Everyone` and `Local` are on every token, whoever you are.

**`groups jack`** looks the name up through the [principal store](~peios/authentication/resolving-names), which returns **recorded** memberships — the ones something actually wrote down.

The stapled groups are not recorded anywhere, because membership in them is a **rule**, not a stored fact. Everyone is in `Everyone`; nothing needs to record it, and nothing can enumerate it. So they appear in the first answer and not the second.

On other Unixes these two forms differ only in the order they print.

## What cannot appear

Both forms are lossy in the same way, and unavoidably so — a group can only be listed if it has a group ID, and some deliberately do not:

- `Interactive`, `Remote Interactive`, `Network`, `Batch` and `Service` are **unnumbered on purpose**. They describe *how* you signed in rather than who you are, which is what lets a rule like "network logons cap at Low" be written at all. You are in `Interactive` at the console and not over the network, so there is no static answer to record.
- Your **logon session** SID is minted fresh for each sign-in and has no number either.
- A group's **attributes** are not representable. A deny-only group — one that can block access through a deny entry but grant nothing through an allow entry — is printed exactly like an ordinary membership.

[`token show --all`](~peios/system-and-processes/token) is the lossless view, with every SID and its attributes.

## Exit status

| Code | Meaning |
|---|---|
| `0` | The groups were printed. |
| `1` | A named user could not be found, or the group list could not be read. |

A group ID with no name is printed as the bare number and sets the exit status, rather than stopping the listing.
