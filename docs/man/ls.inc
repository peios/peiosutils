.SH SECURITY
The Peios long view reports selected native security information rather than
treating traditional ownership and mode bits as access authority. A displayed
owner or execution/protection marker is a summary, not a complete descriptor
export. Use sd(1) for descriptor inspection, noting its component\-selection
limits.
.SH NOTES
Human\-oriented listings are not a filename interchange format. Filenames can
contain whitespace, newlines, and other bytes that require unambiguous
handling by a consumer.
