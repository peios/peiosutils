// A small Markdown-to-terminal renderer for fragment bodies.
//
// Fragment prose is simple — paragraphs, the odd heading, bullet lists, and
// inline `**bold**`, `*emphasis*` and `` `code` ``. Rather than pull in a full
// Markdown + terminal-styling stack, regman renders the handful of constructs
// it actually uses, with width-aware word wrapping. Markup that is really
// markup is always interpreted (so a pipe never shows a literal `**`); ANSI
// styling is layered on only when `Style::color` is set.
//
// What counts as markup is decided by looking for the closer, not by toggling
// a flag, so a delimiter with no partner stays in the text where the author
// put it. Full CommonMark emphasis rules are not implemented and are not
// wanted here; see `parse_inline` for the one extra rule that is.

/// Terminal styling, toggled off for non-tty / `NO_COLOR` output.
#[derive(Clone, Copy, Debug)]
pub struct Style {
    color: bool,
}

impl Style {
    pub fn new(color: bool) -> Self {
        Self { color }
    }
    /// No ANSI — used for pipes and for deterministic tests.
    pub fn plain() -> Self {
        Self { color: false }
    }

    fn wrap(self, s: &str, code: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{s}\x1b[0m")
        } else {
            s.to_string()
        }
    }
    pub fn bold(self, s: &str) -> String {
        self.wrap(s, "1")
    }
    pub fn dim(self, s: &str) -> String {
        self.wrap(s, "2")
    }
    pub fn code(self, s: &str) -> String {
        self.wrap(s, "36")
    }
    pub fn warn(self, s: &str) -> String {
        self.wrap(s, "1;31")
    }

    fn span(self, s: &str, bold: bool, code: bool) -> String {
        let mut out = s.to_string();
        if code {
            out = self.code(&out);
        }
        if bold {
            out = self.bold(&out);
        }
        out
    }
}

/// Render a Markdown body block to styled, width-wrapped terminal text. The
/// result has no leading or trailing blank lines.
pub fn render(body: &str, width: usize, style: Style) -> String {
    let mut out = String::new();
    let mut para: Vec<String> = Vec::new();
    // The lines of the bullet currently being collected. A bullet's
    // continuation lines belong to the bullet, not to a new paragraph: wrapped
    // source prose is one item, so it has to be laid out as one. Treating each
    // source line separately lost the hanging indent and — worse — split an
    // inline span that opened on one line and closed on the next, leaving the
    // markers in the output with nothing to pair them.
    let mut item: Option<Vec<String>> = None;

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_item(&mut out, &mut item, width, style);
            flush_para(&mut out, &mut para, width, style);
            out.push('\n');
        } else if let Some(text) = heading(trimmed) {
            flush_item(&mut out, &mut item, width, style);
            flush_para(&mut out, &mut para, width, style);
            out.push_str(&style.bold(&text));
            out.push('\n');
        } else if let Some(first) = bullet(trimmed) {
            flush_item(&mut out, &mut item, width, style);
            flush_para(&mut out, &mut para, width, style);
            item = Some(vec![first]);
        } else if let Some(lines) = item.as_mut() {
            lines.push(trimmed.to_string());
        } else {
            para.push(trimmed.to_string());
        }
    }
    flush_item(&mut out, &mut item, width, style);
    flush_para(&mut out, &mut para, width, style);

    while out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Strip inline markup to plain text — for one-line contexts (the Values index
/// summaries), where styling and wrapping don't apply.
pub fn strip_inline(s: &str) -> String {
    parse_inline(s).into_iter().map(|seg| seg.text).collect()
}

/// Lay out one collected bullet, hanging its continuation lines under the
/// text rather than under the marker.
fn flush_item(out: &mut String, item: &mut Option<Vec<String>>, width: usize, style: Style) {
    let Some(lines) = item.take() else {
        return;
    };
    let wrapped = layout_paragraph(&lines.join(" "), width.saturating_sub(4), style);
    for (i, l) in wrapped.lines().enumerate() {
        out.push_str(if i == 0 { "  - " } else { "    " });
        out.push_str(l);
        out.push('\n');
    }
}

fn flush_para(out: &mut String, para: &mut Vec<String>, width: usize, style: Style) {
    if para.is_empty() {
        return;
    }
    out.push_str(&layout_paragraph(&para.join(" "), width, style));
    out.push('\n');
    para.clear();
}

fn heading(line: &str) -> Option<String> {
    line.starts_with('#')
        .then(|| line.trim_start_matches('#').trim().to_string())
}

fn bullet(line: &str) -> Option<String> {
    line.strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))
        .map(|s| s.trim().to_string())
}

struct Word {
    vis: usize,
    styled: String,
}

