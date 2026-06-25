// Output layout. Commands assemble both a human view (`Lines`) and a JSON
// value; the dispatcher prints one based on `Settings::json`.

/// A line in the human-readable layout.
#[derive(Debug, Clone)]
pub enum Line {
    /// Section header.
    Section(String),
    /// `key: value`, aligned within a block.
    Kv(String, String),
    /// Free-form line (no alignment).
    Plain(String),
    /// Indented detail line.
    Detail(String),
    /// Blank line.
    Blank,
}

/// A buffer of human-readable lines.
#[derive(Default, Debug, Clone)]
pub struct Lines(pub Vec<Line>);

impl Lines {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn section(&mut self, s: impl Into<String>) {
        self.0.push(Line::Section(s.into()));
    }
    pub fn kv(&mut self, k: impl Into<String>, v: impl Into<String>) {
        self.0.push(Line::Kv(k.into(), v.into()));
    }
    pub fn plain(&mut self, s: impl Into<String>) {
        self.0.push(Line::Plain(s.into()));
    }
    pub fn detail(&mut self, s: impl Into<String>) {
        self.0.push(Line::Detail(s.into()));
    }
    pub fn blank(&mut self) {
        self.0.push(Line::Blank);
    }

    /// Print to stdout, aligning the `Kv` key column.
    pub fn print(&self) {
        let max_key = self
            .0
            .iter()
            .filter_map(|l| match l {
                Line::Kv(k, _) => Some(k.len()),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        for line in &self.0 {
            match line {
                Line::Section(s) => println!("\n[{s}]"),
                Line::Kv(k, v) => println!("  {k:<width$}  {v}", width = max_key),
                Line::Plain(s) => println!("{s}"),
                Line::Detail(s) => println!("      {s}"),
                Line::Blank => println!(),
            }
        }
    }
}

/// Generic command output: both shapes, dispatcher picks one.
pub struct CmdOutput {
    pub human: Lines,
    pub json: serde_json::Value,
}

impl CmdOutput {
    pub fn print(&self, json: bool) {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&self.json).unwrap_or_else(|_| "{}".into())
            );
        } else {
            self.human.print();
        }
    }
}
