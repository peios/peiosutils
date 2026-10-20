---
title: Managing Features
type: how-to
description: Run feature setup and teardown deliberately, and recover an interrupted phase while its scripts are available.
---

A package supplies a feature's definition and scripts. `feat` runs its
explicit
setup and records lifecycle state; installing the package alone does not
enable
the feature. Read the feature description before changing it, including when
its effects take place.

Use `feat list` and `feat info NAME` to inspect definition availability and
state. Install performs setup; enable activates installed setup; add performs
both. Disable retains setup, while remove disables as needed and then runs
teardown. Removing feature setup does not uninstall its supplying package.

Scripts run with the caller's authority. Permission to write the feature's
registry state does not guarantee permission for every script operation.
An enabled record also does not prove that a service started or a replacement
boot artifact was built and activated.

## Recover an interrupted step

The state records a phase as begun before its script runs, then records the
settled state on success. Failure can therefore leave partial external effects
and an interrupted phase. It is not an automatic package-style rollback.

Read the script's error and inspect actual state before retrying. Scripts are
required to tolerate restarting their phase, but that contract is not proof
that an arbitrary third-party script is correct. After addressing the cause,
retry the phase or use the supported opposite transition. Verify both recorded
state and the resources the script manages; editing the numeric state alone
does not finish the work.

## Keep teardown available

Remove setup before removing its package. Otherwise the registry can retain a
feature whose definition and scripts are gone. Restore the supplying package,
review the restored definition, then run teardown while it is available.
Package undo and recovery do not reverse a feature's registry or runtime work.

Machine clients should use the documented JSON query or driven event stream,
not parse human output. Driven changes provide scripts with empty standard
input and do not become cancellation-safe merely because the client goes away.
Inspect the eventual state after a disconnected request. `feat(1)` owns state
values, command options, stream outcomes, and exit statuses.
