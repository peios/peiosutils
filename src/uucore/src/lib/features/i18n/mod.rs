// This file is part of the uutils coreutils package.
//
// For the full copyright and license information, please view the LICENSE
// file that was distributed with this source code.

use std::sync::OnceLock;

use icu_locale::{Locale, locale};

#[cfg(feature = "i18n-charmap")]
pub mod charmap;
#[cfg(feature = "i18n-collator")]
pub mod collator;
#[cfg(feature = "i18n-datetime")]
pub mod datetime;
#[cfg(feature = "i18n-decimal")]
pub mod decimal;

/// The encoding specified by the locale, if specified
/// Currently only supports ASCII and UTF-8 for the sake of simplicity.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UEncoding {
    Ascii,
    Utf8,
}

// Use "und" (undefined) as the marker for C/POSIX locale
// This ensures real locales like "en-US" won't match
const DEFAULT_LOCALE: Locale = locale!("und");

/// The first of LC_ALL, `locale_name` and LANG that has a value. An empty
/// variable counts as unset, as POSIX and glibc have it: a session that
/// exports `LC_ALL=""` still uses its `LANG`.
fn locale_var_from(locale_name: &str, get: impl Fn(&str) -> Option<String>) -> Option<String> {
    ["LC_ALL", locale_name, "LANG"]
        .iter()
        .find_map(|&key| get(key).filter(|value| !value.is_empty()))
}

/// Look at 3 environment variables in the following order
///
/// 1. LC_ALL
/// 2. `locale_name`
/// 3. LANG
///
/// Or fallback on Posix locale, with ASCII encoding.
pub fn get_locale_from_env(locale_name: &str) -> (Locale, UEncoding) {
    let locale_var = locale_var_from(locale_name, |key| std::env::var(key).ok());

    if let Some(locale_var_str) = locale_var {
        let mut split = locale_var_str.split(&['.', '@']);

        if let Some(simple) = split.next() {
            // Handle explicit C and POSIX locales - these should always use byte comparison
            if simple == "C" || simple == "POSIX" {
                return (DEFAULT_LOCALE, UEncoding::Ascii);
            }

            // Naively convert the locale name to BCP47 tag format.
            //
            // See https://en.wikipedia.org/wiki/IETF_language_tag
            let bcp47 = simple.replace('_', "-");
            let locale = Locale::try_from_str(&bcp47).unwrap_or(DEFAULT_LOCALE);

            // If locale parsing failed, parse the encoding part of the
            // locale. Treat the special case of the given locale being "C"
            // which becomes the default locale.
            let encoding = if (locale != DEFAULT_LOCALE || bcp47 == "C")
                && split.next().is_some_and(|enc| {
                    let lower = enc.to_lowercase();
                    lower == "utf-8" || lower == "utf8"
                }) {
                UEncoding::Utf8
            } else {
                UEncoding::Ascii
            };
            return (locale, encoding);
        }
    }
    // Default POSIX locale representing LC_ALL=C
    (DEFAULT_LOCALE, UEncoding::Ascii)
}

/// Get the collating locale from the environment
pub fn get_collating_locale() -> &'static (Locale, UEncoding) {
    static COLLATING_LOCALE: OnceLock<(Locale, UEncoding)> = OnceLock::new();

    COLLATING_LOCALE.get_or_init(|| get_locale_from_env("LC_COLLATE"))
}

/// Get the numeric locale from the environment
pub fn get_numeric_locale() -> &'static (Locale, UEncoding) {
    static NUMERIC_LOCALE: OnceLock<(Locale, UEncoding)> = OnceLock::new();

    NUMERIC_LOCALE.get_or_init(|| get_locale_from_env("LC_NUMERIC"))
}

/// Return the encoding deduced from the locale environment variable.
pub fn get_locale_encoding() -> UEncoding {
    get_collating_locale().1
}

#[cfg(test)]
mod tests {
    use super::locale_var_from;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |key| map.get(key).cloned()
    }

    #[test]
    fn an_empty_lc_all_falls_through_to_lang() {
        // What a GXWI session exports: every LC_* empty, LANG the person's.
        let get = env(&[("LC_ALL", ""), ("LC_TIME", ""), ("LANG", "de_DE.UTF-8")]);
        assert_eq!(
            locale_var_from("LC_TIME", get),
            Some("de_DE.UTF-8".to_string())
        );
    }

    #[test]
    fn a_set_variable_still_wins_in_order() {
        let get = env(&[
            ("LC_ALL", ""),
            ("LC_TIME", "fr_FR.UTF-8"),
            ("LANG", "de_DE.UTF-8"),
        ]);
        assert_eq!(
            locale_var_from("LC_TIME", get),
            Some("fr_FR.UTF-8".to_string())
        );
        let get = env(&[("LC_ALL", "C"), ("LANG", "de_DE.UTF-8")]);
        assert_eq!(locale_var_from("LC_TIME", get), Some("C".to_string()));
    }

    #[test]
    fn all_empty_is_no_locale() {
        let get = env(&[("LC_ALL", ""), ("LC_TIME", ""), ("LANG", "")]);
        assert_eq!(locale_var_from("LC_TIME", get), None);
    }
}
