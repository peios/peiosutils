# revstrm

KMES event-ring inspector and emitter.

## Subcommands

```
revstrm dump [--cpu N]               # one-shot drain of a per-CPU ring
revstrm follow [--cpu N]             # tail the ring
revstrm emit --type T [opts] PAYLOAD # write a userspace-origin event
```

`emit` wraps the payload as msgpack so the kernel validator accepts it.
Use `--map`, `--array`, or `--raw` to control payload shape.

Event headers and payloads are pretty-printed one field per line. msgpack
maps and arrays expand recursively.
