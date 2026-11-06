//! Interface language: the supported languages and how the active one is
//! chosen at startup.
//!
//! The active language is resolved once in `main()` — never lazily inside the
//! `t!` lookup — so tests, which never run `main()`, always see the `en`
//! fallback. Resolution order: the `--lang` option, then the `language` key
//! in the config file, then the `LC_ALL`/`LC_MESSAGES`/`LANG` environment
//! variables, falling back to English.

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// The built-in interface languages.
///
/// Serialized in kebab-case, so the config file accepts the same spellings
/// as the `--lang` command-line option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
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
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let Some(s) = arg.to_str() else { continue };
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
fn config_language() -> Option<Language> {
    #[derive(Deserialize)]
    struct LangOnly {
        language: Option<Language>,
    }
    let path = crate::tui::config_path()?;
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
/// the environment, falling back to English.
pub fn resolve(cli: Option<Language>) -> Language {
    cli.or_else(config_language)
        .or_else(env_language)
        .unwrap_or(Language::En)
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
    fn language_names_match_the_cli_value_names() {
        // clap's ValueEnum uses kebab-case, so the config file accepts the
        // same spellings as --lang
        assert_eq!("en", Language::En.to_possible_value().unwrap().get_name());
        assert_eq!("fr", Language::Fr.to_possible_value().unwrap().get_name());
        #[derive(Serialize)]
        struct Lang {
            language: Language,
        }
        let text = toml::to_string_pretty(&Lang {
            language: Language::Fr,
        })
        .unwrap();
        assert!(text.contains("language = \"fr\""), "unexpected: {text}");
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
        assert_eq!("STOPPED", t!("status.stopped", locale = "en"));
        assert_eq!("ARRÊTÉ", t!("status.stopped", locale = "fr"));
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

    /// Every translation-key literal used in `src/` must exist in the
    /// English catalog (tests run with the `en` fallback).
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
            while let Some(pos) = text[i..].find("t!(\"") {
                let start = i + pos;
                // Skip qualified paths like `rust_i18n::t!(`: the char before
                // `t!` must not be alphanumeric or ':'
                if start > 0
                    && (bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b':')
                {
                    i = start + 1;
                    continue;
                }
                let key_start = start + 4;
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
