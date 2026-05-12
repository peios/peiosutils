# show-sd

Print the KACS security descriptor (owner, group, DACL, label/SACL) of one or
more paths. Calls `kacs_get_sd` and decodes the returned self-relative SD.

## Usage

```
show-sd [--raw] [--sacl|--no-label] <path> [path...]
```

- `--raw`: also dump raw SD bytes as hex.
- `--sacl`: request the SACL instead of the integrity label.
- `--no-label`: omit the integrity-label part of the request.
