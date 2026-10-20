---
title: Changing Your Password
type: how-to
description: Change your own account password through the terminal.
---

Use `passwd` to change the password of the account represented by your
current process token. The account authority decides which credentials to
request and whether the new password meets its policy.

## Before you start

- Use a session for the account whose password you want to change.
- Make sure authd and that account's authority are available.
- Keep any credentials needed to verify your identity ready.

## Change the password

1. Start the conversation:

   ```sh
   passwd
   ```

2. Answer the prompts in the order shown. Terminal password input is not
   echoed. The authority can request several rounds of credentials.

3. Wait for the result before retrying. A refused change reports that the
   password is unchanged; a lost connection after submission can leave the
   outcome unknown.

The command accepts no account name. To administer another local account,
use `lps password`; see `lps(1)` for that command's requirements.

The local principal source requires an accepted current password for this
self-service change. An ordinary account without one needs an administrator
to set its password and choose the appropriate credential policy. A service
principal does not become a personal account through this operation.

A password change does not mint a replacement token or end existing sessions.
On the desktop, [My Settings](~peios/security-tools/my-settings) provides a
client for the same credential-change conversation. For the API, use
[Self-Service Operations](~peios/authentication-client/self-service).

## Check the result

A completed change prints `password changed`. If the outcome is unknown,
verify the account's state before attempting another change. The client
does not automatically replay a submitted request.

## Providing input through a pipe

When standard input is not a terminal, `passwd` reads one line per prompt
and does not print prompts for those lines. This requires knowing the
account authority's conversation; a different policy can request different
credentials or more rounds. Avoid putting passwords in shell history,
command arguments, or ordinary temporary files. See `passwd(1)` for the
command interface.
