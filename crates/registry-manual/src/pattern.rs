// Wildcard path components in an anchor.
//
// Most registry documentation names a fixed path. Some does not: a service
// definition lives at `Machine\System\Services\<name>`, an interface record at
// `Machine\System\Network\Interfaces\<ifid>`, a principal's policy at
// `Machine\Generic\Authn\Policy\<SID>`. There is one page for the whole family,
// and it has to answer a query naming any one member of it.
//
// So a `<…>` segment in an anchor is a wildcard matching exactly one path
// component: one or more characters that are neither `\` nor a space.
//
// Some families are trees rather than flat sets. A PNP rule's subkeys are
// exceptions, nesting to twelve, and a netd profile's subkeys are derived
// profiles; the vocabulary is identical at every depth, so one page is still
// the right answer, but no fixed number of `<…>` components reaches it. For
// those, `<name...>` — a `...` immediately inside the closing bracket — spans
// separators, matching one component or many.
//
// Neither form crosses a space. That is the load-bearing rule: the `path[ value]`
// anchor form (see `fragment`) cannot be split reliably from the outside, since
// both a key path and a value name may contain spaces, so a wildcard allowed to
// swallow one could consume the boundary between them and let a key-path
// wildcard answer for an unrelated value doc.
//
// Both forms demand at least one character, so `\X\<Y...>` no more matches
// `\X\` than `\X\<Y>` does — a family page never answers for the key above it,
// which has its own.
//
// Apart from a trailing `...`, the name inside the brackets is for the reader,
// not the matcher: `<name>`, `<ifid>` and `<x>` all behave identically. There is
// no escape for a literal `<…>` component. LCS permits `<` and `>` in a key name
// (only `\` and `/` are forbidden), so a key genuinely spelled `<Y>` is
// constructible and would be indistinguishable from a wildcard — that is an
// accepted trade, because such a key does not occur and the notation was already
// the house convention for a varying component before it meant anything to the
// tool.
//
// Specificity is the caller's business, not this module's: a concrete record, a
// single-component wildcard and a spanning one can all match the same query, and
// `query` decides which wins.

/// Could this anchor contain a wildcard? A cheap gate — a bare `<` with no
/// closing `>` is matched literally, so this may say yes where `matches` then
/// behaves exactly as byte equality would.
pub fn has_wildcard(anchor: &str) -> bool {
    anchor.as_bytes().contains(&b'<')
}

