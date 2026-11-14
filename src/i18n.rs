//! Interface language: the supported languages and how the active one is
//! chosen at startup.
//!
//! The active language is resolved once in `main()` — never lazily inside the
//! `t!` lookup — so tests, which never run `main()`, always see the `en`
//! fallback. Resolution order: the `--lang` option, then the `language` key
//! in the config file, then the `LC_ALL`/`LC_MESSAGES`/`LANG` environment
//! variables, falling back to English.

use serde::{Deserialize, Serialize};

/// The built-in interface languages.
///
/// Serialized in kebab-case, so the config file accepts the same codes
/// (`"en"`, `"fr"`) as the `--lang` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Language {
    En,
    Fr,
}

impl Language {
    /// Every supported language.
    pub const ALL: &[Language] = &[Language::En, Language::Fr];

    /// The locale code: the name of the language's file in `locales/`
    /// (`locales/<code>.toml`) and the code passed to `rust_i18n::set_locale`.
    pub fn code(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Fr => "fr",
        }
    }

    /// Parse a locale code or tag (`"fr"`, `"fr_FR.UTF-8"`) into a supported
    /// language, ignoring case and any territory/encoding suffix.
    pub fn from_tag(tag: &str) -> Option<Language> {
        let code = tag.split(['_', '-', '.']).next()?.to_lowercase();
        Self::ALL.iter().copied().find(|l| l.code() == code)
    }
}

/// The `--lang` value pre-scanned from the raw arguments, before clap runs,
/// so that `--help` output can already be localized.
pub fn prescan_lang() -> Option<Language> {
    prescan_from(std::env::args_os().skip(1))
}

/// Scan raw arguments for `--lang <value>` / `--lang=<value>`. Stops at `--`
/// (everything after it is positional) and accepts the same locale tags as
/// the clap value parser.
fn prescan_from(args: impl Iterator<Item = std::ffi::OsString>) -> Option<Language> {
    let mut args = args;
    while let Some(arg) = args.next() {
        let Some(s) = arg.to_str() else { continue };
        if s == "--" {
            break;
        }
        if let Some(value) = s.strip_prefix("--lang=") {
            return Language::from_tag(value);
        }
        if s == "--lang"
            && let Some(value) = args.next().and_then(|v| v.to_str().map(str::to_owned))
        {
            return Language::from_tag(&value);
        }
    }
    None
}

/// The `language` key of the config file, when present and valid.
pub fn config_language() -> Option<Language> {
    #[derive(Deserialize)]
    struct LangOnly {
        language: Option<Language>,
    }
    let path = crate::config_path::config_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    toml::from_str::<LangOnly>(&text).ok()?.language
}

/// The language named by the first of `LC_ALL`, `LC_MESSAGES`, `LANG` that is
/// set and matches a supported language.
fn env_language() -> Option<Language> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|var| std::env::var(var).ok())
        .and_then(|value| Language::from_tag(&value))
}

