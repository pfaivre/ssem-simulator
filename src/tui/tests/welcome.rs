//! Tests for the first-run welcome pop-up, shown as a tour: shown once
//! (tracked by the `welcome_seen` config flag), navigated with
//! Enter/Space/Right (next, closing on the last step) and Left (back),
//! closed by Esc from any step and advanced by a click outside the pop-up,
//! with the navigation keys guarded against OS key-repeat, each step
//! pointing an arrow at the element it presents, and the intro rendering
//! the program that was loaded (if any).

use super::super::config::Config;
use super::super::palette::blend;
use super::*;
use crate::i18n::SessionLanguage;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

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
    let mut app = App::from_parts(config, None, None, SessionLanguage::same(config.language));
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
pub(super) fn click_outside_the_welcome_popup_advances_the_tour() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    // The render is needed only for its side effect: recording the pop-up's
    // last-drawn rect for the hit-tests below
    let _ = render(&mut app, &snapshot_with_history(0), 100, 40);
    let outside = (0..100u16)
        .flat_map(|c| (1..40u16).map(move |r| (c, r)))
        .find(|&(c, r)| !app.welcome_popup.contains(c, r))
        .expect("the pop-up does not cover the whole screen");
    let inside = (0..100u16)
        .flat_map(|c| (0..40u16).map(move |r| (c, r)))
        .find(|&(c, r)| app.welcome_popup.contains(c, r))
        .expect("the pop-up covers some cells");
    let click = |app: &mut App, sim: &mut Simulator, (column, row): (u16, u16)| {
        app.handle_mouse(
            MouseEvent {
                column,
                row,
                kind: MouseEventKind::Down(MouseButton::Left),
                modifiers: KeyModifiers::NONE,
            },
            sim,
        );
    };
    click(&mut app, &mut sim, outside);
    assert!(app.welcome_popup.is_open());
    assert_eq!(
        1,
        app.welcome_popup.step(),
        "a click outside advances to the next step instead of closing"
    );
    // A click inside the pop-up is consumed and changes nothing
    click(&mut app, &mut sim, inside);
    assert!(app.welcome_popup.is_open());
    assert_eq!(1, app.welcome_popup.step());
    // Advancing past the last step closes the tour
    click(&mut app, &mut sim, outside);
    click(&mut app, &mut sim, outside);
    assert_eq!(3, app.welcome_popup.step());
    assert!(app.welcome_popup.is_open());
    click(&mut app, &mut sim, outside);
    assert!(
        !app.welcome_popup.is_open(),
        "a click outside on the last step closes the tour"
    );
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
    let palette = app.palette();

    // Step 1: the intro, centered, no arrow
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("Welcome to the SSEM simulator"), "{text}");
    assert!(text.contains("1/4"), "{text}");
    assert!(
        !text.contains('\u{25b2}') && !text.contains('\u{25bc}'),
        "no arrow on the intro: {text}"
    );
    // The intro spotlights nothing: the whole screen around the pop-up is dimmed
    assert!(
        buffer[(0, 0)].modifier.contains(Modifier::DIM),
        "the menu bar is dimmed on the intro"
    );
    assert_eq!(palette.dim, buffer[(0, 0)].fg);

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
    let body = app.body_area;
    let side_row = body.y + 4;
    // Spotlight only: no border is drawn over the store text — the chrome
    // around the store is dimmed, the store itself keeps full brightness
    assert!(
        buffer[(0, 0)].modifier.contains(Modifier::DIM),
        "the menu bar is dimmed on the store step"
    );
    assert!(
        !buffer[(body.x + 5, side_row)]
            .modifier
            .contains(Modifier::DIM),
        "the store body keeps full brightness"
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
    let row_cell = &buffer[(50, timeline_row)];
    assert_eq!(
        palette.highlight, row_cell.bg,
        "the timeline row is filled with the tour color"
    );
    // The snapshot's history is full, so this bar cell is retained fill:
    // dark foreground on the glowing row
    assert_eq!(palette.bg, row_cell.fg);
    assert!(
        row_cell.modifier.contains(Modifier::BOLD) && !row_cell.modifier.contains(Modifier::DIM),
        "the timeline row keeps full brightness and is bold"
    );
    assert!(
        buffer[(0, 0)].modifier.contains(Modifier::DIM),
        "the menu bar is dimmed on the history step"
    );

    // Step 4: the Run button — button highlighted, arrow up at it
    press(&mut app, &mut sim, KeyCode::Char(' '));
    let buffer = render(&mut app, &snap, 100, 40);
    let text = screen_text(&buffer, 40);
    assert!(text.contains("Run the machine"), "{text}");
    assert!(text.contains("4/4"), "{text}");
    let run_button = app
        .menu_item_rect(MenuAction::RunStop)
        .expect("the menu bar records the Run/Stop button");
    let button_center = run_button.x + run_button.width / 2;
    let button_cell = &buffer[(button_center, 0)];
    assert_eq!(
        palette.highlight, button_cell.bg,
        "the Run/Stop button is filled with the tour color"
    );
    assert!(
        button_cell.modifier.contains(Modifier::BOLD)
            && !button_cell.modifier.contains(Modifier::DIM),
        "the button keeps full brightness and is bold"
    );
    let arrow_cell = &buffer[(button_center, 1)];
    assert_eq!(
        "\u{25b2}",
        arrow_cell.symbol(),
        "the arrow on the pop-up's top border points at the button"
    );
    assert!(
        arrow_cell.modifier.contains(Modifier::BOLD),
        "the arrow is bold"
    );
    assert!(
        buffer[(50, 39)].modifier.contains(Modifier::DIM),
        "the rest of the screen is dimmed on the run step"
    );
}

