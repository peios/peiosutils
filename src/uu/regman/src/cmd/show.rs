// `regman <path> [value]` — the lookup.

use clap::ArgMatches;

use crate::corpus;
use crate::error::{Error, Result};
use crate::fold::fold;
use crate::markdown::Style;
use crate::pager;
use crate::query;
use crate::render;
use crate::scan;

pub fn run(matches: &ArgMatches) -> Result<()> {
    let width = term_width();
    let style = Style::new(pager::color_enabled());

    // `regman -k <term>...` — apropos / keyword search.
    if let Some(terms) = matches.get_many::<String>("apropos") {
        let terms: Vec<String> = terms.cloned().collect();
        let hits = scan::apropos(&corpus::dir(), &terms)?;
        if hits.is_empty() {
            return Err(Error::NotFound(format!("-k {}", terms.join(" "))));
        }
        pager::emit(&render::apropos(&hits, width, style));
        return Ok(());
    }

    let Some(path) = matches.get_one::<String>("path") else {
        return Err(Error::Usage(
            "a registry path is required (try `regman <path> [value]`, or `regman -k <term>`)"
                .to_string(),
        ));
    };
    let path = normalize(path);

    let output = if let Some(value) = matches.get_one::<String>("value") {
        let anchor = format!("{} {}", fold(&path), fold(value));
        let hits = query::resolve_exact_default(&anchor)?;
        if hits.is_empty() {
            return Err(Error::NotFound(format!("{path} {value}")));
        }
        render::exact(&hits, width, style)
    } else {
        let folded = fold(&path);
        let hits = query::resolve_key_default(&folded)?;
        if hits.is_empty() {
            return Err(Error::NotFound(path.clone()));
        }
        render::key(&hits, &path, width, style)
    };

    pager::emit(&output);
    Ok(())
}

/// Registry paths normalise forward slashes to backslashes; a trailing
/// separator is dropped (PSD-005 path rules).
fn normalize(path: &str) -> String {
    let p = path.replace('/', "\\");
    p.strip_suffix('\\').map(str::to_string).unwrap_or(p)
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

    #[test]
    fn normalize_slashes_and_trailing() {
        assert_eq!(normalize("Machine/System/KMES"), "Machine\\System\\KMES");
        assert_eq!(normalize("Machine\\System\\KMES\\"), "Machine\\System\\KMES");
        assert_eq!(normalize("Machine\\System\\KMES"), "Machine\\System\\KMES");
    }
}
