---
title: Terminal Disconnects and Job Lifetime
type: concept
description: >-
  Understand what ignoring a hangup signal does and does not preserve.
related:
  - peios/files-and-directories/mkdir
  - peios/system-and-processes/overview
---

`nohup` starts a command with the hangup signal ignored. This can keep a job
from stopping solely because its terminal disconnects, but does not make it a
supervised service.

## Streams outlive the terminal differently

Terminal input cannot be relied on after disconnect. Terminal output is
redirected to `nohup.out` in the working directory, or to a fallback in the
home directory when needed. Terminal error output follows the selected output
destination.

Streams already redirected by the caller remain under that caller's choice. A
background operator belongs to the shell; `nohup` itself does not place the
job in the background.

## Created output has deliberate security

A newly created `nohup.out` receives owner-only security unless an explicit
security descriptor is supplied. An existing output file is appended to
without replacing its existing security descriptor.

That distinction matters when another process created the file. Inspect the
destination rather than assuming a familiar filename is private.

## Ignoring one signal is not supervision

Other signals, process failure, shutdown, or session policy can still stop the
command. Use the service manager for a job that needs restart policy,
readiness tracking, or durable lifecycle management.

`nohup(1)` defines stream redirection, creation security, and setup-failure
statuses.
