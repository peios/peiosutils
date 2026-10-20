---
title: Inspecting and Ending Sessions
type: how-to
description: Inspect visible logon sessions and request that authd end one.
---

Use `logonse` to inspect logon sessions and the processes associated with
them. Ending a session asks authd to sign it out; destroying an empty kernel
session is a different administrative operation.

## Before you start

- Save work in any session you intend to end.
- Use an account allowed to inspect the relevant processes. A complete
  kernel session listing needs access to the kernel's session list.
- To end a session, run authd and use an account allowed by its session-end
  policy. You can request termination of your own eligible sessions.

## Inspect a session

1. List sessions visible to you:

   ```sh
   logonse list
   ```

2. Note the numeric session ID, then inspect it. Replace `1007` with the ID
   from your listing:

   ```sh
   logonse show 1007
   ```

The process listing contains only processes whose tokens you can inspect.
It is a snapshot gathered while processes can start or exit. If the kernel
session list is inaccessible, the command reports the limitation and uses
visible processes to identify sessions. A session with no visible processes
can still be held alive by a token.

## Request a sign-out

1. Check permission without ending the selected session:

   ```sh
   logonse end 1007 --check
   ```

2. After confirming the session ID and saving its work, request sign-out:

   ```sh
   logonse end 1007
   ```

The check does not reserve permission. Authd checks the actual request
again. Service sessions end when their service stops; use `svctl stop`
for that task. The kernel's own sessions cannot be ended this way.

## Check the result

Read the reported counts and any refusal. A nonzero remaining count means
the session is still open. When ending the caller's own session, the reply
can describe planned termination before the caller exits. If the connection
is lost after submission, verify the session state before retrying.

Run `logonse list` from another session to inspect the resulting state.
Token references can keep a session alive after its visible processes exit.

## Other administrative operations

`logonse create` creates a privileged kernel authentication record; it is
not a way to authenticate a user. `logonse destroy` removes an empty
session with no remaining token references. `logonse psb` inspects or changes
process security settings, which have separate permission requirements.
See `logonse(1)` for these interfaces and the JSON output option.

## Interpret session evidence

A process or thread's token statistics contain `auth_id`, the key used to
join it to a kernel logon-session record. Looking at primary tokens answers
which session the processes run under. Looking at effective thread tokens
answers which session an impersonating request uses. These are different
counts, neither of which enumerates every open token handle.

SYSTEM session 999 and Anonymous session 998 are bootstrap records, not
unexpected interactive sign-ins. An authentication record can also exist
before a token has been attached to a process.

The kernel's `kacs.session.destroyed` event records teardown after the last
references are released. Its deferred emission can follow disappearance
from the live list. Collection gaps mean an event stream cannot establish a
complete session history by itself. Authd does not retain a complete queryable
record of the grants and policy used for every past authentication.

Token expiration metadata is not a kernel timer or an AccessCheck revocation
rule. A session-end request coordinates termination of its primary-token
processes; it does not invalidate a token handle held elsewhere. Use the
[kernel lifecycle reference](~kernel/internals/kacs/tokens/logonsessions-and-revocation)
for those limits. For process protection, see
[Inspecting Process Security](~peios/identity-and-access/inspecting-process-security).
