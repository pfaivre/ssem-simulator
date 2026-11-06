//! Persistent interface preferences.
//!
//! The view modes — color theme, flow view, heatmap, comments toggle, bit
//! representation — are saved to `ssem-simulator/config.toml` in the
//! platform's config directory (`$XDG_CONFIG_HOME`, or `$HOME/.config` when
//! unset, on Linux and macOS; `%APPDATA%`, or `%USERPROFILE%\AppData\Roaming`
//! when unset, on Windows) when the application closes, and restored on the
//! next start. There is no settings menu: the file is written once, on exit.
//!
//! A missing file means defaults; an unreadable or invalid file is reported
//! as a warning and ignored. Every key is optional in the file, so partial
//! or hand-edited files keep working, and unknown keys are ignored.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::layout::{FlowMode, HeatmapMode};
use super::palette::Theme;
use crate::i18n::Language;

/// The persisted view modes. The defaults match a fresh start with no
/// config file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct Config {
    pub theme: Theme,
    pub flow: FlowMode,
    pub heatmap: HeatmapMode,
    pub comments: bool,
    /// Index into `BIT_REPRESENTATIONS` (cycled with the D key).
    pub bit_representation: usize,
    /// Interface language. Read at startup (before the TUI) to pick the
    /// locale; `--lang` overrides it for the session without rewriting it.
    pub language: Language,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: Theme::Dark,
            flow: FlowMode::Control,
            heatmap: HeatmapMode::Off,
            comments: true,
            bit_representation: 0,
            language: Language::En,
        }
    }
}

impl Config {
    /// Write the config as TOML, creating the parent directory if needed.
    pub(super) fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, text)
    }
}

/// The config file location, or None when no config directory can be
/// determined from the environment (persistence is then disabled).
pub(crate) fn config_path() -> Option<PathBuf> {
    config_path_from_env(
        cfg!(windows),
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
        std::env::var_os("APPDATA"),
        std::env::var_os("USERPROFILE"),
    )
}

/// Resolve the config file location from environment values. `windows`
/// selects the platform convention: `%APPDATA%`, falling back to
/// `%USERPROFILE%\AppData\Roaming`, on Windows; `$HOME/.config` elsewhere.
/// `XDG_CONFIG_HOME` wins on every platform when set and non-empty.
fn config_path_from_env(
    windows: bool,
    xdg_config_home: Option<OsString>,
    home: Option<OsString>,
    appdata: Option<OsString>,
    userprofile: Option<OsString>,
) -> Option<PathBuf> {
    let base = xdg_config_home
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            if windows {
                appdata
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
                    .or_else(|| {
                        userprofile
                            .filter(|v| !v.is_empty())
                            .map(|p| PathBuf::from(p).join("AppData").join("Roaming"))
                    })
            } else {
                home.filter(|v| !v.is_empty())
                    .map(|h| PathBuf::from(h).join(".config"))
            }
        })?;
    Some(base.join("ssem-simulator").join("config.toml"))
}

/// A loaded config file: the values to start with, and the path to save
/// back to.
pub(super) struct ConfigStore {
    path: Option<PathBuf>,
    values: Config,
}

impl ConfigStore {
    /// Load the config file from its default location. Returns the store
    /// plus an optional warning (unreadable or invalid file) to show before
    /// the TUI starts.
    pub(super) fn load() -> (Self, Option<String>) {
        Self::load_from(config_path())
    }

