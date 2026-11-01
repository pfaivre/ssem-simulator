//! Tests for the mnemonic/data picker: double-clicking the mnemonic or the
//! data field of a store row opens a side list, and clicking (or selecting
//! with the arrows + Enter) rewrites that field of the word.

use super::super::layout::{DATA_WIDTH, MNEMONIC_WIDTH, data_column, mnemonic_column};
use super::super::popups::PickerKind;
use super::*;
use crate::ssem::Opcode;

/// An app with a pinned, predictable layout: no flow gutter, no heatmap, so
/// the mnemonic field starts at column 51 and the data field at column 55.
fn picker_app() -> App {
    let mut app = App::new();
    app.flow = FlowMode::Off;
    app.heatmap = HeatmapMode::Off;
    app
}

/// A simulator whose word 3 is `LDN 24`.
fn sim_with_word() -> Simulator {
    let mut sim = Simulator::new();
    sim.store.words[3] = Store::parse_word("LDN 24").unwrap();
    sim
}

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

/// Screen cell of the picker entry `entry`, found by probing the hit-test
/// over the whole screen (the picker must have been drawn once).
fn entry_cell(app: &App, entry: usize) -> (u16, u16) {
    for row in 0..40u16 {
        for col in 0..100u16 {
            if app.picker.entry_at(col, row) == Some(entry) {
                return (col, row);
            }
        }
    }
    panic!("picker entry {entry} not found on screen");
}

/// Render once, then double-click the mnemonic field of word 3 (screen row
/// 5 + 3 = 8) and return the app/sim with the picker open.
fn open_mnemonic_picker(app: &mut App, sim: &mut Simulator) {
    let snap = Snapshot::from_sim(sim);
    let _ = render(app, &snap, 100, 40);
    let col = mnemonic_column(HeatmapMode::Off) + 1;
    double_click(app, sim, col as u16, 8);
    assert!(app.picker.is_open());
}

#[test]
pub(super) fn double_click_on_the_mnemonic_opens_the_picker() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);

    assert!(app.picker.is_open());
    assert_eq!(PickerKind::Mnemonic, app.picker.kind());
    // The word editor and the comment editor are not involved
    assert!(!app.editor_popup.is_open());
    assert!(!app.comment_editor.is_open());
}

#[test]
pub(super) fn double_click_on_the_data_field_opens_the_data_picker() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // The data field is the 3 cells right after the mnemonic (col 55..58);
    // word 3 is on screen row 8
    double_click(&mut app, &mut sim, 56, 8);
    assert!(app.picker.is_open());
    assert_eq!(PickerKind::Data, app.picker.kind());
    assert!(!app.editor_popup.is_open());
}

#[test]
pub(super) fn double_click_on_the_bits_still_opens_the_word_editor() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 10, 8);
    assert!(app.editor_popup.is_open());
    assert!(!app.picker.is_open());
}

#[test]
pub(super) fn the_field_columns_follow_the_draw_geometry() {
    // No heatmap: pointer (1) + bits (32) + suffix (18) = 51
    assert_eq!(51, mnemonic_column(HeatmapMode::Off));
    assert_eq!(55, data_column(HeatmapMode::Off));
    assert_eq!(51 + MNEMONIC_WIDTH, data_column(HeatmapMode::Off));
    // The heatmap bars are spliced in right after the pointer column, so
    // they shift the mnemonic/data fields by their full extra width
    assert_eq!(
        51 + HeatmapMode::Exec.extra_width() as usize,
        mnemonic_column(HeatmapMode::Exec)
    );
    assert_eq!(
        55 + HeatmapMode::Rw.extra_width() as usize,
        data_column(HeatmapMode::Rw)
    );
    assert_eq!(DATA_WIDTH, 3);
}

#[test]
pub(super) fn mnemonic_cells_shift_with_the_heatmap() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    app.heatmap = HeatmapMode::Exec; // +8 cells
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Column 52 is the mnemonic only without the heatmap; with it the
    // mnemonic starts at 59
    double_click(&mut app, &mut sim, 52, 8);
    assert!(
        !app.picker.is_open(),
        "col 52 is in the bits with the heatmap"
    );
    double_click(&mut app, &mut sim, 60, 8);
    assert!(app.picker.is_open());
    assert_eq!(PickerKind::Mnemonic, app.picker.kind());

    // And the data field moved to 63..66
    let mut app = picker_app();
    app.heatmap = HeatmapMode::Exec;
    let _ = render(&mut app, &snap, 100, 40);
    double_click(&mut app, &mut sim, 64, 8);
    assert!(app.picker.is_open());
    assert_eq!(PickerKind::Data, app.picker.kind());
}

