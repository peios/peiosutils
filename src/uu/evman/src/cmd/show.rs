// `evman <name>` — the lookup — and `evman -k <term>...`.

use std::collections::BTreeMap;

use clap::ArgMatches;

use crate::catalogue::{Catalogue, Lookup};
use crate::corpus;
use crate::error::{Error, Result};
use crate::lint::{self, Finding, Rule};
use crate::markdown::Style;
use crate::pager;
use crate::render;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let width = term_width();
    let style = Style::new(pager::color_enabled());

    // `evman -k <term>...` — apropos / keyword search.
    if let Some(terms) = matches.get_many::<String>("apropos") {
        let terms: Vec<String> = terms.cloned().collect();
        let cat = load()?;
        let hits = cat.apropos(&terms);
        if hits.is_empty() {
            return Err(Error::NotFound(format!("-k {}", terms.join(" "))));
        }
        pager::emit(&render::apropos(&hits, width, style));
        return Ok(());
    }

    let Some(name) = matches.get_one::<String>("name") else {
        return Err(Error::Usage(
            "a name is required (try `evman <event-type | field>`, or `evman -k <term>`)"
                .to_string(),
        ));
    };
    let name = name.trim_end_matches('.');
    let cat = load()?;

    let output = match cat.lookup(name) {
        Some(Lookup::Event(e)) => render::event(&cat, e, width, style),
        Some(Lookup::Field(f)) => render::field(&cat, f, None, width, style),
        Some(Lookup::Variant { field, qualifier }) => {
            let note = format!(
                "{name} is the {qualifier} variant of {}, which any field may take without a definition of its own (PGSS §6.4).",
                field.name()
            );
            render::field(&cat, field, Some(&note), width, style)
        }
        Some(Lookup::Attribute { attribute, index }) => {
            let note = format!(
                "{name} is the standard attribute {attribute} of {}, which any thing or domain may take without a definition of its own (PGSS §6.4).",
                index.prefix
            );
            render::index(&cat, &index, Some(&note), width, style)
        }
        Some(Lookup::Index(index)) => render::index(&cat, &index, None, width, style),
        None => return Err(Error::NotFound(name.to_string())),
    };

    pager::emit(&output);
    Ok(())
}

/// Read and check the corpus. §6.10 says a reader must not treat a fragment
/// that breaks rule 1, 2 or 3 as defining anything; the manual still shows
/// what it can, and says on stderr which fragments the reader should not
/// trust until `evman lint` is clean.
fn load() -> Result<Catalogue> {
    let sources = corpus::read(&corpus::dir())?;
    let (cat, findings) = lint::check(&sources);
    for warning in warnings(&findings) {
        eprintln!("evman: {warning}");
    }
    Ok(cat)
}

/// One line per fragment that breaks rule 1, 2 or 3, naming the rules.
fn warnings(findings: &[Finding]) -> Vec<String> {
    let mut broken: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for f in findings {
        if let Rule::Numbered(n @ 1..=3) = f.rule {
            let rules = broken.entry(f.file_name()).or_default();
            if !rules.contains(&n) {
                rules.push(n);
            }
        }
    }
    broken
        .into_iter()
        .map(|(file, mut rules)| {
            rules.sort_unstable();
            let rules: Vec<String> = rules.iter().map(u8::to_string).collect();
            let plural = if rules.len() == 1 { "" } else { "s" };
            format!(
                "{file} breaks rule{plural} {} of PGSS §6.10, so its definitions are not to be trusted (see `evman lint`)",
                rules.join(", ")
            )
        })
        .collect()
}

fn term_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse::<usize>().ok())
        .filter(|w| *w >= 20)
        .unwrap_or(80)
        .saturating_sub(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn finding(file: &str, rule: Rule) -> Finding {
        Finding {
            file: PathBuf::from(file),
            line: 1,
            rule,
            message: String::new(),
        }
    }

    #[test]
    fn warnings_name_each_fragment_once() {
        let got = warnings(&[
            finding("/x/kacs.evman", Rule::Numbered(3)),
            finding("/x/kacs.evman", Rule::Numbered(1)),
            finding("/x/kacs.evman", Rule::Numbered(1)),
            finding("/x/lcs.evman", Rule::Numbered(2)),
            finding("/x/lcs.evman", Rule::Numbered(5)),
            finding("/x/ntfe.evman", Rule::Format),
        ]);
        assert_eq!(got.len(), 2);
        assert!(got[0].starts_with("kacs.evman breaks rules 1, 3 of PGSS §6.10"));
        assert!(got[1].starts_with("lcs.evman breaks rule 2 of PGSS §6.10"));
    }
}
