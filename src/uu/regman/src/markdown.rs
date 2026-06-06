// A small Markdown-to-terminal renderer for fragment bodies.
//
// Fragment prose is simple — paragraphs, the odd heading, bullet lists, and
// inline `**bold**` / `` `code` ``. Rather than pull in a full Markdown +
// terminal-styling stack, regman renders the handful of constructs it actually
// uses: inline emphasis, headings, bullets, and width-aware word wrapping.
// Markup is always interpreted (so a pipe never shows literal `**`); ANSI
// styling is layered on only when `Style::color` is set.

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

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush_para(&mut out, &mut para, width, style);
            out.push('\n');
        } else if let Some(text) = heading(trimmed) {
            flush_para(&mut out, &mut para, width, style);
            out.push_str(&style.bold(&text));
            out.push('\n');
        } else if let Some(item) = bullet(trimmed) {
            flush_para(&mut out, &mut para, width, style);
            let wrapped = layout_paragraph(&item, width.saturating_sub(4), style);
            for (i, l) in wrapped.lines().enumerate() {
                out.push_str(if i == 0 { "  - " } else { "    " });
                out.push_str(l);
                out.push('\n');
            }
        } else {
            para.push(trimmed.to_string());
        }
    }
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

/// Split a string into styled runs on `**` (bold) and `` ` `` (code) toggles.
/// A `*` not doubled, or an unmatched toggle, is treated literally.
fn parse_inline(s: &str) -> Vec<Seg> {
    let mut segs = Vec::new();
    let mut buf = String::new();
    let mut bold = false;
    let mut code = false;
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if !code && c == '*' && chars.peek() == Some(&'*') {
            chars.next();
            flush_seg(&mut segs, &mut buf, bold, code);
            bold = !bold;
        } else if c == '`' {
            flush_seg(&mut segs, &mut buf, bold, code);
            code = !code;
        } else {
            buf.push(c);
        }
    }
    flush_seg(&mut segs, &mut buf, bold, code);
    segs
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
