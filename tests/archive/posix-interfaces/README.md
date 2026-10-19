# Superseded interface tests

These test sources were inherited by Peiosutils at commit
5413eb76eb26850d7c458d2424e23d505d8043ba. They assume interfaces that Peios had
already removed or deferred: POSIX install attributes, logged-in-user databases,
uptime's utmp-file input, and mkfifo/mknod mode flags.

They are retained as historical design and test material, not compiled into the
maintained integration harness. The active tests check the actual Peios
contracts. Do not enable these tests without an explicit interface decision.
