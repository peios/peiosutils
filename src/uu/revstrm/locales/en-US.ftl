revstrm-about = Print the raw KMES event stream to the terminal for debugging.
revstrm-usage = revstrm [OPTION]...
revstrm-after-help = Attaches directly to the KMES per-CPU ring buffers and prints every
 event it drains, oldest surviving event first. Requires SeSecurityPrivilege.

 This is a low-level probe for debugging the event pipeline itself; for normal
 event viewing, query eventd. By default revstrm follows the live stream
 (Ctrl-C to stop); timestamps are UTC time-of-day.

 Each line is: TIME  cpuN  #SEQUENCE  ORIGIN  event.type  payload
 where ORIGIN is one of USR, KMES, KACS, LCS.

# option help
revstrm-help-type = only show events whose type matches GLOB (repeatable; matches if any)
revstrm-help-origin = only show events from CLASS: userspace, kmes, kacs, or lcs (repeatable)
revstrm-help-pretty = expand the msgpack payload across multiple indented lines
revstrm-help-snapshot = drain the events currently buffered and exit, instead of following

# errors
revstrm-error-attach = cannot attach to the KMES ring buffers (SeSecurityPrivilege required)
revstrm-error-bad-origin = unknown origin class (expected userspace, kmes, kacs, or lcs)
revstrm-error-bad-pattern = invalid type glob
