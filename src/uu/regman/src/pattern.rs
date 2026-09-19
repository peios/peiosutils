// Wildcard path components in an anchor.
//
// Most registry documentation names a fixed path. Some does not: a service
// definition lives at `Machine\System\Services\<name>`, an interface record at
// `Machine\System\Network\Interfaces\<ifid>`, a principal's policy at
// `Machine\Generic\Authn\Policy\<SID>`. There is one page for the whole family,
// and it has to answer a query naming any one member of it.
//
// So a `<…>` segment in an anchor is a wildcard matching exactly one path
// component: one or more characters that are neither `\` nor a space. Excluding
// the space keeps the rule unambiguous against the `path[ value]` anchor form,
// which (see `fragment`) cannot be split reliably from the outside — both a key
// path and a value name may contain spaces, so a wildcard allowed to swallow
// one could consume the boundary between them.
//
// The name inside the brackets is for the reader, not the matcher: `<name>`,
// `<ifid>` and `<x>` all behave identically. There is no escape for a literal
// `<…>` component. LCS permits `<` and `>` in a key name (only `\` and `/` are
// forbidden), so a key genuinely spelled `<Y>` is constructible and would be
// indistinguishable from a wildcard — that is an accepted trade, because such a
// key does not occur and the notation was already the house convention for a
// varying component before it meant anything to the tool.
//
// Specificity is the caller's business, not this module's: a concrete record
// and a wildcard record can both match one query, and `query` decides that the
// concrete one wins.

/// Could this anchor contain a wildcard? A cheap gate — a bare `<` with no
/// closing `>` is matched literally, so this may say yes where `matches` then
/// behaves exactly as byte equality would.
pub fn has_wildcard(anchor: &str) -> bool {
    anchor.as_bytes().contains(&b'<')
}

/// Does `anchor` denote the exact `(path, value)` named by `query`? Equivalent
/// to `anchor == query` for a wildcard-free anchor.
pub fn matches_exact(anchor: &str, query: &str) -> bool {
    if !has_wildcard(anchor) {
        return anchor == query;
    }
    walk(anchor.as_bytes(), query.as_bytes(), &|rest| rest.is_empty())
}

/// Does `anchor` belong to the key `query` — the key doc itself, or one of its
/// directly-attached values? The trailing-space test is what keeps a sibling
/// key (`...\kmesfoo`) from answering for `...\kmes`.
pub fn matches_under_key(anchor: &str, query: &str) -> bool {
    if !has_wildcard(anchor) {
        return anchor == query
            || (anchor.len() > query.len()
                && anchor.starts_with(query)
                && anchor.as_bytes()[query.len()] == b' ');
    }
    walk(anchor.as_bytes(), query.as_bytes(), &|rest| {
        rest.is_empty() || rest[0] == b' '
    })
}

