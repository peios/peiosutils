---
title: System State and Process Identity
type: concept
description: >-
  Separate system observations, process-control requests, and Peios
  identity.
related:
  - peios/system-and-processes/uname
  - peios/system-and-processes/kill
---

System utilities inspect the running environment or request a change to a
process. Identity tools expose a different part of that environment: the
authority under which an operation runs.

## Observation is a moment in time

Processor counts, load, names, and process numbers can change. A successful
query does not reserve a resource or keep a process number from being reused.
Use a mechanism designed for stable identity when later operations depend on
the same object.

These guides help select the relevant view:

- [System identifiers](~peios/system-and-processes/uname) distinguish kernel and product.
- [Processor availability](~peios/system-and-processes/nproc) informs parallel work.
- [System uptime](~peios/system-and-processes/uptime) provides runtime context.
- [Process signals](~peios/system-and-processes/kill) change process state.
- [Numeric identity](~peios/system-and-processes/id) explains compatibility projections.
- [Token inspection](~peios/system-and-processes/token) exposes runtime authority.
- [Session inspection](~peios/system-and-processes/logonse) concerns authenticated sessions.

## Match the tool to the lifetime

A timeout, a hangup-resistant process, and a supervised service have different
lifecycle guarantees. Use service management when the job needs readiness
checks or restart policy rather than composing those properties from unrelated
command wrappers.

Native security and session operations require the Peios kernel and services.
A development-host build does not supply those runtime facilities.