/// Resolve the interface language: `--lang` wins, then the config file, then
/// the environment, falling back to English. The config file language is
/// passed in (already read by the caller), so it is not read twice.
pub fn resolve(cli: Option<Language>, config: Option<Language>) -> Language {
    cli.or(config).or_else(env_language).unwrap_or(Language::En)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_tags_parse_to_supported_languages() {
        assert_eq!(Some(Language::En), Language::from_tag("en"));
        assert_eq!(Some(Language::En), Language::from_tag("en_US.UTF-8"));
        assert_eq!(Some(Language::Fr), Language::from_tag("fr"));
        assert_eq!(Some(Language::Fr), Language::from_tag("fr_FR.UTF-8"));
        assert_eq!(Some(Language::Fr), Language::from_tag("FR-ca"));
        assert_eq!(None, Language::from_tag("de_DE"));
        assert_eq!(None, Language::from_tag("C"));
        assert_eq!(None, Language::from_tag("POSIX"));
        assert_eq!(None, Language::from_tag(""));
    }

    #[test]
    fn language_names_match_the_config_and_cli_spellings() {
        // The config file serializes in kebab-case, and --lang accepts the
        // same codes plus locale tags
        #[derive(Serialize)]
        struct Lang {
            language: Language,
        }
        let text = toml::to_string_pretty(&Lang {
            language: Language::Fr,
        })
        .unwrap();
        assert!(text.contains("language = \"fr\""), "unexpected: {text}");
        assert_eq!(Some(Language::Fr), Language::from_tag("fr"));
        assert_eq!(Some(Language::Fr), Language::from_tag("fr-FR"));
    }

    #[test]
    fn resolve_prefers_cli_then_config() {
        // Cases that do not depend on the test machine's environment
        assert_eq!(
            Language::En,
            resolve(Some(Language::En), Some(Language::Fr))
        );
        assert_eq!(Language::Fr, resolve(None, Some(Language::Fr)));
        assert_eq!(Language::Fr, resolve(Some(Language::Fr), None));
    }

    #[test]
    fn prescan_reads_lang_and_stops_at_double_dash() {
        use std::ffi::OsString;
        fn args<'a>(v: &'a [&'a str]) -> impl Iterator<Item = OsString> + 'a {
            v.iter().map(|s| OsString::from(*s))
        }
        assert_eq!(None, prescan_from(args(&[])));
        assert_eq!(Some(Language::Fr), prescan_from(args(&["--lang", "fr"])));
        assert_eq!(
            Some(Language::Fr),
            prescan_from(args(&["--lang=fr_FR.UTF-8"]))
        );
        assert_eq!(Some(Language::En), prescan_from(args(&["--lang=en"])));
        assert_eq!(None, prescan_from(args(&["--lang="])));
        assert_eq!(None, prescan_from(args(&["--lang"])));
        // After `--`, --lang is a positional file name, not an option
        assert_eq!(None, prescan_from(args(&["--", "--lang", "fr"])));
        assert_eq!(
            Some(Language::Fr),
            prescan_from(args(&["--lang", "fr", "--", "--lang"]))
        );
    }

    /// Flatten a parsed TOML catalog into its dotted keys.
    fn catalog_keys(text: &str) -> std::collections::BTreeSet<String> {
        fn flatten(
            value: &toml::Value,
            prefix: &str,
            out: &mut std::collections::BTreeSet<String>,
        ) {
            match value {
                toml::Value::Table(map) => {
                    for (k, v) in map {
                        flatten(v, &format!("{prefix}.{k}"), out);
                    }
                }
                _ => {
                    out.insert(prefix.trim_start_matches('.').to_string());
                }
            }
        }
        let mut out = std::collections::BTreeSet::new();
        flatten(&toml::from_str::<toml::Value>(text).unwrap(), "", &mut out);
        out
    }

    #[test]
    fn catalogs_answer_in_the_requested_locale() {
        use rust_i18n::t;
        assert_eq!("Stopped", t!("status.stopped", locale = "en"));
        assert_eq!("Arrêté", t!("status.stopped", locale = "fr"));
        assert_eq!("Quit", t!("menu.quit", locale = "en"));
        assert_eq!("Quitter", t!("menu.quit", locale = "fr"));
        // Unknown keys fall back to the key itself
        let missing = "no.such.key";
        assert_eq!(missing, t!(missing, locale = "fr"));
    }

    #[test]
    fn every_language_catalog_has_exactly_the_english_keys() {
        let en = catalog_keys(include_str!("../locales/en.toml"));
        for lang in Language::ALL {
            // Tests run with the crate root as working directory
            let text = std::fs::read_to_string(format!("locales/{}.toml", lang.code())).unwrap();
            let other = catalog_keys(&text);
            assert_eq!(
                en,
                other,
                "locales/{}.toml must define exactly the keys of locales/en.toml",
                lang.code()
            );
        }
    }

    /// Every translation-key literal used in `src/` — through `t!` or `ts!`,
    /// on one line or several — must exist in the English catalog (tests run
    /// with the `en` fallback).
    #[test]
    fn every_translation_key_used_in_code_exists_in_the_catalog() {
        let en = catalog_keys(include_str!("../locales/en.toml"));

        fn walk(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, files);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    files.push(path);
                }
            }
        }
        let mut files = Vec::new();
        walk(std::path::Path::new("src"), &mut files);

        let mut missing = Vec::new();
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            let bytes = text.as_bytes();
            let mut i = 0;
            while let Some(pos) = text[i..].find("!(") {
                let bang = i + pos;
                // The macro name is the identifier right before `!`
                let mut name_start = bang;
                while name_start > 0
                    && (bytes[name_start - 1].is_ascii_alphanumeric()
                        || bytes[name_start - 1] == b'_')
                {
                    name_start -= 1;
                }
                let name = &text[name_start..bang];
                // Skip qualified paths like `rust_i18n::t!` and other macros
                let qualified = name_start > 0
                    && (bytes[name_start - 1] == b':' || bytes[name_start - 1] == b'.');
                if (name != "t" && name != "ts") || qualified {
                    i = bang + 2;
                    continue;
                }
                // Skip whitespace between `!(` and the opening quote
                let mut j = bang + 2;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j >= bytes.len() || bytes[j] != b'"' {
                    i = bang + 2;
                    continue;
                }
                let key_start = j + 1;
                let Some(key_end) = text[key_start..].find('"') else {
                    break;
                };
                let key = &text[key_start..key_start + key_end];
                if !en.contains(key) {
                    missing.push(format!("{}: {key}", file.display()));
                }
                i = key_start + key_end + 1;
            }
        }
        assert!(
            missing.is_empty(),
            "keys missing from locales/en.toml:\n{}",
            missing.join("\n")
        );
    }
}