/// Match the anchor pattern `a` against the literal query `q`, calling
/// `tail_ok` on whatever of the anchor is left once the query runs out.
///
/// Recursive because a wildcard's extent is not always fixed by what follows it
/// (`svc-<name>-x` is legal, if odd). Anchors are short and wildcards few, so
/// the backtracking is bounded by nothing worth optimising; the wildcard-free
/// fast paths above mean ordinary lookups never reach here at all.
fn walk(a: &[u8], q: &[u8], tail_ok: &dyn Fn(&[u8]) -> bool) -> bool {
    if q.is_empty() {
        // A pending wildcard still demands at least one character, so an anchor
        // ending in `<…>` correctly fails to match a query that stopped short.
        return tail_ok(a);
    }
    if a.is_empty() {
        return false;
    }
    if a[0] == b'<' {
        if let Some(close) = a.iter().position(|&b| b == b'>') {
            let rest = &a[close + 1..];
            let mut n = 0;
            while n < q.len() && q[n] != b'\\' && q[n] != b' ' {
                n += 1;
                if walk(rest, &q[n..], tail_ok) {
                    return true;
                }
            }
            return false;
        }
        // Unterminated `<`: not a wildcard, fall through to a literal compare.
    }
    if a[0] == q[0] {
        return walk(&a[1..], &q[1..], tail_ok);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_free_anchors_behave_as_byte_equality() {
        assert!(matches_exact("machine\\system\\kmes buffercapacity", "machine\\system\\kmes buffercapacity"));
        assert!(!matches_exact("machine\\system\\kmes buffercapacity", "machine\\system\\kmes nope"));
        assert!(matches_under_key("machine\\system\\kmes", "machine\\system\\kmes"));
        assert!(matches_under_key("machine\\system\\kmes buffercapacity", "machine\\system\\kmes"));
        assert!(!matches_under_key("machine\\system\\kmesfoo", "machine\\system\\kmes"));
    }

    #[test]
    fn wildcard_matches_one_component() {
        // \X\<Y>\Z answers for \X\B\Z — the shape Jack asked for.
        assert!(matches_exact("\\x\\<y>\\z", "\\x\\b\\z"));
        assert!(matches_exact(
            "machine\\system\\services\\<name> imagepath",
            "machine\\system\\services\\sshd imagepath"
        ));
    }

    #[test]
    fn wildcard_does_not_span_a_separator() {
        // One component, not a path tail: \X\<Y>\Z must not match \X\B\C\Z.
        assert!(!matches_exact("\\x\\<y>\\z", "\\x\\b\\c\\z"));
        assert!(!matches_exact(
            "machine\\system\\services\\<name> imagepath",
            "machine\\system\\services\\a\\b imagepath"
        ));
    }

    #[test]
    fn wildcard_does_not_span_the_value_separator() {
        // `<name>` must not swallow the space and eat the value name, which is
        // what would let a key-path wildcard match an unrelated value doc.
        assert!(!matches_exact(
            "machine\\system\\services\\<name>",
            "machine\\system\\services\\sshd imagepath"
        ));
    }

    #[test]
    fn wildcard_requires_at_least_one_character() {
        assert!(!matches_exact("\\x\\<y>\\z", "\\x\\\\z"));
        assert!(!matches_exact("machine\\system\\services\\<name>", "machine\\system\\services\\"));
        assert!(!matches_exact("machine\\system\\services\\<name>", "machine\\system\\services"));
    }

    #[test]
    fn key_query_reaches_a_wildcards_attached_values() {
        // `regman "Machine\System\Services\sshd"` must collect both the key doc
        // and every value hanging off it.
        assert!(matches_under_key(
            "machine\\system\\services\\<name>",
            "machine\\system\\services\\sshd"
        ));
        assert!(matches_under_key(
            "machine\\system\\services\\<name> imagepath",
            "machine\\system\\services\\sshd"
        ));
        // ...but not a sibling subkey's values.
        assert!(!matches_under_key(
            "machine\\system\\services\\<name>\\timerstate",
            "machine\\system\\services\\sshd"
        ));
    }

    #[test]
    fn several_wildcards_in_one_anchor() {
        assert!(matches_exact("\\x\\<a>\\z\\<b>", "\\x\\one\\z\\two"));
        assert!(!matches_exact("\\x\\<a>\\z\\<b>", "\\x\\one\\z"));
    }

    #[test]
    fn partial_component_wildcard_backtracks() {
        // Odd but legal: the wildcard's extent is not fixed by what follows.
        assert!(matches_exact("\\x\\svc-<n>-x", "\\x\\svc-foo-x"));
        assert!(!matches_exact("\\x\\svc-<n>-x", "\\x\\svc-foo-y"));
    }

    #[test]
    fn unterminated_angle_bracket_is_literal() {
        assert!(matches_exact("\\x\\<y", "\\x\\<y"));
        assert!(!matches_exact("\\x\\<y", "\\x\\b"));
    }

    #[test]
    fn has_wildcard_gates_the_slow_path() {
        assert!(has_wildcard("machine\\system\\services\\<name>"));
        assert!(!has_wildcard("machine\\system\\kmes buffercapacity"));
    }
}
