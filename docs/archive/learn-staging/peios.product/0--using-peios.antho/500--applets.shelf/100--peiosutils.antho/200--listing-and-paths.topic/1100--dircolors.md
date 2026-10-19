---
title: dircolors
type: reference
description: Produce the shell commands that set LS_COLORS — the variable that controls how ls colours its output.
related:
  - peios/listing-and-paths/ls
  - peios/listing-and-paths/overview
---

When [`ls`](~peios/listing-and-paths/ls) colours its output, it reads the colours from an environment variable named `LS_COLORS`. `dircolors` is the command that produces the value of that variable.

```
dircolors [options] [file]
```

`dircolors` does not set the variable itself — a command cannot change its parent shell's environment. Instead it *prints shell commands* that, when run by the shell, set `LS_COLORS`. The usual way to use it is to have the shell evaluate that output:

```
eval "$(dircolors)"
```

Put that line in a shell startup file and every `ls --color` afterwards picks up the colours. With no arguments, `dircolors` uses a built-in default colour scheme.

## Choosing the shell syntax

The commands `dircolors` prints have to match the shell that will run them. By default it guesses the syntax from the `SHELL` environment variable; these options state it outright.

| Option | Output syntax |
|---|---|
| `-b`, `--sh`, `--bourne-shell` | Bourne-shell-family syntax. |
| `-c`, `--csh`, `--c-shell` | C-shell-family syntax. |

If no shell option is given and `SHELL` is not set, `dircolors` cannot guess and reports an error.

## Customising the colours

To change the colours, you start from the default scheme, edit it, and feed it back.

| Option | Effect |
|---|---|
| `-p`, `--print-database` | Print the built-in colour database in its editable source form, instead of shell commands. |
| `--print-ls-colors` | Print the colours fully escaped, one per line, for inspection. |

The workflow is:

```
dircolors -p > ~/.dircolors      # save the default scheme to a file
# ...edit ~/.dircolors to taste...
eval "$(dircolors ~/.dircolors)" # load the edited scheme
```

When `dircolors` is given a `file` argument, it reads the colour definitions from that file instead of using the built-in database. The file maps file types and name extensions to colours; `dircolors -p` shows the format, with each line commented.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | A usage error, or a colour-definition file that could not be read or parsed. |