/// Does this anchor carry a `<…...>` that spans separators? Used by `query` to
/// rank specificity; mirrors `walk`'s own bracket scan exactly, so the two can
/// never disagree about what is a wildcard.
pub fn spans_components(anchor: &str) -> bool {
    let a = anchor.as_bytes();
    let mut i = 0;
    while i < a.len() {
        if a[i] != b'<' {
            i += 1;
            continue;
        }
        // `walk` takes the first `>` at or after the `<`; with none, the `<` is
        // literal and so is every one after it, since none can be closed either.
        let Some(close) = a[i..].iter().position(|&b| b == b'>').map(|p| i + p) else {
            return false;
        };
        if a[i + 1..close].ends_with(b"...") {
            return true;
        }
        i = close + 1;
    }
    false
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

/// Match a pattern without recursive backtracking.
///
/// Each row records which query prefixes can match the processed pattern. A
/// wildcard extends reachable prefixes one byte at a time, so ambiguous wildcard
/// partitions cannot multiply work. Time is O(pattern bytes * query bytes), and
/// the two reused rows require O(query bytes) space. Matching stays byte-based,
/// including the original wildcard and under-key boundary rules.
fn walk(anchor: &[u8], query: &[u8], tail_ok: &dyn Fn(&[u8]) -> bool) -> bool {
    let mut reachable = vec![false; query.len() + 1];
    let mut next = vec![false; query.len() + 1];
    reachable[0] = true;
    let mut offset = 0;
    loop {
        if reachable[query.len()] && tail_ok(&anchor[offset..]) {
            return true;
        }
        if offset == anchor.len() {
            return false;
        }
        next.fill(false);
        let remaining = &anchor[offset..];
        if let Some((width, spans)) = wildcard(remaining) {
            for (index, &byte) in query.iter().enumerate() {
                let allowed = byte != b' ' && (spans || byte != b'\\');
                next[index + 1] = allowed && (reachable[index] || next[index]);
            }
            offset += width;
        } else {
            for (index, &byte) in query.iter().enumerate() {
                next[index + 1] = reachable[index] && remaining[0] == byte;
            }
            offset += 1;
        }
        if !next.iter().any(|&prefix| prefix) {
            return false;
        }
        std::mem::swap(&mut reachable, &mut next);
    }
}

/// Return a leading wildcard's byte length and whether it may cross separators.
fn wildcard(anchor: &[u8]) -> Option<(usize, bool)> {
    if anchor.first() != Some(&b'<') {
        return None;
    }
    let close = anchor.iter().position(|&byte| byte == b'>')?;
    Some((close + 1, anchor[1..close].ends_with(b"...")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_free_anchors_behave_as_byte_equality() {
        assert!(matches_exact(
            "machine\\system\\kmes buffercapacity",
            "machine\\system\\kmes buffercapacity"
        ));
        assert!(!matches_exact(
            "machine\\system\\kmes buffercapacity",
            "machine\\system\\kmes nope"
        ));
        assert!(matches_under_key(
            "machine\\system\\kmes",
            "machine\\system\\kmes"
        ));
        assert!(matches_under_key(
            "machine\\system\\kmes buffercapacity",
            "machine\\system\\kmes"
        ));
        assert!(!matches_under_key(
            "machine\\system\\kmesfoo",
            "machine\\system\\kmes"
        ));
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
        assert!(!matches_exact(
            "machine\\system\\services\\<name>",
            "machine\\system\\services\\"
        ));
        assert!(!matches_exact(
            "machine\\system\\services\\<name>",
            "machine\\system\\services"
        ));
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

    #[test]
    fn spanning_wildcard_crosses_separators() {
        // A PNP exception nests arbitrarily; one record answers at every depth.
        let a = "machine\\system\\network\\rules\\<layer>\\<rule...> actions";
        assert!(matches_exact(
            a,
            "machine\\system\\network\\rules\\packet\\ssh actions"
        ));
        assert!(matches_exact(
            a,
            "machine\\system\\network\\rules\\packet\\ssh\\from-lan actions"
        ));
        assert!(matches_exact(
            a,
            "machine\\system\\network\\rules\\packet\\ssh\\from-lan\\not-vpn actions"
        ));
    }

    #[test]
    fn spanning_wildcard_still_will_not_cross_the_value_separator() {
        // The one rule that must survive: `...` spans `\`, never the space, or
        // a key-path wildcard could swallow the value name after it.
        assert!(!matches_exact(
            "machine\\system\\network\\rules\\<layer>\\<rule...>",
            "machine\\system\\network\\rules\\packet\\ssh actions"
        ));
        assert!(!matches_exact("\\x\\<y...>\\z", "\\x\\a b\\z"));
    }

    #[test]
    fn spanning_wildcard_requires_at_least_one_component() {
        // `Rules\Packet` has its own page; the family must not answer for it.
        assert!(!matches_exact(
            "machine\\system\\network\\rules\\<layer>\\<rule...>",
            "machine\\system\\network\\rules\\packet"
        ));
        assert!(!matches_exact(
            "machine\\system\\network\\rules\\<layer>\\<rule...>",
            "machine\\system\\network\\rules\\packet\\"
        ));
    }

    #[test]
    fn a_spanning_wildcard_does_not_make_its_neighbours_span() {
        // `<a>` is still one component beside a spanning sibling, so a literal
        // the spanning one cannot reach past still pins it: `<a>` alone has to
        // cover `one\two`, and it cannot.
        assert!(!matches_exact(
            "\\x\\<a>\\z\\<b...>",
            "\\x\\one\\two\\z\\three"
        ));
        assert!(matches_exact(
            "\\x\\<a>\\z\\<b...>",
            "\\x\\one\\z\\two\\three"
        ));
    }

    #[test]
    fn key_query_reaches_a_spanning_wildcards_attached_values() {
        assert!(matches_under_key(
            "machine\\system\\network\\rules\\<layer>\\<rule...> actions",
            "machine\\system\\network\\rules\\packet\\ssh\\from-lan"
        ));
        // ...but the family is not listed under the layer key, which is a
        // concrete page of its own: those rules are subkeys, not its values.
        assert!(!matches_under_key(
            "machine\\system\\network\\rules\\<layer>\\<rule...> actions",
            "machine\\system\\network\\rules\\packet"
        ));
    }

    #[test]
    fn spans_components_mirrors_the_matcher() {
        assert!(spans_components("\\x\\<y...>"));
        assert!(spans_components("\\x\\<a>\\<b...>\\z"));
        assert!(!spans_components("\\x\\<y>"));
        assert!(!spans_components("\\x\\<a>\\<b>"));
        assert!(!spans_components("machine\\system\\kmes buffercapacity"));
        // A `...` outside the brackets is ordinary text, not a modifier.
        assert!(!spans_components("\\x\\<y>..."));
        // Unterminated: `walk` treats it literally, and so must this.
        assert!(!spans_components("\\x\\<y..."));
    }

    #[test]
    fn dots_outside_the_brackets_do_not_span() {
        assert!(!matches_exact("\\x\\<y>...\\z", "\\x\\a\\b...\\z"));
        assert!(matches_exact("\\x\\<y>...\\z", "\\x\\a...\\z"));
    }

    #[test]
    fn ambiguous_wildcards_do_not_repeat_partitions() {
        let anchor = format!("{}z", "<x>".repeat(32));
        let query = "a".repeat(64);
        assert!(!matches_exact(&anchor, &query));
        assert!(!matches_under_key(&anchor, &query));
        assert!(matches_exact(
            &format!("{}z", "<x>".repeat(32)),
            &format!("{query}z")
        ));
    }

    #[test]
    fn long_literal_prefix_with_wildcard_uses_no_call_stack() {
        let prefix = "x".repeat(2048);
        let anchor = format!("{prefix}<name>");
        let query = format!("{prefix}value");
        assert!(matches_exact(&anchor, &query));
    }

    #[test]
    fn iterative_matching_preserves_small_pattern_semantics() {
        // A deliberately small copy of the old matcher is an independent oracle
        // for ambiguous boundaries, unterminated brackets, and under-key tails.
        fn original(anchor: &[u8], query: &[u8], tail_ok: &dyn Fn(&[u8]) -> bool) -> bool {
            if query.is_empty() {
                return tail_ok(anchor);
            }
            if anchor.is_empty() {
                return false;
            }
            if anchor[0] == b'<' {
                if let Some(close) = anchor.iter().position(|&byte| byte == b'>') {
                    let spans = anchor[1..close].ends_with(b"...");
                    for index in 0..query.len() {
                        if query[index] == b' ' || (!spans && query[index] == b'\\') {
                            break;
                        }
                        if original(&anchor[close + 1..], &query[index + 1..], tail_ok) {
                            return true;
                        }
                    }
                    return false;
                }
            }
            anchor[0] == query[0] && original(&anchor[1..], &query[1..], tail_ok)
        }
        let pieces = ["a", "b", " ", "\\", "<x>", "<x...>", "<", ">", "é"];
        let queries = ["", "a", "aa", "ab", "a b", "a\\b", "<a", ">", "é", "éa"];
        for first in pieces {
            for second in pieces {
                for third in pieces {
                    let anchor = format!("{first}{second}{third}");
                    for query in queries {
                        for under_key in [false, true] {
                            let tail_ok =
                                |rest: &[u8]| rest.is_empty() || (under_key && rest[0] == b' ');
                            assert_eq!(
                                walk(anchor.as_bytes(), query.as_bytes(), &tail_ok),
                                original(anchor.as_bytes(), query.as_bytes(), &tail_ok),
                                "anchor={anchor:?} query={query:?} under_key={under_key}"
                            );
                        }
                    }
                }
            }
        }
    }
}