#[test]
pub(super) fn clicking_an_opcode_entry_rewrites_the_mnemonic_keeping_the_data() {
    let mut sim = sim_with_word(); // word 3 = LDN 24
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);

    // Draw so the entry cells are known, then click STO (index 3)
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    let (col, row) = entry_cell(&app, 3);
    click(&mut app, &mut sim, col, row);

    assert!(!app.picker.is_open(), "clicking an entry closes the picker");
    assert_eq!(
        Store::parse_word("STO 24").unwrap(),
        sim.store.words[3],
        "only the opcode changed, the data field is kept"
    );
}

#[test]
pub(super) fn clicking_a_data_entry_rewrites_the_data_keeping_the_opcode() {
    let mut sim = sim_with_word(); // word 3 = LDN 24
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    double_click(&mut app, &mut sim, 56, 8); // data field of word 3
    assert!(app.picker.is_open());

    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    let (col, row) = entry_cell(&app, 7);
    click(&mut app, &mut sim, col, row);

    assert!(!app.picker.is_open());
    assert_eq!(
        Store::parse_word("LDN 7").unwrap(),
        sim.store.words[3],
        "only the data field changed, the opcode is kept"
    );
}

#[test]
pub(super) fn bits_outside_the_two_fields_are_preserved() {
    let mut sim = Simulator::new();
    // A "NUM"-style word: bits 16+ set on top of LDN 24
    sim.store.words[3] = (0b101 << 16) | Store::parse_word("LDN 24").unwrap();
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Pick CMP (index 6) in the mnemonic list
    double_click(&mut app, &mut sim, 52, 8);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    let (col, row) = entry_cell(&app, 6);
    click(&mut app, &mut sim, col, row);
    assert_eq!(
        (0b101 << 16) | Store::parse_word("CMP 24").unwrap(),
        sim.store.words[3]
    );

    // Pick 12 in the data list
    double_click(&mut app, &mut sim, 56, 8);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    let (col, row) = entry_cell(&app, 12);
    click(&mut app, &mut sim, col, row);
    assert_eq!(
        (0b101 << 16) | Store::parse_word("CMP 12").unwrap(),
        sim.store.words[3]
    );
}

#[test]
pub(super) fn arrow_keys_and_enter_apply_the_selection() {
    let mut sim = Simulator::new(); // word 0 = 0 = JMP 0
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 52, 5); // mnemonic of word 0
    assert!(app.picker.is_open());
    // The selection starts on the current opcode (JMP); Down moves to JRP
    press(&mut app, &mut sim, KeyCode::Down);
    press(&mut app, &mut sim, KeyCode::Enter);

    assert!(!app.picker.is_open());
    assert_eq!(Store::parse_word("JRP 0").unwrap(), sim.store.words[0]);
}

#[test]
pub(super) fn grid_arrows_move_over_the_data_entries() {
    let mut sim = sim_with_word(); // word 3 = LDN 24, selection starts on 24
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 56, 8); // data picker, selected = 24
    // 24 is row 3 col 0 of the 8-column grid: Left clamps, Up goes to 16
    press(&mut app, &mut sim, KeyCode::Left);
    press(&mut app, &mut sim, KeyCode::Up);
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(Store::parse_word("LDN 16").unwrap(), sim.store.words[3]);

    // Down wraps the row: from 16 Down goes to 24, Right to 25
    double_click(&mut app, &mut sim, 56, 8);
    press(&mut app, &mut sim, KeyCode::Down);
    press(&mut app, &mut sim, KeyCode::Right);
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(Store::parse_word("LDN 25").unwrap(), sim.store.words[3]);
}

#[test]
pub(super) fn esc_closes_the_picker_without_changes() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);

    press(&mut app, &mut sim, KeyCode::Down);
    press(&mut app, &mut sim, KeyCode::Esc);

    assert!(!app.picker.is_open());
    assert_eq!(Store::parse_word("LDN 24").unwrap(), sim.store.words[3]);
}

#[test]
pub(super) fn clicking_outside_the_picker_closes_it_without_changes() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    click(&mut app, &mut sim, 5, 5); // far from the side list
    assert!(!app.picker.is_open());
    assert_eq!(Store::parse_word("LDN 24").unwrap(), sim.store.words[3]);
}

#[test]
pub(super) fn keys_are_routed_to_the_picker_while_it_is_open() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);

    press(&mut app, &mut sim, KeyCode::Char('q'));
    press(&mut app, &mut sim, KeyCode::Char('c'));

    // Neither Q (quit) nor C (comments toggle) fired
    assert!(!app.should_quit);
    assert!(app.show_comments);
    assert!(app.picker.is_open());
}

#[test]
pub(super) fn mouse_is_modal_while_the_picker_is_open() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // A double-click on the store text while the picker is open: the first
    // click closes the picker, the second is a plain click — no editor opens
    double_click(&mut app, &mut sim, 10, 8);
    assert!(!app.picker.is_open());
    assert!(!app.editor_popup.is_open());
    assert!(!app.comment_editor.is_open());
}

