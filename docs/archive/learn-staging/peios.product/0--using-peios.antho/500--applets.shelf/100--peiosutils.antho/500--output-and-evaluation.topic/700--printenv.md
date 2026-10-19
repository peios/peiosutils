---
title: printenv
type: reference
description: Print the values of environment variables.
related:
  - peios/output-and-evaluation/env
  - peios/output-and-evaluation/overview
---

`printenv` prints environment variables.

```
printenv [options] [variable...]
```

Name one or more variables and `printenv` prints the value of each. Name none and it prints the whole environment, one `NAME=VALUE` pair per line.

```
$ printenv HOME
/home/jack
$ printenv
PATH=/bin
HOME=/home/jack
...
```

## Options

| Option | Effect |
|---|---|
| `-0`, `--null` | End each output line with a NUL character instead of a newline — so values that themselves contain newlines can still be told apart. |

## `printenv` and `env`

Both can print the environment. `printenv` is the one built for *reading* it — it can pick out a single variable by name. [`env`](~peios/output-and-evaluation/env) prints the environment too, but its real job is *running a command* with a modified one. Use `printenv` to look something up; use `env` to change something for a command.

## Exit status

| Code | Meaning |
|---|---|
| `0` | Every named variable was found and printed. |
| `1` | A named variable was not set. |
| `2` | A usage error. |
