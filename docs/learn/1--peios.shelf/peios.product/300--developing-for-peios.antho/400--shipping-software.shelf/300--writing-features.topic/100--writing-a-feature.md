---
title: Writing a Feature
type: how-to
description: Implement repeatable lifecycle scripts and metadata for a feat-managed feature.
---

A feature gives `feat` a repeatable way to configure installed software.
Its package supplies scripts and metadata; the operator decides when to
install, turn on, turn off, or uninstall that configuration.

## Before you start

- Install `feat` and the tools your scripts call.
- Identify the state each phase changes and how to recover from a partial run.
- Arrange a disposable Peios test environment with the required permissions.

## Define the feature

1. Choose a name containing only ASCII letters, digits, dots, underscores,
   and hyphens. It must be one path component and cannot be `.` or `..`.
2. Stage a directory at `usr/libexec/features/<name>` in the package. The
   runtime view is `/libexec/features/<name>`.
3. Add the applicable scripts and make them executable:

   | Script | Purpose |
   |---|---|
   | `install.sh` | Prepare the feature's persistent configuration |
   | `enable.sh` | Make the prepared feature active |
   | `disable.sh` | Stop using the feature while retaining prepared state |
   | `uninstall.sh` | Remove the feature's configuration |

A missing phase script is a successful no-op. Do not omit a script if that
would leave state that the reverse phase needs to remove. Each script needs
a shebang naming an interpreter available in its execution environment.

## Make phases repeatable

`feat` executes each script directly, with its working directory set to the
feature directory. `FEAT_NAME`, `FEAT_DIR`, and `FEAT_PHASE` identify the
invocation. Scripts run with the caller's authority; `feat` does not grant
them extra privileges.

A failed phase can leave some of its changes applied. A retry starts the
script again, and `feat` does not roll back those earlier changes. Check the
existing state before each operation and tolerate already completed work.
Reverse phases also need to handle partially completed forward phases.

Keep output brief and identify the operation that is progressing. Write
errors to standard error and return a nonzero status on failure. A failed or
interrupted phase is not a completed transition.

In driven operation, standard input is closed and output is forwarded from
both streams. Do not require interactive prompts. In ordinary operation,
scripts inherit the terminal streams, but the same noninteractive contract
keeps the feature usable through graphical clients.

## Add descriptive metadata

Create an optional `feature.toml` beside the scripts:

```toml
title = "Example Service"
description = """
Prepares the Example service and makes it available at boot.
Turning the feature off retains its configuration.
"""
```

The title falls back to the directory name. The description can contain
paragraphs. Unknown keys do not run code or add lifecycle behavior. Invalid
metadata produces a warning without making the feature disappear from the
list, so check its presentation before shipping.

## Package and check

1. Include the directory, metadata, and executable scripts in the package.
   A `feat-<name>` package name is a convention, not the runtime lookup key.
2. Declare dependencies on `feat`, the script interpreters, and invoked tools.
3. In the disposable environment, exercise the lifecycle and repeat each
   applicable phase. Inject failures between operations and verify recovery.
4. Check that turning the feature off and uninstalling it also work after
   a partial attempt.

Installing the package makes the feature available; it does not activate the
feature. For operator commands and state handling, see
[Managing Features](~peios/features/managing-features).
