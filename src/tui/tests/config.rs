//! Tests for the persistent view modes (the config file) and the
//! no-key-repeat behavior of the mode keys.

use super::super::config::Config;
use super::*;
use crate::i18n::Language;
use crate::tui::Theme;
use crossterm::event::KeyEventKind;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

/// A unique temporary config file path for a test.
fn temp_config_path(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!("ssem-simulator-test-{}-{name}", std::process::id()))
        .join("config.toml")
}

/// A key press event (as opposed to a repeat).
fn press(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A key-repeat event, as reported when a key is held down.
fn repeat(code: KeyCode) -> KeyEvent {
    KeyEvent::new_with_kind(code, KeyModifiers::NONE, KeyEventKind::Repeat)
}

#[test]
pub(super) fn from_parts_restores_the_saved_modes() {
    let config = Config {
        theme: Theme::Monokai,
        flow: FlowMode::Data,
        heatmap: HeatmapMode::Exec,
        comments: false,
        bit_representation: 4,
        language: Language::Fr,
    };
    let app = App::from_parts(config, None, None);
    assert_eq!(Theme::Monokai, app.theme);
    assert_eq!(FlowMode::Data, app.flow);
    assert_eq!(HeatmapMode::Exec, app.heatmap);
    assert!(!app.show_comments);
    assert_eq!(4, app.bit_representation);
}

#[test]
pub(super) fn cli_theme_overrides_the_saved_one() {
    let config = Config {
        theme: Theme::Monokai,
        ..Config::default()
    };
    let app = App::from_parts(config, Some(Theme::Notebook), None);
    assert_eq!(Theme::Notebook, app.theme);
}

#[test]
pub(super) fn modes_are_saved_once_on_close_not_on_key_press() {
    let path = temp_config_path("save");
    let mut sim = Simulator::new();
    let mut app = App::from_parts(Config::default(), None, Some(path.clone()));

    app.handle_key(press(KeyCode::Char('m')), &mut sim);
    app.handle_key(press(KeyCode::Char('c')), &mut sim);
    app.handle_key(press(KeyCode::Char('t')), &mut sim);
    app.handle_key(press(KeyCode::Char('f')), &mut sim);
    app.handle_key(press(KeyCode::Char('d')), &mut sim);
    assert_eq!(HeatmapMode::Exec, app.heatmap);
    assert!(!app.show_comments);
    assert_eq!(Theme::Monokai, app.theme);
    assert_eq!(FlowMode::Data, app.flow);
    assert_eq!(1, app.bit_representation);
    assert!(!path.exists(), "key presses must not write the config file");

    // What the runner does when the user quits
    app.save_config();
    let saved: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(HeatmapMode::Exec, saved.heatmap);
    assert!(!saved.comments);
    assert_eq!(
        Theme::Monokai,
        saved.theme,
        "base16-ocean cycles to monokai"
    );
    assert_eq!(FlowMode::Data, saved.flow, "control cycles to data");
    assert_eq!(1, saved.bit_representation, "D cycles 0 -> 1");
    assert!(app.config_error.is_none());

    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
pub(super) fn startup_language_is_persisted_over_the_saved_one() {
    // A config file saved with language = "en" (e.g. by an older version)
    // while the app started in French from the environment: closing the
    // app must save "fr", not the stale "en"
    let path = temp_config_path("language");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "language = \"en\"\n").unwrap();
    let (store, warning) = super::super::config::ConfigStore::load_from(Some(path.clone()));
    assert!(warning.is_none());

    let (config, _) = super::super::runner::startup_config(store, Language::Fr);
    assert_eq!(Language::Fr, config.language);

    let mut app = App::from_parts(config, None, Some(path.clone()));
    app.save_config();
    let saved: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(Language::Fr, saved.language);

    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
pub(super) fn save_failure_is_reported_in_config_error() {
    // A path whose parent is a regular file: the directory cannot be created
    let blocker = temp_config_path("blocked");
    fs::create_dir_all(blocker.parent().unwrap()).unwrap();
    fs::write(&blocker, "not a directory").unwrap();
    let path = blocker.join("config.toml");

    let mut app = App::from_parts(Config::default(), None, Some(path));
    app.save_config();
    assert!(app.config_error.is_some());

    fs::remove_dir_all(blocker.parent().unwrap()).unwrap();
}

#[test]
pub(super) fn mode_keys_do_not_repeat_when_held() {
    let mut sim = Simulator::new();
    let mut app = App::new();

    for code in [
        KeyCode::Char('m'),
        KeyCode::Char('f'),
        KeyCode::Char('c'),
        KeyCode::Char('t'),
    ] {
        let before = (app.heatmap, app.flow, app.show_comments, app.theme);
        app.handle_key(repeat(code), &mut sim);
        assert_eq!(
            before,
            (app.heatmap, app.flow, app.show_comments, app.theme),
            "repeat of {code:?} must not change a view mode"
        );
    }
}

#[test]
pub(super) fn mode_keys_swallow_os_repeat_but_not_human_double_press() {
    let mut sim = Simulator::new();
    let mut app = App::from_parts(Config::default(), None, None);
    app.mode_key_guard = Duration::from_millis(100);

    app.handle_key(press(KeyCode::Char('m')), &mut sim);
    assert_eq!(HeatmapMode::Exec, app.heatmap);

    // OS key-repeat: same key again within the guard window -> ignored
    app.handle_key(press(KeyCode::Char('m')), &mut sim);
    app.handle_key(press(KeyCode::Char('m')), &mut sim);
    assert_eq!(HeatmapMode::Exec, app.heatmap);

    // A human double-press after the window -> accepted
    std::thread::sleep(Duration::from_millis(150));
    app.handle_key(press(KeyCode::Char('m')), &mut sim);
    assert_eq!(HeatmapMode::Rw, app.heatmap);

    // Other mode keys are guarded too, independently per key
    app.handle_key(press(KeyCode::Char('t')), &mut sim);
    app.handle_key(press(KeyCode::Char('t')), &mut sim);
    assert_eq!(Theme::Monokai, app.theme, "repeated T press must not cycle");
}

#[test]
pub(super) fn scrub_keys_still_act_on_repeats() {
    let mut sim = looping_sim(10);
    let mut app = App::new();
    let head = sim.cycle_count;
    app.handle_key(repeat(KeyCode::Left), &mut sim);
    assert_eq!(head - 1, sim.cycle_count);
}
