// The handful of Python string semantics the lint has to reproduce.
//
// `pkm/tools/evman.py` is the reference implementation of the §6.10 lint, and
// `evman lint` must report what it reports on the same input — the same
// lines, the same messages, byte for byte. Its parser leans on `str.strip`,
// `str.split`, `str.splitlines` and `repr`, and each of those draws its
// lines differently from the nearest Rust method:
//
//   - Python's whitespace (`str.isspace`, regex `\s`) includes U+001C..U+001F,
//     which Rust's `char::is_whitespace` does not.
//   - `Path.read_text` opens in universal-newline mode and `splitlines` then
//     breaks on \v, \f, U+001C..U+001E, U+0085, U+2028 and U+2029 as well.
//   - A message quotes the offending text with `{...!r}`, Python's repr.
//
// These are the only places parity needs care, so they are kept together
// here rather than scattered through the parser.

/// Python's `str.isspace` for one character.
pub fn is_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// Python's `str.strip()`.
pub fn strip(s: &str) -> &str {
    s.trim_matches(is_space)
}

/// Python's `str.split()` with no separator: runs of whitespace separate,
/// and no empty piece is produced.
pub fn split_ws(s: &str) -> Vec<&str> {
    s.split(is_space).filter(|p| !p.is_empty()).collect()
}

/// The lines of a file as `read_text().splitlines()` sees them: `\r\n` and
/// a lone `\r` are one break each, the other Unicode line boundaries break
/// too, and a final terminator does not produce an empty last line.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        let breaks = matches!(
            c,
            '\n' | '\r'
                | '\u{0b}'
                | '\u{0c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        );
        if !breaks {
            continue;
        }
        out.push(&text[start..i]);
        let mut end = i + c.len_utf8();
        if c == '\r' {
            if let Some(&(j, '\n')) = chars.peek() {
                chars.next();
                end = j + 1;
            }
        }
        start = end;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// Python's `repr()` of a string, as an f-string's `!r` writes it.
///
/// Single quotes unless the text holds a `'` and no `"`; backslash and the
/// chosen quote escaped; `\t`, `\n`, `\r` by name; other unprintable
/// characters as `\xNN`, `\uNNNN` or `\UNNNNNNNN`. Python's printability is
/// a Unicode-database property; this covers the control, format and
/// separator characters a text fragment could plausibly contain, which is
/// everything short of unassigned code points.
pub fn repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if printable(c) => out.push(c),
            c => {
                let n = c as u32;
                if n <= 0xff {
                    out.push_str(&format!("\\x{n:02x}"));
                } else if n <= 0xffff {
                    out.push_str(&format!("\\u{n:04x}"));
                } else {
                    out.push_str(&format!("\\U{n:08x}"));
                }
            }
        }
    }
    out.push(quote);
    out
}

/// `str.isprintable` for one character: false for the Cc, Cf, Co, Zl, Zp
/// and non-space Zs characters.
fn printable(c: char) -> bool {
    let n = c as u32;
    !matches!(
        n,
        0x00..=0x1f
            | 0x7f..=0xa0
            | 0xad
            | 0x600..=0x605
            | 0x61c
            | 0x6dd
            | 0x70f
            | 0x890..=0x891
            | 0x8e2
            | 0x1680
            | 0x180e
            | 0x2000..=0x200f
            | 0x2028..=0x202f
            | 0x205f..=0x2064
            | 0x2066..=0x206f
            | 0x3000
            | 0xe000..=0xf8ff
            | 0xfeff
            | 0xfff9..=0xfffb
            | 0x110bd
            | 0x110cd
            | 0x13430..=0x1343f
            | 0x1bca0..=0x1bca3
            | 0x1d173..=0x1d17a
            | 0xe0001
            | 0xe0020..=0xe007f
            | 0xf_0000..=0x10_ffff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_takes_python_whitespace() {
        assert_eq!(strip("  a b \t"), "a b");
        // U+001F is whitespace to Python and not to Rust.
        assert_eq!(strip("\u{1f}x\u{1f}"), "x");
    }

    #[test]
    fn split_ws_drops_empty_pieces() {
        assert_eq!(split_ws("  field   a.b  "), vec!["field", "a.b"]);
        assert_eq!(split_ws(""), Vec::<&str>::new());
    }

    #[test]
    fn splitlines_matches_python() {
        assert_eq!(splitlines("a\nb\n"), vec!["a", "b"]);
        assert_eq!(splitlines("a\r\nb\rc"), vec!["a", "b", "c"]);
        assert_eq!(splitlines("a\n\nb"), vec!["a", "", "b"]);
        assert_eq!(splitlines("a\u{0c}b\u{2028}c"), vec!["a", "b", "c"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
        assert_eq!(splitlines("\n"), vec![""]);
    }

    #[test]
    fn repr_quotes_like_python() {
        assert_eq!(repr("abc"), "'abc'");
        assert_eq!(repr("it's"), "\"it's\"");
        assert_eq!(repr("'\""), "'\\'\"'");
        assert_eq!(repr("a\tb\\"), "'a\\tb\\\\'");
        assert_eq!(repr("\u{1}\u{a0}é"), "'\\x01\\xa0é'");
        assert_eq!(repr("\u{2028}"), "'\\u2028'");
    }
}
