//! Tests for the pop-up layer's mouse behavior: a left click outside an open
//! pop-up closes it (the behavior the settings pop-up always had, now
//! generalized to every pop-up), a click inside a pop-up is consumed by it,
//! and when several pop-ups are open, each one the click lies outside of
//! closes.

use super::*;

/// Send a left-button press at (column, row).
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

/// Double-click: two presses at the same cell within the threshold.
fn double_click(app: &mut App, sim: &mut Simulator, column: u16, row: u16) {
    click(app, sim, column, row);
    click(app, sim, column, row);
}

/// Send a key press.
fn press(app: &mut App, sim: &mut Simulator, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE), sim);
}

/// Type a string, one key press per character.
fn type_str(app: &mut App, sim: &mut Simulator, s: &str) {
    for c in s.chars() {
        press(app, sim, KeyCode::Char(c));
    }
}

/// The first screen cell (row-major, the menu bar row skipped) for which
/// `pred` holds. The app must have been drawn once, so the pop-ups' rects
/// are known to the hit-testing.
fn cell_where(pred: impl Fn(u16, u16) -> bool) -> (u16, u16) {
    for row in 1..40u16 {
        for col in 0..100u16 {
            if pred(col, row) {
                return (col, row);
            }
        }
    }
    panic!("no screen cell matches");
}

/// An app with a pinned, predictable layout: no flow gutter, no heatmap.
fn plain_app() -> App {
    let mut app = App::new();
    app.flow = FlowMode::Off;
    app.heatmap = HeatmapMode::Off;
    app
}

/// A simulator whose store carries comments on words 1 and 4.
fn sim_with_comments() -> Simulator {
    let mut sim = Simulator::new();
    Arc::make_mut(&mut sim.store.comments)[1] = Some(" A = -a".into());
    Arc::make_mut(&mut sim.store.comments)[4] = Some("   x".into());
    sim
}

#[test]
pub(super) fn click_outside_the_help_popup_closes_it() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.help_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| !app.help_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.help_popup.is_open());
}

#[test]
pub(super) fn click_inside_the_help_popup_keeps_it_open() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.help_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| app.help_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(app.help_popup.is_open(), "a click inside is consumed");
}

#[test]
pub(super) fn click_outside_the_breakpoint_popup_closes_it() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.breakpoint_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| !app.breakpoint_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.breakpoint_popup.is_open());
}

#[test]
pub(super) fn click_inside_the_breakpoint_popup_keeps_it_open() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.breakpoint_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| app.breakpoint_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(app.breakpoint_popup.is_open());
}

#[test]
pub(super) fn click_outside_the_editor_popup_cancels_without_applying() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.editor_popup.open_for_ci();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    type_str(&mut app, &mut sim, "7");

    let (col, row) = cell_where(|c, r| !app.editor_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.editor_popup.is_open());
    assert_eq!(0, sim.ci, "the click outside cancels; only Enter applies");
}

#[test]
pub(super) fn click_inside_the_editor_popup_keeps_it_open() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.editor_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| app.editor_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(app.editor_popup.is_open());
}

#[test]
pub(super) fn click_outside_the_save_popup_closes_it() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.save_popup.open(None);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| !app.save_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.save_popup.is_open());
}

#[test]
pub(super) fn click_inside_the_save_popup_keeps_it_open() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.save_popup.open(None);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| app.save_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(app.save_popup.is_open());
}

#[test]
pub(super) fn click_outside_the_comment_editor_cancels_without_applying() {
    let mut sim = sim_with_comments();
    let mut app = plain_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Double-click the comment of word 1 (screen row 6) to open the editor
    double_click(&mut app, &mut sim, 60, 6);
    assert!(app.comment_editor.is_open_for(1));
    type_str(&mut app, &mut sim, "ZZZ");
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| !app.comment_editor.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.comment_editor.is_open());
    assert_eq!(
        Some(" A = -a".to_string()),
        sim.store.comments[1],
        "the click outside cancels; only Enter applies"
    );
}

#[test]
pub(super) fn click_inside_the_comment_box_keeps_the_editor_open() {
    let mut sim = sim_with_comments();
    let mut app = plain_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| app.comment_editor.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(
        app.comment_editor.is_open(),
        "a click inside the box is consumed"
    );
}

