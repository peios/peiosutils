---
title: Building an Initial RAM Filesystem
type: concept
description: >-
  Understand how a source tree becomes a deterministic early-boot archive.
---

`mkirf` builds an initial RAM filesystem, or initramfs, from a source
directory. The kernel unpacks that archive to provide files needed before the
final root filesystem is ready.

## Source files and generated order

Source-tree contents map onto the archive's root. The tree needs an executable
`init` and supported entry types. Sockets and preexisting generated
hook-sequence files are rejected.

Packaged hooks and operator-provided hooks can declare dependencies. Operator
hooks take precedence when filenames match. The builder resolves those
declarations into a valid order and emits versioned sequence files under
`/system/prelude` without writing them into the source tree.

A missing required capability or a cycle prevents a valid order. A hook
without metadata is handled separately, with a warning and a defined
late-order position. Do not mistake that compatibility path for dependency
validation.

## Determinism is an archive property

The builder sorts entries and normalizes metadata that would otherwise vary
between runs. Regular-file executability remains significant, while ownership,
timestamps, and archive inode numbers are normalized.

Compression settings also form part of the result. The default uses zstd; gzip
is available for a kernel without support for the default format. Compare
builds made with the same inputs, settings, and implementation. A
deterministic archive does not itself prove a bootable or trusted image.

## Early segments precede compression

The reserved `++` directory groups content consumed before the main archive is
decompressed, such as microcode data. Its child directory names label
segments; they do not appear as those labels in the archive root.

The segments are combined into an uncompressed early archive. Conflicting
entries are rejected rather than silently choosing a copy.

## Watch mode is a build loop

Watch mode builds once, then waits for source changes to settle before
rebuilding. A failed rebuild is reported while the watcher remains available
for a later correction. Keep the output outside the watched tree to avoid
triggering a rebuild with its own output.

Successful writing replaces the output through a sibling temporary file. That
protects against presenting a partial assembled archive; it is not a boot test
or a full power-loss durability guarantee.

`mkirf(1)` defines source layout, hook rules, metadata normalization,
compression, exclusions, and watch behavior. Validate the resulting image in
the boot environment before selecting it for deployment.

## Hook discovery and validation

The builder scans regular files directly inside `usr/libexec/prelude/hooks.d`
and `lcl/libexec/prelude/hooks.d` in the source tree. It does not recurse into
subdirectories or select a particular extension. A filename identifies one
hook; an operator copy wins over the vendor copy, with a diagnostic naming
the replacement. The legacy root-level `hooks` directory is accepted with
migration warnings.

A hook's metadata opens with exactly `# /// hook` and closes with `# ///`.
Between those lines, each line is a comment. The supported keys are
`provides`, `contributes`, `requires`, and `after`, each holding a list of
double-quoted capability names. Empty lists and trailing commas are accepted.
This is a restricted declaration format, not arbitrary TOML.

Unknown keys, malformed lists, duplicate or unclosed blocks, missing required
capabilities, dependency cycles, and mixed provider/contributor definitions
are build errors. A declared empty block differs from having no metadata.
Undeclared hooks are warned about and placed after declaring hooks.

Dependency edges place suppliers before consumers. Filename order breaks
ties between unrelated hooks. The builder materializes implicit
`initramfs-ready` ordering in the generated manifest, retaining an explicit
`requires` instead of weakening it to `after`. Prelude then consumes that
manifest; it does not rediscover or reorder the source directories at boot.

[Writing Boot Hooks](~peios/boot/writing-boot-hooks)
explains capability achievement, decline, deferral, and console reporting.
Image construction checks declarations; boot testing checks whether those
claims match the hook's behavior.
