---
title: Inspecting Process Security
type: how-to
description: Read a process's protection and hardening without confusing a visible summary with control rights.
---

Use `logonse psb --pid PID` to inspect a process's protection, mitigation
mask, and Process GUID. Replace `PID` with the current process number.
Omitting mutation options leaves those settings unchanged.

## Inspect and correlate

1. Identify the current process and its workload. Record its Process GUID
   when correlating historical events; a PID can be reused.
2. Read its PSB with `logonse psb --pid PID` or Task Manager. Limited query
   access can expose the protection level even when detailed inspection or
   process control is refused.
3. Inspect the primary token to identify the process's account and session.
   Inspect the particular thread's effective token if impersonation matters.
4. Read the process descriptor using the supported SDK surface when the
   question is who can act on it. The DACL and SACL have different access
   requirements; a limited PSB read is not permission to read both.
5. Correlate current state with stored events. Separate reads can observe
   different moments and do not constitute an atomic security snapshot.

The kernel's [process ABI](~kernel/userspace/native/kacs/generated-process-and-psb-abi)
defines rights and mitigation bits. The [PSB
reference](~kernel/internals/kacs/process-security/psb-fields) explains their
meaning. Do not infer an exact grant from a token or protection label alone;
the target object and requested operation also matter.

## Read a refusal accurately

PSB inspection needs limited query rights and does not require PIP dominance.
Detailed inspection and process-control operations have stronger checks.
Impersonation changes identity without changing the caller's PIP, and
`SeDebugPrivilege` does not bypass that protection.

A self-targeted PSB read is supported, but that does not mean every operation
on one's own process succeeds. Reading the SACL still has its own authority
requirement. Missing fields and access errors are not empty permissions.

## Changes over time

PIP follows the executable at exec, so it can change while PID and Process
GUID stay the same. Mitigations only become stricter and persist across exec.
A thread can install or revert impersonation, and token adjustments or
process-descriptor edits can change other inspection results.

Adding `--mitigations MASK` requests a permanent hardening change. It is not
an inspection option or a way to gain control rights. Read `logonse(1)` and
the mitigation reference before using it; failure to activate a requested
protection must not be reported as success.
