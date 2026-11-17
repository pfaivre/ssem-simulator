//! Tests for the first-run welcome pop-up, shown as a tour: shown once
//! (tracked by the `welcome_seen` config flag), navigated with
//! Enter/Space/Right (next, closing on the last step) and Left (back),
//! closed by Esc or a click outside from any step, each step pointing an
//! arrow at the element it presents, and the intro rendering the program
//! that was loaded (if any).

use super::super::config::Config;
use super::*;
use std::fs;
use std::path::PathBuf;

/// A unique temporary config file path for a test.
fn temp_config_path(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join(format!("ssem-simulator-test-{}-{name}", std::process::id()))
        .join("config.toml")
}

/// Send a key press.
fn press(app: &mut App, sim: &mut Simulator, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), sim);
}

/// The whole screen as one string, one character per cell.
fn screen_text(buffer: &Buffer, height: u16) -> String {
    (0..height).map(|row| row_text(buffer, row)).collect()
}

#[test]
pub(super) fn first_run_opens_the_welcome_popup_exactly_once() {
    let mut app = App::new();
    assert!(!app.welcome_seen);
    assert!(!app.welcome_popup.is_open());

    app.show_welcome_on_first_run();
    assert!(app.welcome_popup.is_open());
    assert!(app.welcome_seen, "showing it marks it as seen");

    app.welcome_popup.close();
    app.show_welcome_on_first_run();
    assert!(!app.welcome_popup.is_open(), "not shown a second time");
}

#[test]
pub(super) fn returning_user_does_not_get_the_welcome_popup() {
    let config = Config {
        welcome_seen: true,
        ..Config::default()
    };
    let mut app = App::from_parts(config, None, None);
    app.show_welcome_on_first_run();
    assert!(!app.welcome_popup.is_open());
}

#[test]
pub(super) fn welcome_tour_advances_to_the_last_step_and_enter_closes_it() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    assert_eq!(0, app.welcome_popup.step());
    for (code, expected) in [
        (KeyCode::Enter, 1),
        (KeyCode::Char(' '), 2),
        (KeyCode::Right, 3),
    ] {
        press(&mut app, &mut sim, code);
        assert!(app.welcome_popup.is_open(), "{code:?} advances, not closes");
        assert_eq!(expected, app.welcome_popup.step());
    }
    press(&mut app, &mut sim, KeyCode::Enter);
    assert!(
        !app.welcome_popup.is_open(),
        "Enter on the last step closes the tour"
    );
    // Reopening restarts the tour at the intro
    app.welcome_popup.open();
    assert_eq!(0, app.welcome_popup.step());
}

#[test]
pub(super) fn welcome_tour_left_goes_back() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    press(&mut app, &mut sim, KeyCode::Right);
    press(&mut app, &mut sim, KeyCode::Right);
    assert_eq!(2, app.welcome_popup.step());
    press(&mut app, &mut sim, KeyCode::Left);
    assert_eq!(1, app.welcome_popup.step());
    press(&mut app, &mut sim, KeyCode::Left);
    press(&mut app, &mut sim, KeyCode::Left);
    assert_eq!(0, app.welcome_popup.step(), "cannot go before the intro");
}

#[test]
pub(super) fn welcome_popup_esc_closes_from_any_step() {
    for advance in 0..4 {
        let mut app = App::new();
        let mut sim = Simulator::new();
        app.show_welcome_on_first_run();
        for _ in 0..advance {
            press(&mut app, &mut sim, KeyCode::Right);
        }
        press(&mut app, &mut sim, KeyCode::Esc);
        assert!(
            !app.welcome_popup.is_open(),
            "Esc should close the welcome pop-up from step {advance}"
        );
    }
}

#[test]
pub(super) fn welcome_popup_ignores_other_keys() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    press(&mut app, &mut sim, KeyCode::Char('x'));
    assert!(app.welcome_popup.is_open());
    assert_eq!(0, app.welcome_popup.step());
    // While it is open, main-keymap keys are routed to the pop-up, not acted on
    press(&mut app, &mut sim, KeyCode::Char('m'));
    assert_eq!(
        HeatmapMode::Off,
        app.heatmap,
        "M must not cycle the heatmap"
    );
    assert!(app.welcome_popup.is_open());
    assert_eq!(0, app.welcome_popup.step());
}

