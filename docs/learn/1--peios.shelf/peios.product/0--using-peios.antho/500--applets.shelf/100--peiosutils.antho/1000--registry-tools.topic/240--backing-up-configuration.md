---
title: Backing Up Registry Configuration
type: how-to
description: Choose a reviewable export or a privileged subtree backup and verify replacement recovery.
---

Choose the format according to the recovery requirement. JSON exchange and
opaque registry backups preserve different information.

| Need | Mechanism | Limit |
|---|---|---|
| Review visible configuration | `reg export --json KEY FILE`, then `reg apply FILE` | Does not preserve key descriptors or the complete layered store. |
| Restore a subtree with its descriptors and tagged entries | `reg backup KEY FILE`, then `reg restore KEY FILE` | Privileged replacement, not merge. |
| Withdraw a managed bundle | Its role or policy workflow | Layer removal cannot undo permissions or consumer side effects. |

Registry Editor exposes the corresponding **Export…**, **Import…**,
**Back up…**, and **Restore…** actions. Text export cannot be reapplied as a
text batch.

## Capture a recovery copy

1. Identify the subtree and the permissions, layers, and consumers that
   recovery must preserve. Choose protected storage: backup privilege can
   expose data unavailable through ordinary reads.
2. Capture the subtree, for example:

   ```sh
   reg backup Machine/System/KMES kmes.snap
   ```

3. Confirm success and retain the file with its original path and purpose.
   Interrupted output is not a verified recovery copy.
4. Check whether required layer definitions are also backed up. A stream's
   layer manifest records metadata for validation but does not recreate
   definitions. Their keys under `Machine\System\Registry\Layers` must be
   included or recovered separately.

Backup uses a consistent snapshot while other writes continue. It includes
key descriptors, layer-tagged entries, and absence markers. Integrity checks
detect corruption; they do not make an untrusted backup safe to restore.

## Restore and verify

1. Confirm the trusted backup matches the intended target. Save current
   state if recovery from the restore itself is required.
2. Review replacement scope, restored permissions, needed layer definitions,
   and consumer application timing. The backup root's immutable volatile
   and link flags must match the target.
3. With replacement approved, restore the earlier example:

   ```sh
   reg restore Machine/System/KMES kmes.snap
   ```

4. Reopen paths and inspect representative values, descriptors, and
   `reg layer ls -l`. Then verify consumers and their events. Existing
   handles retain their granted rights, so use fresh opens to test access.

Restore keeps the target key object, replaces its mutable fields and
contents, and replaces descendants. It uses one transaction. Rejection before
commit leaves no partial restored subtree; a timeout after dispatch can
leave an uncertain outcome. Inspect before retrying. Terminal input prompts
for confirmation; redirected input proceeds without that prompt.

Entries whose layer definitions are missing remain inactive until metadata
exists. Restored entries also receive fresh ordering; recovery is not a way
to restore the global write counter.

## Required authority

Backup and restore use `SeBackupPrivilege` and `SeRestorePrivilege`, bypass
ordinary per-key checks, and are audited independently of key audit policy.
Restore can rewrite owners and permissions, including the target key's own
descriptor. Positive-precedence layers in the manifest or existing metadata
also require `SeTcbPrivilege`; restoring their metadata has its own gate.

Read the kernel's [backup and restore
reference](~kernel/internals/lcs/backup-and-restore/overview) before building
a recovery tool. A global-hive backup does not include an unrelated private
hive shadowing the same path for another credential scope.
