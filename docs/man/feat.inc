.SH COMMANDS
list and info NAME inspect definitions and state. install NAME runs setup;
enable NAME activates installed setup; add NAME installs and enables.
disable NAME retains setup while turning it off. remove NAME disables and
uninstalls setup; uninstall is an alias. A request for an already settled
state does no work.
.SH STATE
Settled states are not-installed, installed, and enabled. Interrupted phases
are installing, enabling, disabling, and uninstalling. A definition absent
from disk is reported separately; its recorded state can remain in the registry.
Retrying a phase starts its script again. An interrupted enable can be disabled,
and an interrupted install can be removed. Scripts must tolerate retry.
.SH PROGRAMMATIC INTERFACE
list --json returns an array; info NAME --json returns an object. defined
reports definition availability; may_change reports permission to change
state, not whether every script operation will succeed.
.PP
--driven COMMAND NAME emits JSON Lines for a change and gives scripts empty
standard input. It refuses list and info; use their JSON forms. Disconnecting
the client does not cancel the change. Inspect the eventual state.
.SH EXIT STATUS
0: success or no change needed. 1: usage error. 2: missing feature.
3: permission refusal. 4: invalid lifecycle transition. 5: script, state I/O,
or other operational failure. Scripts can leave partial effects and an
interrupted state; a failure status does not imply rollback.
