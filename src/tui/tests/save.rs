//! Tests for the save pop-up (S key): a single name field, prefilled with
//! `<name>_edited.<ext>`, saved into the open file's folder. The field
//! accepts only alphanumerics, dash, underscore and dot.

use super::*;
use std::path::PathBuf;

/// A temporary directory removed when dropped, even if the test panics
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ssem_tui_save_tests_{name}"));
        let _ = std::fs::remove_dir_all(&path); // Remove any leftover from a previous run
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    /// The canonical (absolute) form of the directory, as the pop-up shows it
    fn canonical(&self) -> PathBuf {
        std::fs::canonicalize(&self.0).unwrap()
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A simulator loaded from a copy of `sample` placed in a fresh temp dir,
/// so saves never touch the repo's samples/
fn sim_from_temp_copy(sample: &str, dir_name: &str, file_name: &str) -> (TempDir, Simulator) {
    let dir = TempDir::new(dir_name);
    std::fs::copy(format!("samples/{sample}"), dir.0.join(file_name)).unwrap();
    let sim = Simulator::from_file(&dir.0.join(file_name)).unwrap();
    (dir, sim)
}

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

/// Clear the name field (the pop-up opens with a prefilled default name)
fn clear_field(app: &mut App, sim: &mut Simulator) {
    for _ in 0..64 {
        press(app, sim, KeyCode::Backspace);
    }
}

/// Whether any screen row contains the given text
fn screen_contains(buffer: &Buffer, text: &str) -> bool {
    (0..buffer.area.height).any(|row| row_text(buffer, row).contains(text))
}

/// The whole screen text, rows joined with newlines
fn screen_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|row| row_text(buffer, row))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The "DIR: ..." text of the save pop-up, if drawn (border stripped)
fn dir_line(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|row| row_text(buffer, row))
        .find(|row| row.contains("DIR: "))
        .and_then(|row| row.find("DIR: ").map(|i| row[i..].to_string()))
        .unwrap_or_default()
        .trim_end()
        .trim_end_matches('│')
        .trim_end()
        .to_string()
}

/// Open the save pop-up with the S key
fn open_save_popup(app: &mut App, sim: &mut Simulator) {
    app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE), sim);
    assert!(app.save_popup.is_open());
}

#[test]
pub(super) fn s_key_opens_the_save_popup_and_esc_closes_it() {
    let mut app = App::new();
    let mut sim = Simulator::new();
    let snap = snapshot_with_history(0);

    // The Save action is advertised in the menu bar (the top row)
    let buffer = render(&mut app, &snap, 200, 40);
    let menu = row_text(&buffer, 0);
    assert!(menu.contains("Save"), "menu bar: {menu:?}");

    assert!(!app.save_popup.is_open());
    open_save_popup(&mut app, &mut sim);
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(screen_contains(&buffer, " SAVE "), "pop-up title missing");
    assert!(screen_contains(&buffer, "FILE:"), "input line missing");
    assert!(screen_contains(&buffer, "DIR:"), "folder line missing");
    assert!(screen_contains(&buffer, ".asm"));

    app.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &mut sim);
    assert!(!app.save_popup.is_open());
}

#[test]
pub(super) fn enter_with_prefilled_name_saves_asm_into_the_open_file_folder() {
    let mut app = App::new();
    let (dir, mut sim) = sim_from_temp_copy("factorct.asm", "save_asm", "factorct.asm");
    sim.run(5);

    open_save_popup(&mut app, &mut sim);
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 120, 40);

    // The pop-up targets the open file's folder, as an absolute path...
    assert_eq!(
        format!("DIR: {}", dir.canonical().display()),
        dir_line(&buffer)
    );
    // ...and the field is prefilled with <name>_edited.<ext>
    assert!(
        screen_contains(&buffer, "FILE: factorct_edited.asm_"),
        "default name missing: {}",
        screen_text(&buffer)
    );

    // Enter right away saves into that folder as factorct_edited.asm
    press(&mut app, &mut sim, KeyCode::Enter);
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(
        screen_contains(&buffer, "SAVED TO"),
        "no confirmation shown"
    );
    let saved = dir.canonical().join("factorct_edited.asm");
    let content = std::fs::read_to_string(&saved).unwrap();
    assert!(
        content.starts_with("; Machine state saved"),
        "content: {content}"
    );
    assert!(
        content.contains(&format!("CI: {}", sim.ci)),
        "content: {content}"
    );
    // Instructions as mnemonics, data words as NUM, source comments after ';'
    assert!(content.contains("01 LDN 24; A = -a"), "content: {content}");
    assert!(content.contains("20 NUM -3;"), "content: {content}");
    let reloaded = Simulator::from_file(&saved).unwrap();
    assert_eq!(sim.store, reloaded.store);
    assert_eq!(sim.store.comments, reloaded.store.comments);
}

