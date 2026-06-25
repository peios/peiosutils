// This file is part of the peiosutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

//! Rendering: the tree/list table, JSON, key=value pairs, and raw output.

use std::io::Write;

use crate::column::Column;
use crate::config::{Config, OutputMode};
use crate::device::Device;
use crate::error::Result;

/// Box-drawing glyphs for the NAME tree prefix.
struct Glyphs {
    branch: &'static str, // non-last sibling
    last: &'static str,   // last sibling
    vert: &'static str,   // ancestor continues
    space: &'static str,  // ancestor ended
}

const UNICODE: Glyphs = Glyphs { branch: "├─", last: "└─", vert: "│ ", space: "  " };
const ASCII: Glyphs = Glyphs { branch: "|-", last: "`-", vert: "| ", space: "  " };

/// A flattened device with its precomputed NAME tree-prefix.
struct Row<'a> {
    dev: &'a Device,
    prefix: String,
}

/// Render the tree in the configured mode.
pub fn render(tree: &[Device], config: &Config) -> Result<()> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let r = match config.mode {
        OutputMode::Tree | OutputMode::List => render_table(&mut out, tree, config),
        OutputMode::Json => render_json(&mut out, tree, config),
        OutputMode::Pairs => render_pairs(&mut out, tree, config),
        OutputMode::Raw => render_raw(&mut out, tree, config),
    };
    // A broken pipe (e.g. `| head`) is not an error.
    match r {
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(crate::error::LsblkError::System(format!("write error: {e}"))),
        Ok(()) => Ok(()),
    }
}

/// Depth-first flatten, computing the NAME prefix for tree mode (empty for list
/// mode, which is otherwise identical).
fn flatten<'a>(tree: &'a [Device], config: &Config) -> Vec<Row<'a>> {
    let glyphs = if config.ascii { &ASCII } else { &UNICODE };
    let tree_mode = config.mode == OutputMode::Tree;
    let mut rows = Vec::new();
    for dev in tree {
        walk(dev, "", true, true, glyphs, tree_mode, &mut rows);
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn walk<'a>(
    dev: &'a Device,
    parent_prefix: &str,
    is_last: bool,
    is_root: bool,
    glyphs: &Glyphs,
    tree_mode: bool,
    rows: &mut Vec<Row<'a>>,
) {
    let prefix = if !tree_mode || is_root {
        String::new()
    } else {
        format!("{parent_prefix}{}", if is_last { glyphs.last } else { glyphs.branch })
    };
    rows.push(Row { dev, prefix });

    let child_prefix = if !tree_mode || is_root {
        String::new()
    } else {
        format!("{parent_prefix}{}", if is_last { glyphs.space } else { glyphs.vert })
    };
    let n = dev.children.len();
    for (i, child) in dev.children.iter().enumerate() {
        walk(child, &child_prefix, i + 1 == n, false, glyphs, tree_mode, rows);
    }
}

/// The displayed cell text for a column (NAME gets the tree prefix; multi-value
/// MOUNTPOINTS collapses to a single line).
fn cell(row: &Row, col: Column, config: &Config) -> String {
    let raw = col.value(row.dev, config).unwrap_or_default();
    let raw = raw.replace('\n', ",");
    if col == config.tree_column {
        format!("{}{raw}", row.prefix)
    } else {
        raw
    }
}

fn render_table(out: &mut impl Write, tree: &[Device], config: &Config) -> std::io::Result<()> {
    let rows = flatten(tree, config);
    let cols = &config.columns;

    // Column widths: header vs widest cell (char count; box glyphs are width 1).
    let mut widths: Vec<usize> = cols.iter().map(|c| c.id().chars().count()).collect();
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|row| cols.iter().map(|c| cell(row, *c, config)).collect())
        .collect();
    for line in &cells {
        for (i, text) in line.iter().enumerate() {
            widths[i] = widths[i].max(text.chars().count());
        }
    }

    if !config.noheadings {
        let header: Vec<String> = cols.iter().map(|c| col_key(*c, config)).collect();
        write_row(out, &padded_row(&header, &widths, cols), config)?;
    }
    for line in &cells {
        write_row(out, &padded_row(line, &widths, cols), config)?;
    }
    Ok(())
}

/// Build one space-separated, padded row (no trailing newline). The final column
/// is not padded.
fn padded_row(line: &[String], widths: &[usize], cols: &[Column]) -> String {
    let last = line.len().saturating_sub(1);
    let mut out = String::new();
    for (i, text) in line.iter().enumerate() {
        let pad = " ".repeat(widths[i].saturating_sub(text.chars().count()));
        if cols[i].right_aligned() {
            out.push_str(&pad);
            out.push_str(text);
        } else if i == last {
            out.push_str(text);
        } else {
            out.push_str(text);
            out.push_str(&pad);
        }
        if i != last {
            out.push(' ');
        }
    }
    out
}

