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

## Read more than the user name

Token state includes security identifiers (SIDs), group attributes,
privileges, integrity, and logon information. A deny-only group can deny
access without granting it, even when its friendly name matches an ordinary
membership elsewhere.

The logon-session identifier and the interactive-environment scope are
separate fields. Changing a scope is not the same operation as moving a token
into a different authenticated logon session.

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