#[test]
pub(super) fn click_outside_the_welcome_popup_closes_it() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    let buffer = render(&mut app, &snapshot_with_history(0), 100, 40);
    // A cell outside the pop-up's last-drawn rect
    let (col, row) = (0..100u16)
        .flat_map(|c| (1..40u16).map(move |r| (c, r)))
        .find(|&(c, r)| !app.welcome_popup.contains(c, r))
        .expect("the pop-up does not cover the whole screen");
    let _ = buffer;
    app.handle_mouse(
        MouseEvent {
            column: col,
            row,
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
        },
        &mut sim,
    );
    assert!(!app.welcome_popup.is_open());
}

#[test]
pub(super) fn welcome_popup_shows_the_loaded_program() {
    let mut app = App::new();
    app.show_welcome_on_first_run();
    let snap = Snapshot {
        source_file: Some("samples/factorct.asm".to_string()),
        ..snapshot_with_history(0)
    };
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("Welcome to the SSEM simulator"), "{text}");
    assert!(
        text.contains("Program loaded: samples/factorct.asm"),
        "{text}"
    );
    assert!(
        text.contains("Space"),
        "quick-start keys are listed: {text}"
    );
    assert!(text.contains("Quick start"), "{text}");
}

#[test]
pub(super) fn welcome_popup_mentions_the_empty_store_without_a_file() {
    let mut app = App::new();
    app.show_welcome_on_first_run();
    let buffer = render(&mut app, &snapshot_with_history(0), 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("No program loaded"), "{text}");
}

#[test]
pub(super) fn welcome_tour_presents_each_element_with_an_arrow() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    let snap = snapshot_with_history(0);

    // Step 1: the intro, centered, no arrow
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("Welcome to the SSEM simulator"), "{text}");
    assert!(text.contains("1/4"), "{text}");
    assert!(
        !text.contains('\u{25b2}') && !text.contains('\u{25bc}'),
        "no arrow on the intro: {text}"
    );

    // Step 2: the store — side bars around the body, arrow up into it
    press(&mut app, &mut sim, KeyCode::Enter);
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("The store"), "{text}");
    assert!(text.contains("2/4"), "{text}");
    assert!(
        text.contains('\u{25b2}'),
        "the store step points up: {text}"
    );
    assert_eq!(
        "\u{2502}",
        buffer[(0, 5)].symbol(),
        "side bar on the body's left edge"
    );
    assert_eq!(
        "\u{2502}",
        buffer[(99, 5)].symbol(),
        "side bar on the body's right edge"
    );

    // Step 3: the history — timeline row highlighted, arrow down at the bar
    press(&mut app, &mut sim, KeyCode::Right);
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("The history"), "{text}");
    assert!(text.contains("3/4"), "{text}");
    assert!(
        text.contains('\u{25bc}'),
        "the history step points down: {text}"
    );
    let timeline_row = app.timeline_area.y;
    assert!(
        buffer[(50, timeline_row)]
            .modifier
            .contains(Modifier::REVERSED),
        "the timeline row is highlighted"
    );

    // Step 4: the Run button — button highlighted, arrow up at it
    press(&mut app, &mut sim, KeyCode::Char(' '));
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("Run the machine"), "{text}");
    assert!(text.contains("4/4"), "{text}");
    let run_button = app
        .menu_items
        .iter()
        .find(|(_, action)| matches!(action, MenuAction::RunStop))
        .map(|(rect, _)| *rect)
        .expect("the menu bar records the Run/Stop button");
    let button_center = run_button.x + run_button.width / 2;
    assert!(
        buffer[(button_center, 0)]
            .modifier
            .contains(Modifier::REVERSED),
        "the Run/Stop button is highlighted"
    );
    assert_eq!(
        "\u{25b2}",
        buffer[(button_center, 1)].symbol(),
        "the arrow on the pop-up's top border points at the button"
    );
}

#[test]
pub(super) fn welcome_seen_is_persisted_on_save() {
    let path = temp_config_path("welcome");
    let mut app = App::from_parts(Config::default(), None, Some(path.clone()));
    app.show_welcome_on_first_run();
    app.save_config();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("welcome_seen = true"), "unexpected: {text}");
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
