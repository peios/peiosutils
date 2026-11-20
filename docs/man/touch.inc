.SH SECURITY
Requested creation security applies to newly created objects. The shared
creation options select owner, group, integrity label, inheritance protection,
or a complete SDDL descriptor. The file is created carrying the descriptor;
see mkdir(1) for how an integrity label is set. A dangling symbolic link at the
name is an error: touch does not create the file at the link's target. A
creation request does not
replace the security of an object that already exists, including one created
by another program after touch found the name free.
