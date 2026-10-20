---
title: Effective Account Names
type: concept
description: >-
  Understand which identity whoami names and what a name cannot prove.
related:
  - peios/system-and-processes/id
  - peios/system-and-processes/logname
---

`whoami` prints the account name associated with the effective user identity.
It is a concise answer to which account the command is running as, not a
complete description of its authority.

## Resolution can fail independently of identity

A token's identity can exist even when the identity service cannot resolve a
friendly account name. Treat a lookup failure as a lookup failure rather than
proof that no identity is present.

The name does not show integrity, privileges, deny-only groups, or other token
restrictions. Use `id` for the compatibility summary and `token` for a fuller
runtime view.

`whoami(1)` describes the output and failure status.
