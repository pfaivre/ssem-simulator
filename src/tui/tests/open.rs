//! Tests for the open pop-up (O key / Open menu item): a file browser over
//! the directory tree. Only loadable files (.asm/.snp) and subdirectories
//! are listed (hidden entries are skipped, directories first); the arrows
//! move the highlight, ENTER opens a file or enters a directory,
//! LEFT/BACKSPACE goes up, and mouse clicks work too.

use super::*;
use std::fs;
use std::path::{Path, PathBuf};

/// Press a non-character key
fn press(app: &mut App, sim: &mut Simulator, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), sim);
}

/// Send a left-button press at (column, row)
fn click(app: &mut App, sim: &mut Simulator, column: u16, row: u16) {
    app.handle_mouse(
        MouseEvent {
            column,
            row,
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
        },
        sim,
    );
}

/// Send a mouse wheel event
fn wheel(app: &mut App, sim: &mut Simulator, up: bool) {
    app.handle_mouse(
        MouseEvent {
            column: 100,
            row: 20,
            kind: if up {
                MouseEventKind::ScrollUp
            } else {
                MouseEventKind::ScrollDown
            },
            modifiers: KeyModifiers::NONE,
        },
        sim,
    );
}

/// The whole screen text, rows joined with newlines
fn screen_text(buffer: &Buffer) -> String {
    (0..buffer.area.height)
        .map(|row| row_text(buffer, row))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A fresh temp directory with a known tree, canonicalized:
/// `alpha.asm`, `beta.snp`, `broken.asm`, `notes.txt`, `.hidden.asm`,
/// `sub/gamma.asm`, `empty/`
fn fixture(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("ssem-open-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::create_dir_all(root.join("empty")).unwrap();
    fs::write(root.join("alpha.asm"), "00 JMP 0\n01 JMP 0\n").unwrap();
    fs::write(
        root.join("beta.snp"),
        "0000: 10000000000000000000000000000000\n",
    )
    .unwrap();
    fs::write(root.join("broken.asm"), "00 FOO 1\n").unwrap();
    fs::write(root.join("notes.txt"), "not loadable\n").unwrap();
    fs::write(root.join(".hidden.asm"), "00 JMP 0\n").unwrap();
    fs::write(root.join("sub").join("gamma.asm"), "00 JMP 0\n").unwrap();
    std::fs::canonicalize(&root).unwrap()
}

/// Screen cell of the browser entry `entry`, found by probing the hit-test
/// over the whole screen (the pop-up must have been drawn once).
fn entry_cell(app: &App, entry: usize) -> (u16, u16) {
    for row in 0..40u16 {
        for col in 0..200u16 {
            if app.open_popup.entry_at(col, row) == Some(entry) {
                return (col, row);
            }
        }
    }
    panic!("browser entry {entry} not found on screen");
}

#[test]
pub(super) fn o_key_opens_the_browser_and_esc_closes_it() {
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
    let buffer = render(&mut app, &snap, 200, 40);
    let all = screen_text(&buffer);
    assert!(all.contains(" Open "), "pop-up title missing: {all}");
    assert!(all.contains("ENTER open"), "footer missing: {all}");
    // Without a loaded file, the browser starts in the cwd, shown in the title
    let cwd = std::fs::canonicalize(std::env::current_dir().unwrap()).unwrap();
    assert_eq!(cwd, app.open_popup.dir());
    assert!(
        all.contains(&cwd.display().to_string()),
        "dir in title: {all}"
    );

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
pub(super) fn browser_starts_in_the_loaded_files_folder_with_the_file_selected() {
    let mut app = App::new();
    let mut sim = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    press(&mut app, &mut sim, KeyCode::Char('o'));
    let samples = std::fs::canonicalize("samples").unwrap();
    assert_eq!(samples, app.open_popup.dir());
    assert_eq!(Some("factorct.asm"), app.open_popup.selected_name());
    let names = app.open_popup.entry_names();
    assert_eq!(Some(".."), names.first().copied());
    assert!(names.contains(&"tests"), "subdirectory listed: {names:?}");
    assert!(names.contains(&"factorct.asm"));
    assert!(names.contains(&"nightmare.snp"));
}

#[test]
pub(super) fn browser_lists_only_compatible_files_with_dirs_first() {
    let root = fixture("list");
    let mut app = App::new();
    let sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));

    assert_eq!(root, app.open_popup.dir());
    // ".." first, then directories, then loadable files, each alphabetical;
    // notes.txt and the hidden file are not listed
    assert_eq!(
        vec!["..", "empty", "sub", "alpha.asm", "beta.snp", "broken.asm"],
        app.open_popup.entry_names()
    );
    assert_eq!(Some("alpha.asm"), app.open_popup.selected_name());

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 200, 40);
    let all = screen_text(&buffer);
    assert!(
        all.contains("sub/"),
        "directories get a trailing slash: {all}"
    );
    assert!(all.contains("alpha.asm"));
    assert!(all.contains("6/6"), "footer entry count: {all}");
    assert!(
        !all.contains("notes.txt"),
        "incompatible file hidden: {all}"
    );
    assert!(!all.contains("hidden"), "hidden file skipped: {all}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn long_lists_scroll_and_the_footer_counts_the_entries() {
    let root = big_fixture("scroll");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));
    // "..", "empty", "sub", then 33 files: 36 entries in all
    assert_eq!(36, app.open_popup.entry_names().len());

    // End jumps to the last entry, and the window scrolls to show the tail
    press(&mut app, &mut sim, KeyCode::End);
    assert_eq!(Some("f29.asm"), app.open_popup.selected_name());
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 200, 40);
    let all = screen_text(&buffer);
    assert!(all.contains("f29.asm"), "the tail is visible: {all}");
    assert!(all.contains("20/36"), "footer count of shown/total: {all}");
    assert!(!all.contains("alpha.asm"), "scrolled past the top: {all}");

    // Home scrolls back to the top
    press(&mut app, &mut sim, KeyCode::Home);
    assert_eq!(Some(".."), app.open_popup.selected_name());
    let buffer = render(&mut app, &snap, 200, 40);
    assert!(
        screen_text(&buffer).contains("alpha.asm"),
        "top visible again"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn arrows_move_and_enter_loads_the_highlighted_file() {
    let root = fixture("load");
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.enable_history(100);
    sim.run(5);
    sim.store.words[3] = 42;
    sim.stop_flag = true;

    // Start in sub/ with gamma.asm highlighted
    app.open_popup
        .open(Some(&root.join("sub").join("gamma.asm")));
    assert_eq!(root.join("sub"), app.open_popup.dir());
    assert_eq!(Some("gamma.asm"), app.open_popup.selected_name());

    // Left goes up, highlighting the folder just left
    press(&mut app, &mut sim, KeyCode::Left);
    assert_eq!(root, app.open_popup.dir());
    assert_eq!(Some("sub"), app.open_popup.selected_name());

    // Down moves to the next entry, Enter loads it
    press(&mut app, &mut sim, KeyCode::Down);
    assert_eq!(Some("alpha.asm"), app.open_popup.selected_name());
    press(&mut app, &mut sim, KeyCode::Enter);

    assert!(
        !app.open_popup.is_open(),
        "pop-up closed after a successful load"
    );
    let reference = Simulator::from_file(&root.join("alpha.asm")).unwrap();
    assert_eq!(
        reference.store.words, sim.store.words,
        "store loaded from the file"
    );
    assert_eq!(0, sim.cycle_count, "cycle count reset");
    assert!(sim.stop_flag, "the loaded machine starts paused");
    assert_eq!(Some(root.join("alpha.asm")), sim.source_path);
    let h = sim.history.as_ref().expect("history kept");
    assert_eq!(1, h.len(), "history restarted at the loaded state");
    assert_eq!(sim.store.words, h.get(0).unwrap().store);
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn enter_on_a_directory_descends_and_backspace_goes_back_up() {
    let root = fixture("nav");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));
    // entries: 0="..", 1="empty", 2="sub", 3="alpha.asm", …
    assert_eq!(Some("alpha.asm"), app.open_popup.selected_name());

    press(&mut app, &mut sim, KeyCode::Up);
    assert_eq!(Some("sub"), app.open_popup.selected_name());
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(root.join("sub"), app.open_popup.dir());
    assert_eq!(vec!["..", "gamma.asm"], app.open_popup.entry_names());
    assert_eq!(Some(".."), app.open_popup.selected_name());

    press(&mut app, &mut sim, KeyCode::Backspace);
    assert_eq!(root, app.open_popup.dir());
    assert_eq!(Some("sub"), app.open_popup.selected_name());

    // The current path is shown in the pop-up title
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 200, 40);
    assert!(
        screen_text(&buffer).contains(&root.display().to_string()),
        "current dir in the title"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn loading_a_broken_file_shows_an_error_and_stays_open() {
    let root = fixture("error");
    let mut app = App::new();
    let mut sim = Simulator::new();
    sim.store.words[3] = 42;

    app.open_popup.open(Some(&root.join("broken.asm")));
    assert_eq!(Some("broken.asm"), app.open_popup.selected_name());
    press(&mut app, &mut sim, KeyCode::Enter);

    assert!(app.open_popup.is_open(), "pop-up stays open on error");
    assert_eq!(42, sim.store.words[3], "machine untouched");
    assert_eq!(None, sim.source_path);
    assert!(
        app.open_popup.message().contains("Error:"),
        "error in the message: {:?}",
        app.open_popup.message()
    );
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 200, 40);
    assert!(
        screen_text(&buffer).contains("Error:"),
        "error message shown"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn a_folder_without_compatible_files_shows_a_hint() {
    let root = fixture("empty");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));

    // entries: 0="..", 1="empty", 2="sub", 3="alpha.asm", …
    press(&mut app, &mut sim, KeyCode::Up);
    press(&mut app, &mut sim, KeyCode::Up);
    assert_eq!(Some("empty"), app.open_popup.selected_name());
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(root.join("empty"), app.open_popup.dir());
    assert_eq!(vec![".."], app.open_popup.entry_names());

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 200, 40);
    assert!(
        screen_text(&buffer).contains("no .asm or .snp files"),
        "empty-folder hint shown"
    );

    // Going up still works from an empty folder
    press(&mut app, &mut sim, KeyCode::Backspace);
    assert_eq!(root, app.open_popup.dir());
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn clicking_a_file_loads_it_and_a_click_outside_closes_the_browser() {
    let root = fixture("click");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));

    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 200, 40);
    // entries: 0="..", 1="empty", 2="sub", 3="alpha.asm", 4="beta.snp", 5="broken.asm"
    let (col, row) = entry_cell(&app, 4);
    click(&mut app, &mut sim, col, row);
    assert!(
        !app.open_popup.is_open(),
        "click on a file loads it and closes"
    );
    assert_eq!(Some(root.join("beta.snp")), sim.source_path);
    let reference = Simulator::from_file(&root.join("beta.snp")).unwrap();
    assert_eq!(reference.store.words, sim.store.words);

    // Reopen, then click outside the pop-up: it closes without loading
    app.open_popup.open(Some(&root.join("alpha.asm")));
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 200, 40);
    let mut outside = None;
    'search: for row in 0..40u16 {
        for col in 0..200u16 {
            if !app.open_popup.contains(col, row) {
                outside = Some((col, row));
                break 'search;
            }
        }
    }
    let (col, row) = outside.expect("a cell outside the pop-up");
    click(&mut app, &mut sim, col, row);
    assert!(
        !app.open_popup.is_open(),
        "click outside closes the browser"
    );
    assert_eq!(
        Some(root.join("beta.snp")),
        sim.source_path,
        "machine unchanged"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn clicking_a_directory_enters_it() {
    let root = fixture("clickdir");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));

    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 200, 40);
    // entries: 0="..", 1="empty", 2="sub", …
    let (col, row) = entry_cell(&app, 2);
    click(&mut app, &mut sim, col, row);
    assert!(
        app.open_popup.is_open(),
        "clicking a directory keeps the browser open"
    );
    assert_eq!(root.join("sub"), app.open_popup.dir());
    assert_eq!(vec!["..", "gamma.asm"], app.open_popup.entry_names());
    let _ = fs::remove_dir_all(&root);
}