#[test]
pub(super) fn the_mnemonic_list_is_drawn_on_the_side_with_the_current_marked() {
    let mut sim = sim_with_word(); // word 3 = LDN 24
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim);

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);

    // The border title names the field and the word, on the right side
    let mut titled = false;
    for row in 0..40u16 {
        if row_text(&buffer, row).contains("MNEMONIC [03]") {
            titled = true;
        }
    }
    assert!(titled, "the picker title is drawn");

    // All eight opcodes are listed, the current one (LDN) marked with '>'
    for (i, op) in Opcode::ALL.iter().enumerate() {
        let (col, row) = entry_cell(&app, i);
        let text = row_text(&buffer, row);
        let _ = col;
        let expected = if *op == Opcode::LDN {
            format!(">{:<4}", op.to_string())
        } else {
            format!(" {:<4}", op.to_string())
        };
        assert!(
            text.contains(&expected),
            "entry {op} drawn as {expected:?}: {text:?}"
        );
    }

    // The list sits just right of the store text: border at column
    // STORE_TEXT_WIDTH (58), first entry 2 cells later (border + margin)
    let (col, _) = entry_cell(&app, 0);
    assert_eq!(
        (STORE_TEXT_WIDTH + 2) as u16,
        col,
        "picker entries start right after the store text"
    );

    // The selected entry (the current opcode, LDN = index 2) is highlighted
    let (sel_col, sel_row) = entry_cell(&app, 2);
    assert!(
        buffer[(sel_col, sel_row)]
            .modifier
            .contains(Modifier::REVERSED),
        "the selected entry is reversed"
    );
    let (other_col, other_row) = entry_cell(&app, 3);
    assert!(
        !buffer[(other_col, other_row)]
            .modifier
            .contains(Modifier::REVERSED),
        "the other entries are not"
    );
}

#[test]
pub(super) fn the_data_list_is_drawn_as_a_grid_with_the_current_marked() {
    let mut sim = sim_with_word(); // word 3 = LDN 24
    let mut app = picker_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);
    double_click(&mut app, &mut sim, 56, 8); // data picker of word 3

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);

    let mut titled = false;
    for row in 0..40u16 {
        if row_text(&buffer, row).contains("DATA [03]") {
            titled = true;
        }
    }
    assert!(titled, "the picker title is drawn");

    // The current value 24 is marked with '>'
    let (_, row) = entry_cell(&app, 24);
    assert!(
        row_text(&buffer, row).contains(">24"),
        "row: {:?}",
        row_text(&buffer, row)
    );
    // The grid has 8 entries per row: entry 8 is one row below entry 0
    let (c0, r0) = entry_cell(&app, 0);
    let (c8, r8) = entry_cell(&app, 8);
    assert_eq!(r0 + 1, r8);
    assert_eq!(c0, c8);
}

#[test]
pub(super) fn the_list_anchors_right_of_the_store_text_next_to_the_word() {
    let mut sim = sim_with_word(); // word 3 = LDN 24
    let mut app = picker_app();
    open_mnemonic_picker(&mut app, &mut sim); // anchored on screen row 8

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);

    // The edited row's disassembly is left intact by the popup: word 3 is
    // on screen row 8, its mnemonic at columns 51..55
    let row8 = row_text(&buffer, 8);
    let disasm: String = row8.chars().skip(51).take(7).collect();
    assert_eq!("LDN  24", disasm, "row8: {row8:?}");
    // The popup border starts at column 58 (STORE_TEXT_WIDTH), right after
    // the data field
    assert_eq!("╭", buffer[(STORE_TEXT_WIDTH as u16, 3)].symbol());
}

#[test]
pub(super) fn the_anchor_follows_the_heatmap_width() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    app.heatmap = HeatmapMode::Exec; // +8 cells
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 120, 40);

    double_click(&mut app, &mut sim, 60, 8); // mnemonic at 59..63 with the heatmap
    assert!(app.picker.is_open());
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 120, 40);

    // Anchor = 58 + 8 = 66: border at 66, first entry at 68
    let (col, _) = entry_cell(&app, 0);
    assert_eq!((STORE_TEXT_WIDTH + 8 + 2) as u16, col);
}

#[test]
pub(super) fn the_list_is_shifted_left_when_it_would_overflow_the_right_edge() {
    let mut sim = sim_with_word();
    let mut app = picker_app();
    // Width 70: anchor 58 + popup width 36 would end at 94, so the popup is
    // clamped to end exactly at the right edge (x = 70 - 36 = 34)
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 70, 40);
    double_click(&mut app, &mut sim, 52, 8);
    assert!(app.picker.is_open());

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 70, 40);
    let (col, _) = entry_cell(&app, 0);
    assert_eq!(70 - 36 + 2, col, "first entry at the clamped position");
    // The popup still fits on screen: its last column is the right edge
    let title_row = (0..40u16)
        .find(|&r| row_text(&buffer, r).contains("MNEMONIC [03]"))
        .unwrap();
    let row = row_text(&buffer, title_row);
    assert_eq!('╮', row.chars().nth(69).unwrap(), "row: {row:?}");
}
