//! Location of the persisted config file (`ssem-simulator/config.toml` in
//! the platform's config directory). Kept out of `tui` so the startup
//! language resolution in `i18n` does not depend on the TUI module.

use std::ffi::OsString;
use std::path::PathBuf;

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
/// `%USERPROFILE%\\AppData\\Roaming`, on Windows; `$HOME/.config` elsewhere.
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

#[cfg(test)]
mod tests {
    use super::*;

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