#[test]
pub(super) fn enter_with_prefilled_name_saves_snp_and_keeps_extension() {
    let mut app = App::new();
    let (dir, mut sim) = sim_from_temp_copy("nightmare.snp", "save_snp", "nightmare.snp");
    sim.run(5);

    open_save_popup(&mut app, &mut sim);
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 120, 40);
    assert_eq!(
        format!("DIR: {}", dir.canonical().display()),
        dir_line(&buffer)
    );
    assert!(screen_contains(&buffer, "FILE: nightmare_edited.snp_"));

    press(&mut app, &mut sim, KeyCode::Enter);
    let saved = dir.canonical().join("nightmare_edited.snp");
    let content = std::fs::read_to_string(&saved).unwrap();
    assert!(content.contains("0000: "), "not a snapshot body: {content}");
    let reloaded = Simulator::from_file(&saved).unwrap();
    assert_eq!(sim.store, reloaded.store);
    assert_eq!(sim.store.comments, reloaded.store.comments);
}

#[test]
pub(super) fn default_name_is_not_doubled_for_an_already_edited_file() {
    let mut app = App::new();
    let (_dir, mut sim) = sim_from_temp_copy("factorct.asm", "save_edited", "factorct_edited.asm");

    open_save_popup(&mut app, &mut sim);
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 120, 40);
    // The prefilled name keeps the single _edited suffix
    assert!(
        screen_contains(&buffer, "FILE: factorct_edited.asm_"),
        "default name wrong: {}",
        screen_text(&buffer)
    );
    assert!(!screen_contains(&buffer, "_edited_edited"));
}

#[test]
pub(super) fn unknown_extension_reports_an_error_and_writes_nothing() {
    let mut app = App::new();
    let (dir, mut sim) = sim_from_temp_copy("factorct.asm", "bad_ext", "factorct.asm");

    open_save_popup(&mut app, &mut sim);
    clear_field(&mut app, &mut sim);
    type_str(&mut app, &mut sim, "state.txt");
    press(&mut app, &mut sim, KeyCode::Enter);

    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 140, 40);
    assert!(screen_contains(&buffer, "ERROR:"), "no error shown");
    assert!(screen_contains(&buffer, ".asm or .snp"));
    assert!(
        !dir.canonical().join("state.txt").exists(),
        "a file must not be written on a bad extension"
    );
    // The pop-up stays open so the name can be fixed
    assert!(app.save_popup.is_open());
}

#[test]
pub(super) fn only_alphanumeric_dash_underscore_dot_are_accepted() {
    let mut app = App::new();
    let (_dir, mut sim) = sim_from_temp_copy("factorct.asm", "whitelist", "factorct.asm");

    open_save_popup(&mut app, &mut sim);
    clear_field(&mut app, &mut sim);
    // '/', space, '$', '!', 'é' are rejected; a b c d . e - pass through
    type_str(&mut app, &mut sim, "a/b c$d!.e-\u{e9}");
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(
        screen_contains(&buffer, "FILE: abcd.e-_"),
        "whitelist not applied: {}",
        screen_text(&buffer)
    );
    assert!(screen_contains(&buffer, "ONLY A-Z 0-9 - _ . ALLOWED"));

    // A bare ".." passes the whitelist but cannot escape the folder:
    // writing to <dir>/.. fails with an I/O error shown in the pop-up
    clear_field(&mut app, &mut sim);
    type_str(&mut app, &mut sim, "..");
    press(&mut app, &mut sim, KeyCode::Enter);
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(screen_contains(&buffer, "ERROR:"), "expected a write error");
    assert!(app.save_popup.is_open());
}

#[test]
pub(super) fn keys_go_to_the_popup_not_the_app_while_it_is_open() {
    let mut app = App::new();
    let mut sim = Simulator::new();

    open_save_popup(&mut app, &mut sim);
    // 'q' would quit if it reached the app handler; it lands in the name field
    app.handle_key(
        KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        &mut sim,
    );
    assert!(
        !app.should_quit,
        "Q must not quit while the save pop-up is open"
    );
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(
        screen_contains(&buffer, "FILE: q_"),
        "q should be in the field"
    );

    // Left would scrub the history if it reached the app handler; the
    // pop-up ignores it
    press(&mut app, &mut sim, KeyCode::Left);
    assert_eq!(
        0, sim.cycle_count,
        "arrows must not scrub while the save pop-up is open"
    );

    // Backspace edits the field: it removes the typed 'q'
    press(&mut app, &mut sim, KeyCode::Backspace);
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(
        screen_contains(&buffer, "FILE: _"),
        "backspace should empty the field"
    );
}

#[test]
pub(super) fn save_popup_without_open_file_starts_in_cwd_with_empty_name() {
    let mut app = App::new();
    let mut sim = Simulator::new(); // no source file

    open_save_popup(&mut app, &mut sim);
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 140, 40);

    let cwd = std::env::current_dir().unwrap();
    let canonical = std::fs::canonicalize(&cwd).unwrap();
    assert_eq!(format!("DIR: {}", canonical.display()), dir_line(&buffer));
    assert!(screen_contains(&buffer, "FILE: _"), "field should be empty");
}
