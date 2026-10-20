---
title: Checking Who a Command Runs As
type: how-to
description: Read the account and groups a command runs with using whoami, id, groups, and logname, and know when to use token for the full picture.
related:
  - peios/identity-and-access/access-tokens
  - peios/identity-and-access/token
  - peios/identity-and-access/security-identifiers
---

`whoami`, `id`, `groups`, and `logname` give quick answers about the identity
a command runs with. They show the account name and group memberships, but not
everything that decides what the command can do. For that, read the command's
[access token](~peios/identity-and-access/access-tokens) with `token`.

## Read the account name

`whoami` prints the name of the account the command runs as. If the identity
service can't find a name for the account, `whoami` fails, but the command
still has an identity: a failed lookup is only a failed lookup.

`logname` prints the account that signed in to the session. It can differ from
`whoami` when a program has taken on another identity.

## Read identity numbers and groups

`id` shows the account and its groups as numbers, with their names, and adds
the user's security identifier (SID) and integrity level. The numbers are kept
for compatibility. Peios decides access with SIDs and the other contents of
the token, and the numbers can't show all of them: some groups have no number,
and a deny-only group looks the same as an ordinary one. Two commands can
print the same numbers and still be allowed different things.

`groups` prints the group memberships that have numbers.

## Ask about the command or about an account

Without an account name, `id` and `groups` describe the command you run them
from, using its token. With an account name, they ask the identity service
about the account record instead. A token includes groups added at sign-in,
such as Everyone, that aren't stored in the account record, so the two answers
can differ without either being wrong.

## See the full token

When an access decision depends on more than names and numbers, read the
token:

```sh
token show --all
```

It shows every group with its attributes, the privileges, and the integrity
level. See [Inspecting Effective Identity](~peios/identity-and-access/token).

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `whoami` | Print the account a command runs as | `whoami(1)` |
| `logname` | Print the account that signed in to the session | `logname(1)` |
| `id` | Print identity numbers, names, SID, and integrity level | `id(1)` |
| `groups` | Print group memberships | `groups(1)` |
| `token` | Show the full access token | `token(1)` |