/// Word-wrap one paragraph to `width` visible columns, applying inline styling.
/// Wrapping is on visible characters; ANSI escapes carry zero width.
fn layout_paragraph(text: &str, width: usize, style: Style) -> String {
    let mut words: Vec<Word> = Vec::new();
    let mut cur = Word {
        vis: 0,
        styled: String::new(),
    };

    for seg in parse_inline(text) {
        let mut piece = String::new();
        for ch in seg.text.chars() {
            if ch.is_whitespace() {
                push_piece(&mut cur, &mut piece, &seg, style);
                if cur.vis > 0 {
                    words.push(std::mem::replace(
                        &mut cur,
                        Word {
                            vis: 0,
                            styled: String::new(),
                        },
                    ));
                }
            } else {
                piece.push(ch);
            }
        }
        push_piece(&mut cur, &mut piece, &seg, style);
    }
    if cur.vis > 0 {
        words.push(cur);
    }

    let width = width.max(1);
    let mut out = String::new();
    let mut col = 0usize;
    for w in words {
        if col == 0 {
            out.push_str(&w.styled);
            col = w.vis;
        } else if col + 1 + w.vis <= width {
            out.push(' ');
            out.push_str(&w.styled);
            col += 1 + w.vis;
        } else {
            out.push('\n');
            out.push_str(&w.styled);
            col = w.vis;
        }
    }
    out
}

fn push_piece(cur: &mut Word, piece: &mut String, seg: &Seg, style: Style) {
    if piece.is_empty() {
        return;
    }
    cur.vis += piece.chars().count();
    cur.styled.push_str(&style.span(piece, seg.bold, seg.code));
    piece.clear();
}

struct Seg {
    text: String,
    bold: bool,
    code: bool,
}

/// Split a string into styled runs on `**` (bold), `*` (emphasis) and
/// `` ` `` (code).
///
/// A delimiter is markup only when its closer is actually present: an opener
/// with nothing to close it is literal text, so `2 ** 3`, a lone backtick and
/// a bare `*` survive into the output as written. Deciding that by looking
/// ahead rather than by toggling a flag is what makes it true — a toggle
/// silently swallows the odd delimiter and mis-styles everything after it,
/// which is what this used to do despite the comment claiming otherwise.
///
/// `*` and `**` additionally require tight delimiters — no whitespace just
/// inside either end — which is the one rule that keeps arithmetic and globs
/// out of it: `2 * 3 * 4` and `*.conf and *.h` are matched pairs that were
/// never meant as markup. Code spans take no such rule, since a backtick
/// span's content is whatever is between the backticks.
///
/// Emphasis carries no styling of its own: the markers are removed and the
/// text rendered plain. Italic is not dependable across terminals, and bold
/// already covers "make this stand out".
fn parse_inline(s: &str) -> Vec<Seg> {
    let chars: Vec<char> = s.chars().collect();
    let mut segs = Vec::new();
    let mut buf = String::new();
    emit(&chars, 0, chars.len(), false, false, &mut buf, &mut segs);
    flush_seg(&mut segs, &mut buf, false, false);
    segs
}

/// Walk `chars[from..to]` under the given styles, recursing into each span so
/// that nesting (`**a `b` c**`) carries both.
fn emit(
    chars: &[char],
    from: usize,
    to: usize,
    bold: bool,
    code: bool,
    buf: &mut String,
    segs: &mut Vec<Seg>,
) {
    let mut i = from;
    while i < to {
        let c = chars[i];

        // Inside a code span nothing else is markup: the content is literal
        // until the closing backtick, which the caller located.
        if code {
            buf.push(c);
            i += 1;
            continue;
        }

        if c == '`' {
            if let Some(close) = find_closer(chars, i + 1, to, '`', 1, false) {
                flush_seg(segs, buf, bold, code);
                emit(chars, i + 1, close, bold, true, buf, segs);
                flush_seg(segs, buf, bold, true);
                i = close + 1;
                continue;
            }
        } else if c == '*' {
            let n = run_len(chars, i, '*').min(2);
            if let Some(close) = find_closer(chars, i + n, to, '*', n, true) {
                if n == 2 {
                    flush_seg(segs, buf, bold, code);
                    emit(chars, i + n, close, true, code, buf, segs);
                    flush_seg(segs, buf, true, code);
                } else {
                    // Emphasis is undecorated, so there is no style change to
                    // flush around: drop the markers and keep going.
                    emit(chars, i + n, close, bold, code, buf, segs);
                }
                i = close + n;
                continue;
            }
        }

        buf.push(c);
        i += 1;
    }
}

/// How many `delim` characters run consecutively from `i`.
fn run_len(chars: &[char], i: usize, delim: char) -> usize {
    chars[i..].iter().take_while(|&&c| c == delim).count()
}

