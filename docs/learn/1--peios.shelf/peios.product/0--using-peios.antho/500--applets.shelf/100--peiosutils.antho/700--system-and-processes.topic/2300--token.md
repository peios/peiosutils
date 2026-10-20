---
title: Inspecting Effective Identity
type: concept
description: >-
  Use token views to distinguish runtime authority from account records.
---

`token` exposes the identity and authority carried by running processes and
threads. It is a diagnostic tool for questions that a user name or numeric
group list cannot answer.

## Choose the identity being observed

A process has a primary token. A thread can have an impersonation token that
supplies its effective identity. A peer-token query observes identity captured
on a connected socket, which is another distinct target.

Reading another process's token requires the appropriate access. Knowing a
process number or a socket descriptor does not bypass that check.

Use an explicit selector when diagnosing another program:

```sh
token show --pid PID --all
token groups --pid PID
token privs --pid PID
token stats --pid PID
token show --pid PID --tid TID --all
```

Replace the process and thread IDs with identified targets. The process view
selects its primary token; the thread view matters for impersonation.
`--self` is the default, `--real` selects the caller's primary token, and
`--peer SOCK_FD` selects captured identity on a connected socket. A peer query
is not a lookup of whatever identity its process happens to use afterward.

Preserve the selector, time, token ID, and modification ID with observations.
Use `--json` for structured output and the documented raw-SID/label display
choices when comparing identities. These queries do not authorize mutation.

## Read more than the user name

Token state includes security identifiers (SIDs), group attributes,
privileges, integrity, and logon information. A deny-only group can deny
access without granting it, even when its friendly name matches an ordinary
membership elsewhere.

The logon-session identifier and the interactive-environment scope are
separate fields. Changing a scope is not the same operation as moving a token
into a different authenticated logon session.

Field commands also expose integrity, logon context, claims, capabilities,
origin/source, and creation defaults. Apply the same target selector to each.
A default DACL concerns new objects, not the descriptor of every existing
object. `show --all` means all supported query classes, not every internal
field. Absence of an internal field from the output does not establish its
value; the kernel's query reference owns that boundary.

Full or Limited elevation type can outlive the active link. `token linked`
queries a counterpart where one exists and is accessible; a role alone is
not evidence that elevation is available. The privilege used bit is not a
timestamped record of each exercise.

## Compare observations carefully

Token comparison reports selected identity, group, and privilege state. Equal
and different comparisons can both succeed: the status indicates whether the
queries worked, not whether the tokens matched.

Thread enumeration can race with thread exits or access changes. A failed row
makes the result incomplete and is reported rather than silently removed.
Multiple queries are consecutive observations, not one atomic snapshot of all
token state.

## Derived tokens remain controlled objects

Restriction, duplication, adjustment, and impersonation have different
purposes. A restriction limits an existing authority; it does not grant a new
one. Creating or installing identity requires the corresponding privilege and
is not a normal way to switch users.

`token(1)` defines target selection, comparison output, thread failures, and
the mutation interfaces. Use the normal authentication workflow when your goal
is signing in rather than testing a token mechanism.

## Refused and partial views

A cross-process token query needs process-query rights, PIP dominance, and
query access to the token. Own-process `/proc` query-only token paths have a
special rule; direct self-open APIs still check the token descriptor.
Do not generalize one surface's self-access exception to every operation.

Process walks cannot enumerate all tokens held only by file descriptors.
A missing target, denied query, and invalid request are different failures;
none is a successfully read empty token. Token pseudo-files are handles, not
text files. SDK users should follow the [opening
reference](~peios/sdk-tokens/opening-and-creating-tokens) and
[query protocol](~peios/sdk-tokens/query).
