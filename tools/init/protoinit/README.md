# protoinit

Transitional PID 1 stub for Phase 0 boots. Mounts `/proc`, `/sys`, `/dev`, then
execs `/bin/sh`. Will be replaced by real `peinit` when Phase 2 lands.

## Usage

Lives at `/init` inside the initramfs. Not run directly.
