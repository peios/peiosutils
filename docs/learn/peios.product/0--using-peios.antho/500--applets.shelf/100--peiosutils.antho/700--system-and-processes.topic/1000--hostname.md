---
title: Reading the Host Name
type: how-to
description: >-
  Inspect the local host name without assuming that name is resolvable.
related:
  - peios/system-and-processes/uname
  - peios/system-and-processes/overview
---

`hostname` prints the machine's configured host name. A local name and a name
registered in a network naming service are separate facts.

## Before you start

Use a terminal with Peiosutils installed. For examples that create files,
start in a disposable working directory without existing example names.

## Perform the task

Run the command:

```sh
hostname
```

## Check the result

The output is the configured host name in the current system context.

## Separate naming from networking

A displayed name does not prove that another machine can resolve it or connect
to this host. Network configuration and name-service registration must agree
for that to work.

Changing the name requires the relevant authority and can be temporary if a
configuration service restores its own value. Use the maintained network
configuration workflow for persistent changes. See `hostname(1)` for the
command's display and setting forms.