/// Where the span opened before `from` closes, or `None` if it never does —
/// in which case the opener is not markup at all.
///
/// The closer must be a run of exactly `n`, so a `**` span is not closed by a
/// stray `*`, and must leave the span non-empty. With `tight`, neither end of
/// the content may be whitespace.
fn find_closer(
    chars: &[char],
    from: usize,
    to: usize,
    delim: char,
    n: usize,
    tight: bool,
) -> Option<usize> {
    if from >= to || (tight && chars[from].is_whitespace()) {
        return None;
    }
    let mut i = from;
    while i < to {
        if chars[i] == delim && i > from && run_len(chars, i, delim) == n {
            let inner_ends_tight = !tight || !chars[i - 1].is_whitespace();
            if inner_ends_tight && i + n <= to {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn flush_seg(segs: &mut Vec<Seg>, buf: &mut String, bold: bool, code: bool) {
    if !buf.is_empty() {
        segs.push(Seg {
            text: std::mem::take(buf),
            bold,
            code,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_to_width() {
        let out = render("one two three four five six", 12, Style::plain());
        assert!(out.lines().all(|l| l.chars().count() <= 12));
        // Round-trips the words in order.
        assert_eq!(out.split_whitespace().collect::<Vec<_>>(), ["one","two","three","four","five","six"]);
    }

    #[test]
    fn strips_markup_in_plain_mode() {
        let out = render("This is **bold** and `code`.", 80, Style::plain());
        assert_eq!(out, "This is bold and code.");
        assert!(!out.contains('*'));
        assert!(!out.contains('`'));
    }

    #[test]
    fn emphasis_loses_its_markers() {
        let out = render("A value that is *not* here.", 80, Style::plain());
        assert_eq!(out, "A value that is not here.");
    }

    /// The whole point of looking ahead: an opener with no closer is text.
    /// Toggling instead swallowed the delimiter and mis-styled the remainder.
    #[test]
    fn an_unmatched_delimiter_stays_literal() {
        for (input, want) in [
            ("2 ** 3 and more after it", "2 ** 3 and more after it"),
            ("an unclosed ` and more after it", "an unclosed ` and more after it"),
            ("a lone * and more after it", "a lone * and more after it"),
        ] {
            assert_eq!(render(input, 80, Style::plain()), want, "input: {input}");
        }
    }

    /// Matched pairs that were never markup. The tight rule is what excludes
    /// them, and it is the only rule beyond "is there a closer?".
    #[test]
    fn tight_delimiters_keep_arithmetic_and_globs_literal() {
        for text in ["2 * 3 * 4", "*.conf and *.h", "a ** b ** c"] {
            assert_eq!(render(text, 80, Style::plain()), text, "input: {text}");
        }
    }

    /// An unmatched delimiter inside a code span is content, not markup —
    /// which is what keeps `Tag.*` and `Counter.*` intact in netd's prose.
    #[test]
    fn a_code_span_protects_what_is_inside_it() {
        let out = render("a `Tag.*` or `Counter.*` condition", 80, Style::plain());
        assert_eq!(out, "a Tag.* or Counter.* condition");
    }

    #[test]
    fn spans_nest() {
        let out = render("**bold with `code` inside**", 80, Style::new(true));
        assert!(out.contains("\x1b[1m"), "bold applied: {out:?}");
        assert!(out.contains("\x1b[36m"), "code applied inside it: {out:?}");
        assert_eq!(render("**bold with `code` inside**", 80, Style::plain()), "bold with code inside");
    }

    /// Emphasis may hold other markup, and markers still come off cleanly.
    #[test]
    fn emphasis_may_contain_code() {
        assert_eq!(
            render("the *`Enabled` value* matters", 80, Style::plain()),
            "the Enabled value matters"
        );
    }

    #[test]
    fn applies_ansi_when_colored() {
        let out = render("a **b** c", 80, Style::new(true));
        assert!(out.contains("\x1b[1mb\x1b[0m"));
        // 'a' and 'c' stay unstyled.
        assert!(out.starts_with("a "));
    }

    #[test]
    fn bold_span_across_words() {
        let out = render("**two words** plain", 80, Style::new(true));
        assert!(out.contains("\x1b[1mtwo\x1b[0m \x1b[1mwords\x1b[0m"));
        assert!(out.trim_end().ends_with("plain"));
    }

    #[test]
    fn heading_and_bullets() {
        let body = "# Title\n\n- first item\n- second item";
        let out = render(body, 80, Style::plain());
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "Title");
        assert!(lines.iter().any(|l| *l == "  - first item"));
        assert!(lines.iter().any(|l| *l == "  - second item"));
    }

    /// A bullet's continuation lines are part of the bullet: they hang under
    /// the text, and an inline span may open on one and close on the next.
    /// Both used to break, the second visibly once markup stopped toggling.
    #[test]
    fn a_bullet_continues_across_source_lines() {
        let out = render(
            "- `restart` — what the process **is or\n  runs as**: `ImagePath`.",
            72,
            Style::plain(),
        );
        assert!(!out.contains('*'), "span must close across the lines: {out:?}");
        for (i, l) in out.lines().enumerate() {
            assert!(
                l.starts_with(if i == 0 { "  - " } else { "    " }),
                "line {i} not hung under the text: {l:?}"
            );
        }
    }

    #[test]
    fn bullet_hanging_indent() {
        let out = render("- a fairly long bullet that needs to wrap onto another line here", 24, Style::plain());
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines[0].starts_with("  - "));
        assert!(lines[1].starts_with("    ")); // continuation indented under text
    }

    #[test]
    fn strip_inline_is_plain() {
        assert_eq!(strip_inline("**Validation** is `key`"), "Validation is key");
    }
}