#[test]
pub(super) fn a_click_inside_the_settings_popup_but_not_on_a_control_keeps_it_open() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.settings_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| {
        app.settings_popup.contains(c, r) && app.settings_popup.control_at(c, r).is_none()
    });
    click(&mut app, &mut sim, col, row);
    assert!(app.settings_popup.is_open());
}

#[test]
pub(super) fn with_several_popups_open_a_click_closes_only_the_ones_it_lies_outside_of() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    // The app normally keeps pop-ups mutually exclusive, but open two at
    // once to exercise the edge case
    app.help_popup.open();
    app.save_popup.open(None);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    assert!(app.help_popup.is_open() && app.save_popup.is_open());

    // Inside the help pop-up but outside the (smaller, centered) save one:
    // only the save pop-up closes
    let (col, row) =
        cell_where(|c, r| app.help_popup.contains(c, r) && !app.save_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(
        !app.save_popup.is_open(),
        "the click was outside the save pop-up"
    );
    assert!(
        app.help_popup.is_open(),
        "the click was inside the help pop-up"
    );

    // A click outside the remaining pop-up closes it too
    let (col, row) = cell_where(|c, r| !app.help_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.help_popup.is_open());
}

#[test]
pub(super) fn with_several_popups_open_a_click_outside_all_of_them_closes_all_of_them() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.help_popup.open();
    app.editor_popup.open();
    app.breakpoint_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) = cell_where(|c, r| {
        !app.help_popup.contains(c, r)
            && !app.editor_popup.contains(c, r)
            && !app.breakpoint_popup.contains(c, r)
    });
    click(&mut app, &mut sim, col, row);
    assert!(!app.help_popup.is_open());
    assert!(!app.editor_popup.is_open());
    assert!(!app.breakpoint_popup.is_open());
}

#[test]
pub(super) fn a_click_inside_every_open_popup_closes_none_of_them() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.help_popup.open();
    app.save_popup.open(None);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    let (col, row) =
        cell_where(|c, r| app.help_popup.contains(c, r) && app.save_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(app.help_popup.is_open());
    assert!(app.save_popup.is_open());
}

#[test]
pub(super) fn a_double_click_while_a_popup_is_open_closes_it_and_still_opens_the_target() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.help_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Double-click on a store row, outside the help pop-up (in the bits
    // column, so the word editor is the double-click target): the first
    // click closes the pop-up, the second completes the double-click
    let (col, row) =
        cell_where(|c, r| (5..36).contains(&r) && c < 50 && !app.help_popup.contains(c, r));
    double_click(&mut app, &mut sim, col, row);
    assert!(
        !app.help_popup.is_open(),
        "the click outside closed the pop-up"
    );
    assert!(
        app.editor_popup.is_open(),
        "the double-click still opened the word editor"
    );
    assert!(!app.comment_editor.is_open());
    assert!(!app.picker.is_open());
}

#[test]
pub(super) fn a_click_inside_a_popup_does_not_open_another_popup_on_top_of_it() {
    let mut app = plain_app();
    let mut sim = Simulator::new();
    app.help_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Double-click inside the help pop-up: the clicks are consumed by it,
    // so no editor opens and the pop-up stays open
    let (col, row) = cell_where(|c, r| app.help_popup.contains(c, r));
    double_click(&mut app, &mut sim, col, row);
    assert!(app.help_popup.is_open());
    assert!(!app.editor_popup.is_open());
    assert!(!app.comment_editor.is_open());
    assert!(!app.picker.is_open());
}

#[test]
pub(super) fn a_click_on_the_timeline_while_a_popup_is_open_closes_it_and_scrubs() {
    let mut app = plain_app();
    let mut sim = looping_sim(10);
    sim.stop_flag = true;
    app.help_popup.open();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    let timeline_row = app.timeline_area.y;

    // The click closes the pop-up (it lands outside of it) and then falls
    // through to the timeline, starting a scrub
    let (col, row) = cell_where(|c, r| r == timeline_row && !app.help_popup.contains(c, r));
    click(&mut app, &mut sim, col, row);
    assert!(!app.help_popup.is_open());
    assert!(app.scrubbing, "the click fell through to the timeline");
    assert_ne!(10, sim.cycle_count, "the history was scrubbed");
}
