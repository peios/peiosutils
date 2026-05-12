# whoami-token

Dump the calling process's KACS access token. Opens its own primary token via
`kacs_open_self_token`, then iterates `KACS_IOC_QUERY` across the well-known
token classes and prints decoded contents.

## Usage

```
whoami-token
```

No arguments. Prints user/owner/group SIDs, integrity level, token type,
elevation, session ID, statistics, and the full privilege table.
