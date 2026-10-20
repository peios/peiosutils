---
title: Assembling a Unified Kernel Image
type: concept
description: >-
  Understand how the kernel, initramfs, and command line become one firmware
  executable.
related:
  - peios/boot-images/mkirf
---

`mkuki` assembles a unified kernel image (UKI). It appends a kernel, an
initramfs, and a command line to a firmware executable stub so that the boot
payload travels as one file.

## Three payloads and a stub

The command line, kernel, and initramfs occupy separate named sections in the
executable. The stub reads those sections at boot. The default stub is bundled
into the utility; its provenance can be inspected separately.

Command-line input can come from a literal value or a file. Trailing line
endings are removed and a terminator is added. Embedded zero bytes are
rejected rather than silently shortening the command line.

## Select a concrete kernel or a release directory

A concrete image path selects exactly that file. Directory selection looks for
a unique release-named kernel image. Zero matches indicate missing input, and
multiple matches are ambiguous; neither case selects an arbitrary candidate.

Directory selection suits package replacement because the watcher can resolve
the release again after a directory changes. A transient ambiguous state makes
that rebuild fail while leaving the watcher available to retry.

## Publication and signing are separate

The assembled image is written through an exclusively created sibling
temporary file and then replaces the output. A failed staging write does not
replace the accepted output with a partial assembly.

The command does not sign the image. A complete firmware executable is not
proof that its payloads are trusted or bootable. Signing, trust policy, and
boot qualification are separate steps.

## Watch the inputs without watching the output

Concrete input files are observed through their parent directories so that
atomic file replacement is visible. A kernel release directory is watched
recursively. Output paths that feed back into those watched locations are
rejected.

After changes settle, the watcher rebuilds. Failure is logged rather than
ending the loop, so the last output can remain older than the inputs. Monitor
diagnostics instead of treating the existence of an output file as proof of a
successful latest build.

`mkuki(1)` defines section construction, kernel selection, stub inspection,
watching, and output failures.