#[test]
pub(super) fn welcome_tour_dims_the_solid_chrome_bars_including_their_background() {
    let mut app = App::new();
    app.show_welcome_on_first_run();
    let snap = snapshot_with_history(0);
    let palette = app.palette();
    // The same screen without the tour, to compare cell styles against
    app.welcome_popup.close();
    let plain = render(&mut app, &snap, 100, 40);
    app.welcome_popup.open(); // intro step: the whole screen is dimmed
    let buffer = render(&mut app, &snap, 100, 40);
    // Menu bar and status bar cells: dimmed foreground + DIM modifier, and
    // the solid bar background blended toward the base background, so the
    // bars recede instead of glowing at full brightness
    for (x, y) in [(0u16, 0u16), (99, 0), (0, 39), (50, 39)] {
        let cell = &buffer[(x, y)];
        assert!(
            cell.modifier.contains(Modifier::DIM),
            "cell ({x}, {y}) is dimmed"
        );
        assert_eq!(palette.dim, cell.fg, "cell ({x}, {y}) foreground");
        assert_eq!(
            blend(plain[(x, y)].bg, palette.bg, 1, 2),
            cell.bg,
            "cell ({x}, {y}) background recedes toward the base bg"
        );
    }
}

#[test]
pub(super) fn welcome_tour_nav_keys_swallow_os_repeat() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    // Any repeat within the window is swallowed
    app.welcome_popup
        .set_nav_key_guard(Duration::from_secs(3600));

    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(1, app.welcome_popup.step());
    // The same key again immediately (OS key-repeat on terminals without
    // the kitty protocol): must not advance
    press(&mut app, &mut sim, KeyCode::Enter);
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(
        1,
        app.welcome_popup.step(),
        "a held key must not rush the tour"
    );
    // A different navigation key is guarded independently
    press(&mut app, &mut sim, KeyCode::Right);
    assert_eq!(2, app.welcome_popup.step());
    // Kitty-protocol Repeat events never advance either
    app.handle_key(
        KeyEvent::new_with_kind(KeyCode::Right, KeyModifiers::NONE, KeyEventKind::Repeat),
        &mut sim,
    );
    assert_eq!(2, app.welcome_popup.step());
}

#[test]
pub(super) fn welcome_tour_human_double_press_still_advances() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    app.welcome_popup
        .set_nav_key_guard(Duration::from_millis(60));

    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(1, app.welcome_popup.step());
    // OS key-repeat: same key within the window -> ignored
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(1, app.welcome_popup.step());
    // A human double-press after the window -> accepted
    std::thread::sleep(Duration::from_millis(100));
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(2, app.welcome_popup.step(), "a human double-press counts");
}

#[test]
pub(super) fn welcome_tour_history_track_uses_a_dim_highlight_variant() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.show_welcome_on_first_run();
    press(&mut app, &mut sim, KeyCode::Right);
    press(&mut app, &mut sim, KeyCode::Right); // step 2 = history
    let palette = app.palette();
    // Half-retained history: the bar's second half is the empty track
    let snap = Snapshot {
        history_len: 50,
        ..snapshot_with_history(0)
    };
    let buffer = render(&mut app, &snap, 100, 40);
    let timeline = app.timeline_area;
    let (bar_x, bar_width) = timeline_bar_geometry(timeline, snap.history_start, snap.history_head);
    let filled = timeline_filled_len(bar_width, snap.history_len, snap.history_capacity);
    assert!(
        filled > 0 && filled < usize::from(bar_width),
        "need both fill and track"
    );
    let dim_highlight = blend(palette.highlight, palette.bg, 1, 2);
    // A retained (filled) bar cell: dark foreground on the glowing row
    let fill_cell = &buffer[(bar_x + (filled / 2) as u16, timeline.y)];
    assert_eq!(palette.highlight, fill_cell.bg);
    assert_eq!(palette.bg, fill_cell.fg);
    // A track cell: a dimmer variant of the tour color, not the flat bg
    let track_cell = &buffer[(
        bar_x + filled as u16 + (bar_width as usize - filled) as u16 / 2,
        timeline.y,
    )];
    assert_eq!(palette.highlight, track_cell.bg);
    assert_eq!(dim_highlight, track_cell.fg);
    // A cell outside the bar (the label area): dark text on the glowing row
    let label_cell = &buffer[(timeline.x, timeline.y)];
    assert_eq!(palette.highlight, label_cell.bg);
    assert_eq!(palette.bg, label_cell.fg);
}

#[test]
pub(super) fn welcome_tour_renders_at_degenerate_terminal_sizes() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = snapshot_with_history(0);
    // No minimum terminal size: every step must render without panicking,
    // even on a 0-cell or 1-2-column screen (the arrow is simply dropped
    // when the pop-up is too small to carry one)
    for &(w, h) in &[
        (0u16, 0u16),
        (1, 10),
        (2, 40),
        (3, 5),
        (10, 3),
        (10, 0),
        (100, 40),
    ] {
        app.welcome_popup.open();
        for _ in 0..4 {
            let _ = render(&mut app, &snap, w, h);
            press(&mut app, &mut sim, KeyCode::Right);
        }
    }
}

#[test]
pub(super) fn welcome_seen_is_persisted_on_save() {
    let path = temp_config_path("welcome");
    let mut app = App::from_parts(
        Config::default(),
        None,
        Some(path.clone()),
        SessionLanguage::default(),
    );
    app.show_welcome_on_first_run();
    app.save_config();
    let text = fs::read_to_string(&path).unwrap();
    assert!(text.contains("welcome_seen = true"), "unexpected: {text}");
    fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
