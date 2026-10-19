---
title: Process Groups and Account Memberships
type: concept
description: >-
  Explain why process group output can differ from a named-account query.
related:
  - peios/system-and-processes/id
  - peios/system-and-processes/token
---

`groups` displays group memberships that have numeric representations. The
process view and the account-record view answer different questions.

## Runtime membership includes logon decisions

With no account argument, the command reads the calling process's credentials.
A token can contain memberships supplied when identity is created, including
groups that are not individually stored in the account record.

With an account name, the command asks the identity service for recorded
memberships. It does not construct every token that account could obtain.
Consequently, adding an account argument can change more than the display
order.

## Some token state cannot appear

Logon-type groups and session identifiers can have no numeric group ID. Group
attributes, including deny-only state, also cannot be expressed by a list of
group numbers or names.

Use `token show --all` when diagnosing an access decision that depends on
those properties. A group with no friendly name can appear numerically; check
the status and diagnostics rather than assuming every printed row means all
lookups succeeded.

`groups(1)` defines the account operands and error behavior.
