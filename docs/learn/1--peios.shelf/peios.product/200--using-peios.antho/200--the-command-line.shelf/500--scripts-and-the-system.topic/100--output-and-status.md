---
title: Printing Output and Reading Command Status
type: how-to
description: Print messages and formatted values safely, run a command with a changed environment, evaluate conditions and small expressions, and read exit statuses correctly in scripts.
related:
  - peios/scripts-and-the-system/running-and-controlling-commands
  - peios/command-line-basics/text-and-archives
---

A command reports results in two ways: the data it writes to its output, and
an exit status, a number where zero means success. Scripts use output for data
and status for decisions. These commands are the building blocks for both. The
man page for each command lists every option.

Your shell has its own versions of some of these commands, such as `echo`,
`printf`, `test`, and `kill`. To run the utility itself, put `peiosutils`
before its name, as some examples here do.

The examples use sample file names such as `report.txt`. Create those files
first, or substitute your own.

## Print messages and values

`echo` prints its arguments, which suits a short fixed message:

```sh
peiosutils echo 'Build complete'
```

For any value that comes from elsewhere, use `printf` with a fixed format, so
that the value can't be read as an option or as formatting:

```sh
peiosutils printf '%s: %d\n' Aster 3
```

Keep the format under your control and pass data as the later arguments. The
format repeats if more arguments remain.

`seq 2 2 8` prints numbers from 2 to 8 in steps of 2, and `yes sample` prints
a line repeatedly until it is stopped. Don't pipe `yes` into a program to
answer prompts you haven't read: use the program's own option for running
without prompts. `numfmt --to=iec-i` shortens a byte count with a binary
prefix, which scales by powers of 1024. The result is rounded, so keep the
exact number for any calculation.

## Run a command with a changed environment

`env` runs one command with an extra or changed environment variable, without
changing your shell:

```sh
env PEIOS_EXAMPLE=ready peiosutils printenv PEIOS_EXAMPLE
```

`printenv` prints one variable when you name it, and fails if the variable
doesn't exist, which tells an empty value apart from a missing one. Without a
command, `env` prints the whole environment, which can contain credentials, so
don't share that output. A cleared environment helps make a command
repeatable, but it isn't a security boundary.

## Test conditions

`test` reports a condition through its exit status and prints nothing:

```sh
if peiosutils test -f report.txt; then
  peiosutils printf '%s\n' 'Report is a regular file'
fi
```

Quote variables so that an empty value stays one argument. A file test
describes one moment: the file can change before you use it. `true` and
`false` exist only for their status. Don't add `|| true` to hide a failure
unless you've handled the failure you expect.

`expr 7 + 5` evaluates a small integer expression; quote operators such as `*`
that the shell would interpret. `factor 84` prints the prime factors of a
number.

## Read exit statuses

A false condition isn't always an error. `expr`, for example, uses one status
for a zero or empty result, another for a true result, and a third for an
invalid expression. In a pipeline, each command has its own status, and the
shell decides which one the pipeline reports. Read a command's man page before
treating every nonzero status the same way.

## Save a stream while showing it

`tee` writes its input both to standard output and to a file:

```sh
printf 'Starting\nFinished\n' | tee progress.log
```

`tee` replaces the file's contents unless you use append mode. A new file
inherits security from its directory, and an existing file keeps its own, so
choose the directory with care when the stream holds private data.

## Commands on this page

This page covers these commands:

| Command | Use it to | Manual |
|---|---|---|
| `echo` | Print a short message | `echo(1)` |
| `printf` | Print values with a fixed format | `printf(1)` |
| `seq` | Print a sequence of numbers | `seq(1)` |
| `yes` | Print a line repeatedly | `yes(1)` |
| `numfmt` | Format numbers for people to read | `numfmt(1)` |
| `env` | Run a command with a changed environment | `env(1)` |
| `printenv` | Print an environment variable | `printenv(1)` |
| `test` | Check a condition | `test(1)` |
| `true`, `false` | Succeed or fail | `true(1)`, `false(1)` |
| `expr` | Evaluate a small expression | `expr(1)` |
| `factor` | Print prime factors | `factor(1)` |
| `tee` | Copy a stream to a file and to output | `tee(1)` |