/// Write a built row, truncated to `-w NUM` columns when set.
fn write_row(out: &mut impl Write, row: &str, config: &Config) -> std::io::Result<()> {
    match config.width {
        Some(w) if row.chars().count() > w => {
            let truncated: String = row.chars().take(w).collect();
            writeln!(out, "{truncated}")
        }
        _ => writeln!(out, "{row}"),
    }
}

/// A column's output key, made shell-safe under `-y` (`MAJ:MIN` → `MAJ_MIN`).
fn col_key(col: Column, config: &Config) -> String {
    if config.shell {
        col.id()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
            .collect()
    } else {
        col.id().to_string()
    }
}

fn render_pairs(out: &mut impl Write, tree: &[Device], config: &Config) -> std::io::Result<()> {
    for row in flatten(tree, config) {
        let mut first = true;
        for col in &config.columns {
            let val = col.value(row.dev, config).unwrap_or_default().replace('\n', " ");
            if !first {
                out.write_all(b" ")?;
            }
            first = false;
            write!(out, "{}=\"{}\"", col_key(*col, config), escape_pair(&val))?;
        }
        out.write_all(b"\n")?;
    }
    Ok(())
}

fn render_raw(out: &mut impl Write, tree: &[Device], config: &Config) -> std::io::Result<()> {
    if !config.noheadings {
        let header: Vec<String> = config.columns.iter().map(|c| col_key(*c, config)).collect();
        writeln!(out, "{}", header.join(" "))?;
    }
    for row in flatten(tree, config) {
        let line: Vec<String> = config
            .columns
            .iter()
            .map(|c| {
                let v = c.value(row.dev, config).unwrap_or_default().replace('\n', " ");
                // Raw output is whitespace-separated with no quoting, so values
                // that could contain a space or control char are hex-escaped
                // (matching upstream) to keep fields parseable.
                if c.raw_escaped() { raw_escape(&v) } else { v }
            })
            .collect();
        writeln!(out, "{}", line.join(" "))?;
    }
    Ok(())
}

fn render_json(out: &mut impl Write, tree: &[Device], config: &Config) -> std::io::Result<()> {
    writeln!(out, "{{")?;
    writeln!(out, "   \"blockdevices\": [")?;
    let n = tree.len();
    for (i, dev) in tree.iter().enumerate() {
        json_device(out, dev, config, 6, i + 1 == n)?;
    }
    writeln!(out, "   ]")?;
    writeln!(out, "}}")
}

fn json_device(
    out: &mut impl Write,
    dev: &Device,
    config: &Config,
    indent: usize,
    last: bool,
) -> std::io::Result<()> {
    let pad = " ".repeat(indent);
    writeln!(out, "{pad}{{")?;
    let inner = " ".repeat(indent + 3);

    let n = config.columns.len();
    for (i, col) in config.columns.iter().enumerate() {
        let key = col.id().to_ascii_lowercase();
        let comma = if i + 1 == n && dev.children.is_empty() { "" } else { "," };
        if *col == Column::Mountpoints {
            // MOUNTPOINTS is an array in JSON; empty is `[]` (current upstream).
            let mps = &dev.mountpoints;
            let items: Vec<String> =
                mps.iter().map(|m| format!("\"{}\"", escape_json(m))).collect();
            writeln!(out, "{inner}\"{key}\": [{}]{comma}", items.join(", "))?;
        } else if col.is_boolean() {
            // Flag columns are bare JSON booleans, not quoted strings.
            let truthy = col.value(dev, config).as_deref() == Some("1");
            writeln!(out, "{inner}\"{key}\": {truthy}{comma}")?;
        } else {
            match col.value(dev, config) {
                Some(v) => writeln!(out, "{inner}\"{key}\": \"{}\"{comma}", escape_json(&v))?,
                None => writeln!(out, "{inner}\"{key}\": null{comma}")?,
            }
        }
    }

    if !dev.children.is_empty() {
        writeln!(out, "{inner}\"children\": [")?;
        let cn = dev.children.len();
        for (i, child) in dev.children.iter().enumerate() {
            json_device(out, child, config, indent + 6, i + 1 == cn)?;
        }
        writeln!(out, "{inner}]")?;
    }

    writeln!(out, "{pad}}}{}", if last { "" } else { "," })
}

fn escape_pair(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Hex-escape characters that would make a raw (`-r`) field ambiguous: spaces,
/// control characters, and the backslash itself. Matches upstream's `\xNN`.
fn raw_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        let u = c as u32;
        // The unsafe characters are all single-byte ASCII; pass valid multibyte
        // UTF-8 through untouched.
        if c == ' ' || c == '\\' || u < 0x20 || u == 0x7f {
            out.push_str(&format!("\\x{u:02x}"));
        } else {
            out.push(c);
        }
    }
    out
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
