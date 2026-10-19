.SH SECURITY
Numeric user and group identities are projections of Peios SIDs. Some
logon\-type groups have no numeric representation, and group attributes such
as deny\-only cannot be represented by a group number.
.PP
The default process view adds a context containing user SID and integrity. A
named\-account query has no runtime token to supply that context.
\fBPOSIXLY_CORRECT\fR also suppresses it. Use token(1) for the broader native
view rather than treating numeric equality as token equivalence.
.PP
Process group membership and named\-account membership can differ because
tokens include automatically supplied memberships not stored as account
records. Real/effective identity reflects the primary/effective identity model
rather than a set\-id privilege mechanism.
