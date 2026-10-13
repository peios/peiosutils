# Token inspection design

The shipped command is defined in `src/uu/token/src/cli.rs`, dispatched in `cmd/mod.rs`, and uses `target.rs` for all token opens. Public syntax and behavior live in the learn token reference. Existing commands inspect individual query classes or mutate tokens through their corresponding SDK operations; the two commands below are read-only.

## Comparing tokens

`compare LEFT RIGHT` takes PID/self positional operands. Existing target flags select the right-hand operand when RIGHT is omitted. A second target is required, and combining RIGHT with target flags is an error. The left operand is PID/self; this deliberately keeps the task's PID-versus-thread and self-versus-primary forms unambiguous.

`cmd/compare.rs` takes QUERY handles through `TargetSpec`, then reuses the show SID query helper, token payload parser, privilege snapshot and SID renderer. User, integrity and LogonSession form the header; group SID and privilege-bit unions ensure absent entries remain visible. Group comparison is of enabled/disabled/deny-only state, and privilege comparison is of enabled/disabled state. Token IDs, GUIDs, modification counters and other fields are outside this diagnostic comparison. Friendly labels never determine identity equality. Unknown privilege bits remain visible.

Human output has left/right columns and equality markers. Structured output uses `left`, `right`, `matches` and null for absence. A successful comparison exits zero whether values match or differ; existing usage/query error codes apply otherwise.

## Listing threads

`threads PID` enumerates and sorts numeric `/proc/PID/task` entries, then uses `TargetSpec::Thread` with QUERY. The SDK returns an impersonation token or the process primary token. The token type determines the impersonating flag, so impersonating the same user is still visible.

Enumeration is best effort. A vanished or denied thread keeps a row containing its error and unknown identity fields. Structured output marks the report incomplete and the command returns the first query error after emitting every row. An enumeration failure fails the command. Neither command promises an atomic snapshot across queries.

## Validation

CLI tests cover the task's positional/thread/real examples and ambiguous or missing operands. Pure comparison tests cover deny-only groups, disabled versus absent entries and unnamed privilege bits. Thread tests cover numeric ordering, enumeration failure and partial reports without guessing impersonation state.

`src/uu/token/tests/native_inspection.rs` exercises the actual command binaries on a disposable Peios kernel with its initial SYSTEM token. It holds a real impersonating thread with a deny-only administrator group, one removed privilege and one disabled privilege; checks sorted primary/impersonating thread rows, structured comparisons and human output; and runs the two-PID and self-versus-primary forms. It also checks a missing process error. This test is explicitly ignored on ordinary host runs; run it in the guest with `TOKEN_TEST_BINARY=/path/to/token cargo test -p pu_token --test native_inspection -- --ignored`. A prebuilt test binary accepts the same environment variable and `--ignored --test-threads=1` arguments.
