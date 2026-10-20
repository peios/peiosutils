---
title: Choosing a Message Language
type: how-to
description: Read utility help and diagnostics in an installed language.
---

Peiosutils can display help and diagnostics in different languages. You can
choose a language for one command without changing the environment of your
shell or other applications.

## Before you start

- Install the matching `peiosutils-common` package with the utilities. It
  supplies translated messages, including the shared usage and error text.
- Use a command with translations for your chosen language. Coverage varies
  between commands; missing messages fall back to English.

## Choose a language

1. Set `LANG` for the command you want to run. For example, read the help for
   `sort` in French:

   ```sh
   env LANG=fr_FR.UTF-8 sort --help
   ```

2. To read the same help in English, select the English locale:

   ```sh
   env LANG=en_US.UTF-8 sort --help
   ```

These assignments apply to the command launched by `env`. Your shell keeps
its existing environment. See `env(1)` for other ways to control a command's
environment.

## Check the result

The French help includes `Utilisation:`; the English help includes `Usage:`.
Both should show readable usage text rather than an internal message name
such as `common-usage`.

If the command remains in English, check that its translation is installed.
A missing or unsupported language falls back to English. The shared message
loader reads `LANG`; other locale-sensitive operations can use additional
locale settings. Changing the language of help does not establish the
sorting or numeric-formatting rules for every command.
