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
            // `<name...>` spans separators; `<name>` stops at one component.
            // Neither may cross the space that divides a path from a value.
            let spans = a[1..close].ends_with(b"...");
            let mut n = 0;
            while n < q.len() && q[n] != b' ' && (spans || q[n] != b'\\') {
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

    #[test]
    fn spanning_wildcard_crosses_separators() {
        // A PNP exception nests arbitrarily; one record answers at every depth.
        let a = "machine\\system\\network\\rules\\<layer>\\<rule...> actions";
        assert!(matches_exact(a, "machine\\system\\network\\rules\\packet\\ssh actions"));
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
        assert!(!matches_exact("\\x\\<a>\\z\\<b...>", "\\x\\one\\two\\z\\three"));
        assert!(matches_exact("\\x\\<a>\\z\\<b...>", "\\x\\one\\z\\two\\three"));
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
}