/// A fixture root with 30 extra files: 36 entries in all (0="..", 1="empty",
/// 2="sub", 3="alpha.asm", 4="beta.snp", 5="broken.asm", 6..36="f00".."f29")
fn big_fixture(tag: &str) -> PathBuf {
    let root = fixture(tag);
    for i in 0..30 {
        fs::write(root.join(format!("f{i:02}.asm")), "00 JMP 0\n").unwrap();
    }
    root
}

#[test]
pub(super) fn wheel_scrolls_the_view_when_the_list_exceeds_the_popup() {
    let root = big_fixture("wheel");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 200, 40);
    assert_eq!(Some("alpha.asm"), app.open_popup.selected_name());

    // Five notches down: the view scrolls and the highlight follows the top
    for _ in 0..5 {
        wheel(&mut app, &mut sim, false);
    }
    assert_eq!(Some("broken.asm"), app.open_popup.selected_name()); // entry 5
    let buffer = render(&mut app, &snap, 200, 40);
    let all = screen_text(&buffer);
    assert!(!all.contains("alpha.asm"), "scrolled past the top: {all}");
    assert!(all.contains("f18.asm"), "the new tail is visible: {all}");
    assert!(!all.contains("f19.asm"), "one row beyond the window: {all}");
    // The modal browser swallowed the wheel: the store view did not scroll
    assert_eq!(0, app.store_scroll);

    // Five notches up scroll back to the top
    for _ in 0..5 {
        wheel(&mut app, &mut sim, true);
    }
    let buffer = render(&mut app, &snap, 200, 40);
    let all = screen_text(&buffer);
    assert!(all.contains("alpha.asm"), "top visible again: {all}");
    assert!(!all.contains("f29.asm"), "tail scrolled away: {all}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn wheel_does_nothing_when_the_whole_list_fits() {
    let root = fixture("wheelfit");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm")));
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 200, 40);

    for _ in 0..3 {
        wheel(&mut app, &mut sim, false);
    }
    for _ in 0..3 {
        wheel(&mut app, &mut sim, true);
    }
    assert_eq!(Some("alpha.asm"), app.open_popup.selected_name());
    let buffer = render(&mut app, &snap, 200, 40);
    assert!(
        screen_text(&buffer).contains(".."),
        "the view did not scroll"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
pub(super) fn page_up_and_page_down_move_the_highlight_by_a_page() {
    let root = big_fixture("page");
    let mut app = App::new();
    let mut sim = Simulator::new();
    app.open_popup.open(Some(&root.join("alpha.asm"))); // selected = 3
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 200, 40); // 20 entry rows fit

    // PageDown moves one page (20 rows) and scrolls the view along
    press(&mut app, &mut sim, KeyCode::PageDown);
    assert_eq!(Some("f17.asm"), app.open_popup.selected_name()); // entry 23
    let buffer = render(&mut app, &snap, 200, 40);
    assert!(
        !screen_text(&buffer).contains("alpha.asm"),
        "the page scrolled past the top"
    );

    press(&mut app, &mut sim, KeyCode::PageUp);
    assert_eq!(Some("alpha.asm"), app.open_popup.selected_name());

    // PageUp at the top clamps to the first entry, PageDown at the end to the last
    press(&mut app, &mut sim, KeyCode::PageUp);
    assert_eq!(Some(".."), app.open_popup.selected_name());
    press(&mut app, &mut sim, KeyCode::End);
    press(&mut app, &mut sim, KeyCode::PageDown);
    assert_eq!(Some("f29.asm"), app.open_popup.selected_name());
    let _ = fs::remove_dir_all(&root);
}
