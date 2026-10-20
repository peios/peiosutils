---
title: Registry Documentation Patterns
type: concept
description: Understand dynamic registry names, lookup precedence, and the text that regman renders.
---

Registry documentation patterns describe families of keys without publishing
one record for every instance. Use a pattern only where the implementation
accepts dynamic names; a literal record makes a fixed name easier to verify.

## Match dynamic names

In a canonical path, `<name>` matches one path component. `<name...>` matches
one or more components separated by backslashes. The label inside the brackets
explains the role to a reader; it does not constrain the matched text.
Neither pattern matches a space. The format has no escape for a literal name
that itself has this placeholder shape.

For example, `Machine\Software\Example\Profiles\<profile>` describes one
profile key. A pattern ending in `<path...>` also reaches its descendants.
Use the narrower form when the documented behavior applies to one level.

Literal matches take precedence over patterns. Among patterns, more specific
matches take precedence over broader ones. Key and value lookup remain
separate: a matching key record does not define all its values.

Dots do not split registry components. A value pattern such as `Local.<op>`
can match `Local.User.Equal` as well as `Local.Equal`. If those names have
different meanings, document the operators explicitly instead of relying on
placeholder labels to separate them.

Overlapping records from different providers remain visible. Provider order
is not a mechanism for overriding another package's documentation.

## Write terminal prose

Use paragraphs, headings, bullet lists, bold, and code spans. The renderer
rewraps paragraphs to terminal width. Follow the shared prose style and avoid
relying on emphasis styling that a plain-text terminal cannot represent.
Unmatched markup remains literal; whitespace inside an emphasis marker can
prevent it from being recognized.

A line beginning with `--- ` starts another record, even in body text. Use
`***` or `___` for a horizontal separator if one is needed. Keep examples of
record fences out of a fragment's prose.

Piped output and `NO_COLOR` suppress terminal styling. Review both styled and
plain output when presentation carries meaning. State a warning in words;
color alone cannot communicate it.

## Keep format and meaning separate

Headers such as `access` or `since` do not extend the parser's schema. Put
required access and relevant constraints in the body, and omit historical
narrative. Unknown headers being ignored does not make them supported.

Regman does not read the live registry to validate a documented default or
range. Review those claims against the consuming code, including how that
code handles a missing or malformed value.