    /// Load the config from the given path (None disables persistence).
    fn load_from(path: Option<PathBuf>) -> (Self, Option<String>) {
        let Some(path) = path else {
            return (
                Self {
                    path: None,
                    values: Config::default(),
                },
                None,
            );
        };
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return (
                    Self {
                        path: Some(path),
                        values: Config::default(),
                    },
                    None,
                );
            }
            Err(e) => {
                let warning = ts!(
                    "warn.config_read",
                    path = path.display().to_string(),
                    error = e.to_string()
                );
                return (
                    Self {
                        path: Some(path),
                        values: Config::default(),
                    },
                    Some(warning),
                );
            }
        };
        match toml::from_str::<Config>(&text) {
            Ok(values) => (
                Self {
                    path: Some(path),
                    values,
                },
                None,
            ),
            Err(e) => {
                let warning = ts!(
                    "warn.config_invalid",
                    path = path.display().to_string(),
                    error = e.to_string()
                );
                (
                    Self {
                        path: Some(path),
                        values: Config::default(),
                    },
                    Some(warning),
                )
            }
        }
    }

    /// Split into the values to start the app with and the path to save to.
    pub(super) fn into_parts(self) -> (Config, Option<PathBuf>) {
        (self.values, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temporary file path for a test.
    fn temp_config_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ssem-simulator-test-{}-{name}", std::process::id()));
        dir.join("config.toml")
    }

    #[test]
    fn config_round_trips_through_toml() {
        let path = temp_config_path("roundtrip");
        let config = Config {
            theme: Theme::CatpuccinMacchiato,
            flow: FlowMode::Data,
            heatmap: HeatmapMode::Rw,
            comments: false,
            bit_representation: 3,
            language: Language::Fr,
        };
        config.save(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let parsed: Config = toml::from_str(&text).unwrap();
        assert_eq!(config, parsed);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn missing_keys_keep_their_defaults() {
        let parsed: Config = toml::from_str("theme = \"light\"\n").unwrap();
        assert_eq!(Theme::Light, parsed.theme);
        assert_eq!(FlowMode::Control, parsed.flow);
        assert_eq!(HeatmapMode::Off, parsed.heatmap);
        assert!(parsed.comments);
        assert_eq!(0, parsed.bit_representation);
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let parsed: Config = toml::from_str("future_option = 42\nflow = \"off\"\n").unwrap();
        assert_eq!(FlowMode::Off, parsed.flow);
    }

    #[test]
    fn theme_names_match_the_cli_value_names() {
        // clap's ValueEnum uses kebab-case, so the config file accepts the
        // same spellings as --theme
        let parsed: Config = toml::from_str("theme = \"catpuccin-macchiato\"\n").unwrap();
        assert_eq!(Theme::CatpuccinMacchiato, parsed.theme);
        let text = toml::to_string_pretty(&Config::default()).unwrap();
        assert!(text.contains("theme = \"dark\""), "unexpected: {text}");
        assert!(text.contains("flow = \"control\""), "unexpected: {text}");
        assert!(text.contains("heatmap = \"off\""), "unexpected: {text}");
        assert!(text.contains("comments = true"), "unexpected: {text}");
        assert!(
            text.contains("bit_representation = 0"),
            "unexpected: {text}"
        );
        assert!(text.contains("language = \"en\""), "unexpected: {text}");
    }

    #[test]
    fn out_of_range_bit_representation_is_clamped_on_load() {
        let parsed: Config = toml::from_str("bit_representation = 99\n").unwrap();
        let app = super::super::app::App::from_parts(parsed, None, None);
        assert_eq!(
            super::super::app::BIT_REPRESENTATIONS.len() - 1,
            app.bit_representation
        );
    }

    #[test]
    fn missing_file_loads_defaults_without_warning() {
        let path = temp_config_path("missing");
        let (store, warning) = ConfigStore::load_from(Some(path.clone()));
        assert!(warning.is_none());
        let (values, saved_path) = store.into_parts();
        assert_eq!(Config::default(), values);
        assert_eq!(Some(path), saved_path);
    }

    #[test]
    fn invalid_file_loads_defaults_with_warning() {
        let path = temp_config_path("invalid");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "theme = \"not-a-theme\"\n").unwrap();
        let (store, warning) = ConfigStore::load_from(Some(path.clone()));
        assert!(warning.is_some());
        let (values, _) = store.into_parts();
        assert_eq!(Config::default(), values);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn no_path_disables_persistence() {
        let (store, warning) = ConfigStore::load_from(None);
        assert!(warning.is_none());
        let (values, path) = store.into_parts();
        assert_eq!(Config::default(), values);
        assert_eq!(None, path);
    }

    #[test]
    fn config_path_prefers_xdg_config_home_on_any_platform() {
        for windows in [false, true] {
            let path = config_path_from_env(
                windows,
                Some(OsString::from("/xdg")),
                Some(OsString::from("/home/u")),
                Some(OsString::from("C:\\Users\\u\\AppData\\Roaming")),
                Some(OsString::from("C:\\Users\\u")),
            )
            .unwrap();
            assert_eq!(
                PathBuf::from("/xdg")
                    .join("ssem-simulator")
                    .join("config.toml"),
                path
            );
        }
    }

    #[test]
    fn config_path_uses_home_dot_config_off_windows() {
        let path =
            config_path_from_env(false, None, Some(OsString::from("/home/u")), None, None).unwrap();
        assert_eq!(
            PathBuf::from("/home/u")
                .join(".config")
                .join("ssem-simulator")
                .join("config.toml"),
            path
        );
    }

    #[test]
    fn config_path_uses_appdata_on_windows_and_ignores_home() {
        // HOME is often set to an MSYS-style path when the Windows binary
        // runs from Git Bash; that path is meaningless to the native app
        let path = config_path_from_env(
            true,
            None,
            Some(OsString::from("/c/Users/u")),
            Some(OsString::from("C:\\Users\\u\\AppData\\Roaming")),
            Some(OsString::from("C:\\Users\\u")),
        )
        .unwrap();
        assert_eq!(
            PathBuf::from("C:\\Users\\u\\AppData\\Roaming")
                .join("ssem-simulator")
                .join("config.toml"),
            path
        );
    }

    #[test]
    fn config_path_falls_back_to_userprofile_appdata_on_windows() {
        let path =
            config_path_from_env(true, None, None, None, Some(OsString::from("C:\\Users\\u")))
                .unwrap();
        assert_eq!(
            PathBuf::from("C:\\Users\\u")
                .join("AppData")
                .join("Roaming")
                .join("ssem-simulator")
                .join("config.toml"),
            path
        );
    }

    #[test]
    fn config_path_ignores_empty_values_and_is_none_without_any() {
        assert!(
            config_path_from_env(
                false,
                Some(OsString::from("")),
                Some(OsString::from("")),
                None,
                None
            )
            .is_none()
        );
        assert!(
            config_path_from_env(
                true,
                None,
                None,
                Some(OsString::from("")),
                Some(OsString::from(""))
            )
            .is_none()
        );
        assert!(config_path_from_env(false, None, None, None, None).is_none());
        assert!(config_path_from_env(true, None, None, None, None).is_none());
    }
}
