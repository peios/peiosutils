# Coding and documentation style

Peiosutils follows the Peios Style Code in Panorama's documentation. Rust uses
four spaces and a strong 100-column target. The repository's formatter settings
preserve import order. Documentation follows PSC Core Style and the rules for
its publication format.

## Command compatibility

Preserve established command names, options, output formats and exit statuses.
For example, `du -h` requests human-readable sizes; it is not a help alias.
Commands such as `test` distinguish a false result from an invocation error.
These contracts take precedence over PSC's generic short-option and exit-status
rules. Apply the generic rules to new interfaces where they do not conflict
with an established contract.

Do not change observable behavior as a formatting correction. Propose a
compatibility change separately, document it and add regression coverage.

## Upstream attribution

Peiosutils is maintained independently of uutils. Retain copyright notices,
licenses and attribution when changing inherited code. The imported upstream
Git history is preserved. Peios-owned work follows PSC commit style.
