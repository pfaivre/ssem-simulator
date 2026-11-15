//! Tests for the open pop-up (O key / Open menu item): a path field;
//! ENTER loads the file into the machine, replacing the current state.

use super::*;
use std::path::{Path, PathBuf};

/// Type a string into the app as if the user pressed its keys
fn type_str(app: &mut App, sim: &mut Simulator, text: &str) {
    for c in text.chars() {
        app.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), sim);
    }
}

/// Press a non-character key
fn press(app: &mut App, sim: &mut Simulator, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), sim);
}

/// The whole screen text, rows joined with newlines
fn screen_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|row| row_text(buffer, row))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
pub(super) fn o_key_opens_the_open_popup_and_esc_closes_it() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = snapshot_with_history(0);

    // The Open action is advertised in the menu bar (the top row)
    let buffer = render(&mut app, &snap, 200, 40);
    let menu = row_text(&buffer, 0);
    assert!(menu.contains("Open"), "menu bar: {menu:?}");

    assert!(!app.open_popup.is_open());
    press(&mut app, &mut sim, KeyCode::Char('o'));
    assert!(app.open_popup.is_open());
    let buffer = render(&mut app, &snap, 100, 40);
    let all = screen_text(&buffer);
    assert!(all.contains(" Open "), "pop-up title missing: {all}");
    assert!(all.contains("File:"), "input line missing: {all}");

    press(&mut app, &mut sim, KeyCode::Esc);
    assert!(!app.open_popup.is_open());
}

#[test]
pub(super) fn opening_the_open_popup_pauses_a_running_machine() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.stop_flag = false; // running

    press(&mut app, &mut sim, KeyCode::Char('o'));
    assert!(app.open_popup.is_open());
    assert!(sim.stop_flag, "the open pop-up pauses the machine");
}

#[test]
pub(super) fn open_popup_prefills_the_source_path() {
    let mut app = App::new();
    let mut sim = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    press(&mut app, &mut sim, KeyCode::Char('o'));
    assert_eq!("samples/factorct.asm", app.open_popup.buffer());
}

#[test]
pub(super) fn enter_loads_the_file_and_replaces_the_machine_state() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.enable_history(100);
    sim.run(5);
    sim.store.words[3] = 42;
    sim.stop_flag = true;

    press(&mut app, &mut sim, KeyCode::Char('o'));
    type_str(&mut app, &mut sim, "samples/factorct.asm");
    press(&mut app, &mut sim, KeyCode::Enter);

    assert!(
        !app.open_popup.is_open(),
        "pop-up closed after a successful load"
    );
    let reference = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    assert_eq!(
        reference.store.words, sim.store.words,
        "store loaded from the file"
    );
    assert_eq!(0, sim.cycle_count, "cycle count reset");
    assert!(sim.stop_flag, "the loaded machine starts paused");
    assert_eq!(Some(PathBuf::from("samples/factorct.asm")), sim.source_path);
    let h = sim.history.as_ref().expect("history kept");
    assert_eq!(1, h.len(), "history restarted at the loaded state");
    assert_eq!(sim.store.words, h.get(0).unwrap().store);
}

#[test]
pub(super) fn open_popup_reports_errors_and_stays_open() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.store.words[3] = 42;

    // A missing file: the error is shown, the pop-up stays open, the
    // machine is untouched
    press(&mut app, &mut sim, KeyCode::Char('o'));
    type_str(&mut app, &mut sim, "samples/no_such_file.asm");
    press(&mut app, &mut sim, KeyCode::Enter);
    assert!(app.open_popup.is_open(), "pop-up stays open on error");
    assert_eq!(42, sim.store.words[3], "machine untouched");
    assert_eq!(None, sim.source_path);
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(
        screen_text(&buffer).contains("Error:"),
        "error message shown"
    );

    // A bad extension is reported the same way
    for _ in 0..64 {
        press(&mut app, &mut sim, KeyCode::Backspace);
    }
    type_str(&mut app, &mut sim, "samples/virpet.txt");
    press(&mut app, &mut sim, KeyCode::Enter);
    assert!(app.open_popup.is_open());
    assert_eq!(42, sim.store.words[3], "machine untouched");
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(
        screen_text(&buffer).contains("Error:"),
        "error message shown"
    );
}

#[test]
pub(super) fn open_popup_path_field_whitelist_and_editing() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    press(&mut app, &mut sim, KeyCode::Char('o'));

    // Path characters are accepted
    type_str(&mut app, &mut sim, "a/b-c_d.e");
    assert_eq!("a/b-c_d.e", app.open_popup.buffer());

    // A character outside the whitelist is rejected with a message
    press(&mut app, &mut sim, KeyCode::Char(';'));
    assert_eq!("a/b-c_d.e", app.open_popup.buffer());
    assert!(
        !app.open_popup.message().is_empty(),
        "rejection message shown"
    );

    // Backspace edits, and an empty field + Enter asks for a path
    press(&mut app, &mut sim, KeyCode::Backspace);
    assert_eq!("a/b-c_d.", app.open_popup.buffer());
    for _ in 0..16 {
        press(&mut app, &mut sim, KeyCode::Backspace);
    }
    assert!(app.open_popup.buffer().is_empty());
    press(&mut app, &mut sim, KeyCode::Enter);
    assert!(
        app.open_popup.is_open(),
        "empty path does not close the pop-up"
    );
    assert!(
        !app.open_popup.message().is_empty(),
        "type-a-path message shown"
    );
}
