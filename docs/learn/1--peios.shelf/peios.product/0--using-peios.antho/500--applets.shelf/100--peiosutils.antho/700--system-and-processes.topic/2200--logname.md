---
title: Logon Identity and Effective Identity
type: concept
description: >-
  Distinguish the original logon user from a process's effective user.
related:
  - peios/system-and-processes/whoami
  - peios/system-and-processes/id
---

`logname` names the user associated with the logon identity. That can differ
from the effective identity a process uses after an identity change or
impersonation.

## Ask the identity question you mean

Use `whoami` for the effective account name. Use the session and token
inspection tools when you need the relationship between the authenticated
session and a running process.

A missing or unresolvable logon identity is an error, not a reason to
substitute an arbitrary environment variable. A display name is also not an
authorization decision.

`logname(1)` describes the lookup and error contract.
