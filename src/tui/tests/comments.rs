//! Tests for the source-comment column (C key): parsing is covered in
//! `ssem::store::tests`, these cover the rendering.

use super::*;

/// Screen column where the comment column starts: after the store text and
/// the flow gutter (the pointer arrow in data mode, the graph in control
/// mode, nothing when the flow view is off).
fn comment_column(flow: FlowMode) -> usize {
    STORE_TEXT_WIDTH + flow_gutter_width(flow) as usize
}

/// A snapshot whose store words carry the given source comments.
fn comment_snapshot(comments: [Option<String>; 32]) -> Snapshot {
    Snapshot {
        comments: Arc::new(comments),
        ..snapshot_with_history(0)
    }
}

#[test]
pub(super) fn comment_column_width_is_the_widest_comment_capped() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    assert_eq!(
        0,
        comment_column_width(&comments),
        "no comments, zero width"
    );
    comments[1] = Some("short".into());
    comments[2] = Some("a bit longer".into());
    assert_eq!("a bit longer".len(), comment_column_width(&comments));
    comments[3] = Some("z".repeat(100));
    assert_eq!(MAX_COMMENT_WIDTH, comment_column_width(&comments));
}

#[test]
pub(super) fn comments_are_shown_by_default_next_to_each_word() {
    // Comments are stored verbatim after the ';', leading space included,
    // exactly as the assembler parses them
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[1] = Some(" A = -a".into());
    comments[4] = Some("   x".into());
    let snap = comment_snapshot(comments);

    let mut app = App::new();
    app.flow = FlowMode::Data; // pin the flow mode: the comment column follows the gutter
    assert!(app.show_comments, "the comment column is on by default");
    // Wide enough that the status-bar label is not truncated by the screen edge
    let buffer = render(&mut app, &snap, 120, 40);

    // Store line i is drawn on screen row 5 + i; the comment starts after
    // the store text and the (empty) arrow gutter, dimmed, indentation kept
    let col = comment_column(app.flow);
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(" ; A = -a"), "row1: {row1:?}");
    let row4: String = row_text(&buffer, 9).chars().skip(col).collect();
    assert!(row4.starts_with(" ;   x"), "row4: {row4:?}");
    // A word without a comment gets blank padding, keeping the column aligned
    let row2: String = row_text(&buffer, 7).chars().skip(col).collect();
    assert!(
        row2.starts_with(&" ".repeat(" A = -a".len() + 2)),
        "row2: {row2:?}"
    );
    // The status bar advertises the column
    assert!(row_text(&buffer, buffer.area.height - 1).contains("Comments"));
}

#[test]
pub(super) fn c_key_toggles_the_comment_column() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[1] = Some(" A = -a".into());
    let snap = comment_snapshot(comments);

    let mut sim = Simulator::new();
    let mut app = App::new();
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(row_text(&buffer, 6).contains("A = -a"));

    app.handle_key(
        KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE),
        &mut sim,
    );
    assert!(!app.show_comments);
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(!row_text(&buffer, 6).contains("A = -a"));
    assert!(!row_text(&buffer, buffer.area.height - 1).contains("Comments"));

    // Uppercase toggles back on
    app.handle_key(
        KeyEvent::new(KeyCode::Char('C'), KeyModifiers::NONE),
        &mut sim,
    );
    assert!(app.show_comments);
    let buffer = render(&mut app, &snap, 100, 40);
    assert!(row_text(&buffer, 6).contains("A = -a"));
}

#[test]
pub(super) fn no_comments_still_draws_a_minimal_column_when_enabled() {
    let snap = snapshot_with_history(0); // all comments None
    let mut app = App::new();
    app.flow = FlowMode::Off; // pin the layout: the comment column starts at 58
    assert!(app.show_comments);
    let buffer = render(&mut app, &snap, 100, 40);

    // The status bar advertises the column...
    assert!(row_text(&buffer, buffer.area.height - 1).contains("Comments"));
    // ...and every store row shows a dim " ;" gutter as the double-click
    // target for adding the first comment (word 0 is on screen row 5)
    let col = comment_column(app.flow);
    let row0: String = row_text(&buffer, 5).chars().skip(col).collect();
    assert!(row0.starts_with(" ;"), "row0: {row0:?}");
    // The store text is not shifted: the bits start right after the pointer
    let chars: Vec<char> = row_text(&buffer, 10).chars().collect();
    assert_eq!('.', chars[1]);
}

#[test]
pub(super) fn long_comments_are_truncated_to_the_column_width() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[3] = Some("y".repeat(100));
    let snap = comment_snapshot(comments);

    let mut app = App::new();
    app.flow = FlowMode::Data; // 4-cell arrow gutter, as assumed below
    // Wide enough that the terminal crop does not kick in before the
    // MAX_COMMENT_WIDTH cap: 58 + 4 gutter + 2 + 80 = 144 cells needed
    let buffer = render(&mut app, &snap, 160, 40);
    // row_text returns the full row width, so compare only the comment span
    let row: String = row_text(&buffer, 8)
        .chars()
        .skip(comment_column(app.flow))
        .take(2 + MAX_COMMENT_WIDTH)
        .collect();
    assert_eq!(format!(" ;{}", "y".repeat(MAX_COMMENT_WIDTH)), row);
}

#[test]
pub(super) fn comment_column_is_cropped_when_the_terminal_is_narrow() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[0] = Some(" a comment".into()); // 10 cells with the leading space
    let snap = comment_snapshot(comments);

    let mut app = App::new();
    app.flow = FlowMode::Off;
    let col = comment_column(app.flow);

    // The full comment span needs the column + " ;" (2) + 10 cells
    let full = col + 2 + " a comment".len();

    // One cell short: the comment is cropped by one character, not hidden
    let buffer = render(&mut app, &snap, (full - 1) as u16, 40);
    let cropped: String = row_text(&buffer, 5).chars().skip(col).take(2 + 9).collect();
    assert_eq!(" ; a commen", cropped);
    assert!(!row_text(&buffer, 5).contains("a comment"));

    // No room left for any comment text: the column disappears entirely
    // (a single cell past the store text is less than the " ;" prefix)
    let buffer = render(&mut app, &snap, STORE_TEXT_WIDTH as u16 + 1, 40);
    assert!(!row_text(&buffer, 5).contains(';'));
    assert!(!row_text(&buffer, buffer.area.height - 1).contains("Comments"));

    // Wide enough: the full comment and the status-bar label are drawn
    let buffer = render(&mut app, &snap, (full + 20) as u16, 40);
    assert!(row_text(&buffer, 5).contains("; a comment"));
    assert!(row_text(&buffer, buffer.area.height - 1).contains("Comments"));
}

#[test]
pub(super) fn comment_crop_accounts_for_the_heatmap_width() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[0] = Some(" a comment".into()); // 10 cells with the leading space
    let snap = comment_snapshot(comments);

    let mut app = App::new();
    app.flow = FlowMode::Off;
    app.heatmap = HeatmapMode::Rw;

    // The r/w heatmap fits at this width (>= STORE_TEXT_WIDTH + its extra width)
    // and eats into the room left for comments:
    // 80 - STORE_TEXT_WIDTH - r/w extra (11) - " ;" (2) = 9 cells
    let extra = HeatmapMode::Rw.extra_width() as usize;
    let col = comment_column(app.flow) + extra;
    let buffer = render(&mut app, &snap, 80, 40);
    let cropped: String = row_text(&buffer, 5).chars().skip(col).take(2 + 9).collect();
    assert_eq!(" ; a commen", cropped);
}

#[test]
pub(super) fn arrow_gutter_stays_left_of_the_comment_column() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 10").unwrap();
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[2] = Some(" store A to 10".into()); // 14 cells with the leading space
    let mut snap = pointer_snapshot(2, store);
    snap.comments = Arc::new(comments);

    let mut app = App::new();
    app.flow = FlowMode::Data; // the pointer arrow gutter under test
    let col = comment_column(app.flow);

    // Wide: the arrow keeps its usual columns, the comment follows the gutter
    let buffer = render(&mut app, &snap, 100, 40);
    let source = row_text(&buffer, 3); // accumulator row
    assert_eq!(
        '╮',
        source.chars().nth(GUTTER_COLUMN).unwrap(),
        "row: {source:?}"
    );
    let target = row_text(&buffer, 15); // line 10
    assert_eq!(
        '◀',
        target.chars().nth(ARROWHEAD_COLUMN).unwrap(),
        "row: {target:?}"
    );
    let comment: String = row_text(&buffer, 7)
        .chars()
        .skip(col)
        .take(2 + 14)
        .collect();
    assert_eq!(" ; store A to 10", comment);

    // Narrow: the comment crops (70 - 58 - 4 gutter - 2 = 6 cells of text)
    // but the arrow columns are unchanged
    let buffer = render(&mut app, &snap, 70, 40);
    let target = row_text(&buffer, 15);
    assert_eq!(
        '◀',
        target.chars().nth(ARROWHEAD_COLUMN).unwrap(),
        "row: {target:?}"
    );
    let cropped: String = row_text(&buffer, 7).chars().skip(col).take(2 + 6).collect();
    assert_eq!(" ; store", cropped);
}

#[test]
pub(super) fn snapshot_carries_the_store_comments() {
    let mut sim = Simulator::new();
    Arc::make_mut(&mut sim.store.comments)[5] = Some("fifth word".into());
    let snap = Snapshot::from_sim(&sim);
    assert_eq!(Some("fifth word".to_string()), snap.comments[5]);
    assert_eq!(None, snap.comments[0]);
}

#[test]
pub(super) fn comment_column_reaches_the_right_edge_in_every_flow_mode() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[0] = Some("x".repeat(100)); // cropped by the terminal, not by the cap
    let snap = comment_snapshot(comments);

    for flow in [FlowMode::Control, FlowMode::Data, FlowMode::Off] {
        let mut app = App::new();
        app.flow = flow;
        for width in [80u16, 100, 120] {
            let buffer = render(&mut app, &snap, width, 40);
            // Store line 0 is drawn on screen row 5: when the terminal is
            // the limiting factor, the comment must fill the row up to the
            // last cell, leaving no blank gap on the right of the window
            let row = row_text(&buffer, 5);
            assert_eq!(
                width as usize,
                row.trim_end().chars().count(),
                "flow={flow:?} width={width} row='{row}'"
            );
        }
    }
}

// --- Inline comment editing (double-click a comment) ---

/// A simulator whose store carries comments on words 1 and 4.
fn sim_with_comments() -> Simulator {
    let mut sim = Simulator::new();
    Arc::make_mut(&mut sim.store.comments)[1] = Some(" A = -a".into());
    Arc::make_mut(&mut sim.store.comments)[4] = Some("   x".into());
    sim
}

/// An app with a pinned, predictable layout: no flow gutter, no heatmap,
/// so the comment column starts right after the 58-cell store text.
fn comment_edit_app() -> App {
    let mut app = App::new();
    app.flow = FlowMode::Off;
    app.heatmap = HeatmapMode::Off;
    app
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

/// Type a string, one key press per character.
fn type_str(app: &mut App, sim: &mut Simulator, s: &str) {
    for c in s.chars() {
        press(app, sim, KeyCode::Char(c));
    }
}

#[test]
pub(super) fn double_click_on_a_comment_opens_the_inline_editor() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    // Render once so last_area is known to the mouse hit-testing
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Word 1 is on screen row 6; with no gutter the comment column starts
    // at column 58 (STORE_TEXT_WIDTH)
    double_click(&mut app, &mut sim, 60, 6);
    assert!(app.comment_editor.is_open());
    assert!(app.comment_editor.is_open_for(1));
    assert!(!app.comment_editor.is_open_for(4));
    // Prefilled with the word's current comment, verbatim
    assert_eq!(" A = -a", app.comment_editor.buffer());
    // The word editor pop-up is not involved
    assert!(!app.editor_popup.is_open());
}

#[test]
pub(super) fn double_click_in_the_flow_gutter_is_not_a_comment_cell() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    app.flow = FlowMode::Data; // 4-cell arrow gutter: comments start at 62
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Column 60 is inside the gutter, not the comment column
    double_click(&mut app, &mut sim, 60, 6);
    assert!(!app.comment_editor.is_open());
    assert!(app.editor_popup.is_open());

    // Column 63 is past the gutter: the comment editor opens
    double_click(&mut app, &mut sim, 63, 6);
    assert!(app.comment_editor.is_open_for(1));
    assert!(!app.editor_popup.is_open());
}

#[test]
pub(super) fn double_click_on_the_store_text_still_opens_the_word_editor() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 10, 6);
    assert!(app.editor_popup.is_open());
    assert!(!app.comment_editor.is_open());
}

#[test]
pub(super) fn double_click_on_a_commentless_word_opens_an_empty_editor() {
    let mut sim = sim_with_comments(); // word 0 has no comment
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 5); // word 0 is on screen row 5
    assert!(app.comment_editor.is_open_for(0));
    assert_eq!("", app.comment_editor.buffer());

    type_str(&mut app, &mut sim, "hello");
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(Some("hello".to_string()), sim.store.comments[0]);
}

#[test]
pub(super) fn double_clicking_another_comment_moves_the_editor() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6); // word 1
    type_str(&mut app, &mut sim, "ZZZ");
    double_click(&mut app, &mut sim, 60, 9); // word 4 is on screen row 9
    assert!(app.comment_editor.is_open_for(4));
    assert!(!app.comment_editor.is_open_for(1));
    // The new word's own comment is the buffer, the typed text is dropped
    assert_eq!("   x", app.comment_editor.buffer());
    // Word 1's comment was not modified (no Enter was pressed on it)
    assert_eq!(Some(" A = -a".to_string()), sim.store.comments[1]);
}

#[test]
pub(super) fn enter_validates_the_edit_into_the_store() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6); // word 1, buffer " A = -a"
    for _ in 0..7 {
        press(&mut app, &mut sim, KeyCode::Backspace);
    }
    assert_eq!("", app.comment_editor.buffer());
    type_str(&mut app, &mut sim, " new text");
    press(&mut app, &mut sim, KeyCode::Enter);

    assert!(!app.comment_editor.is_open(), "Enter closes the editor");
    assert_eq!(Some(" new text".to_string()), sim.store.comments[1]);
    // The next snapshot sees the new comment
    let snap = Snapshot::from_sim(&sim);
    assert_eq!(Some(" new text".to_string()), snap.comments[1]);
    // And saving keeps it
    assert!(sim.store.to_asm_string().contains("; new text"));
}

#[test]
pub(super) fn a_blank_edit_clears_the_comment() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6);
    for _ in 0..7 {
        press(&mut app, &mut sim, KeyCode::Backspace);
    }
    type_str(&mut app, &mut sim, "   "); // whitespace only
    press(&mut app, &mut sim, KeyCode::Enter);

    assert_eq!(None, sim.store.comments[1]);
}

#[test]
pub(super) fn esc_cancels_the_edit() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6);
    type_str(&mut app, &mut sim, "ZZZ");
    press(&mut app, &mut sim, KeyCode::Esc);

    assert!(!app.comment_editor.is_open());
    assert_eq!(
        Some(" A = -a".to_string()),
        sim.store.comments[1],
        "Esc must not apply the edit"
    );
}

#[test]
pub(super) fn keys_are_routed_to_the_editor_while_it_is_open() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6);
    press(&mut app, &mut sim, KeyCode::Char('q'));
    press(&mut app, &mut sim, KeyCode::Char('c'));

    // Neither Q (quit) nor C (comments toggle) fired: they went into the buffer
    assert!(!app.should_quit);
    assert!(app.show_comments);
    assert_eq!(" A = -aqc", app.comment_editor.buffer());
}

#[test]
pub(super) fn held_backspace_repeats_while_editing() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6);
    let repeat =
        KeyEvent::new_with_kind(KeyCode::Backspace, KeyModifiers::NONE, KeyEventKind::Repeat);
    for _ in 0..3 {
        app.handle_key(repeat, &mut sim);
    }
    assert_eq!(" A =", app.comment_editor.buffer());
}

#[test]
pub(super) fn the_inline_box_is_drawn_on_the_edited_row() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    double_click(&mut app, &mut sim, 60, 6);
    type_str(&mut app, &mut sim, "XYZ");

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);
    let col = comment_column(app.flow);

    // The edited row shows the buffer with a cursor, highlighted (reversed)
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(" ; A = -aXYZ_"), "row1: {row1:?}");
    assert!(
        buffer[(col as u16, 6)]
            .modifier
            .contains(Modifier::REVERSED),
        "the box is highlighted"
    );
    // The other rows keep the plain dimmed comment
    let row4: String = row_text(&buffer, 9).chars().skip(col).collect();
    assert!(row4.starts_with(" ;   x"), "row4: {row4:?}");
    assert!(
        !buffer[(col as u16, 9)]
            .modifier
            .contains(Modifier::REVERSED)
    );
}

#[test]
pub(super) fn the_visual_cursor_sits_at_the_editing_position() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Open on word 4 ("   x", 4 chars): the cursor must sit right after
    // the text, at the editing position — not at the right edge of the box
    double_click(&mut app, &mut sim, 60, 9);
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);
    let col = comment_column(app.flow);
    let row4: String = row_text(&buffer, 9).chars().skip(col).collect();
    assert!(row4.starts_with(" ;   x_"), "row4: {row4:?}");
    let chars: Vec<char> = row4.chars().collect();
    // The cell after the cursor is padding, not another cursor
    assert_eq!(' ', chars[2 + 4 + 1], "row4: {row4:?}");

    // Typing moves the cursor along with the text
    type_str(&mut app, &mut sim, "ab");
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);
    let row4: String = row_text(&buffer, 9).chars().skip(col).collect();
    assert!(row4.starts_with(" ;   xab_"), "row4: {row4:?}");
}

#[test]
pub(super) fn a_long_buffer_shows_its_tail_with_the_cursor_at_the_end() {
    let mut sim = sim_with_comments(); // word 1: " A = -a"
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    // Width 70 leaves 70 - 58 - 2 = 10 cells for the comment column
    let _ = render(&mut app, &snap, 70, 40);

    double_click(&mut app, &mut sim, 60, 6);
    type_str(&mut app, &mut sim, &"0123456789".repeat(2)); // buffer: 7 + 20 = 27 chars

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 70, 40);
    let col = comment_column(app.flow);
    // The column is 10 wide: 9 cells of text (the buffer's tail) + cursor,
    // so the cursor stays at the editing position instead of mid-buffer
    let raw = app.comment_editor.buffer().to_string();
    let tail: String = raw.chars().skip(raw.chars().count() - 9).collect();
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(&format!(" ;{tail}_")), "row1: {row1:?}");
    assert!(
        !row1.starts_with(" ; A = -a0"),
        "the head is scrolled off: {row1:?}"
    );
}

#[test]
pub(super) fn double_click_with_no_comments_anywhere_opens_an_empty_editor() {
    let mut sim = Simulator::new(); // no comments at all
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Even far right on a store row, a double-click opens the comment
    // editor: the minimal comment column is drawn, so that cell is a
    // comment cell (word 1 is on screen row 6)
    double_click(&mut app, &mut sim, 90, 6);
    assert!(app.comment_editor.is_open());
    assert!(app.comment_editor.is_open_for(1));
    assert!(!app.editor_popup.is_open());
    assert_eq!("", app.comment_editor.buffer());

    // Typing and Enter adds the first comment to the store
    type_str(&mut app, &mut sim, "first comment");
    press(&mut app, &mut sim, KeyCode::Enter);
    assert!(!app.comment_editor.is_open());
    assert_eq!(Some("first comment".to_string()), sim.store.comments[1]);

    // Once a comment exists, the column grows to fit it and shows the text
    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);
    let col = comment_column(app.flow);
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(" ;first comment"), "row1: {row1:?}");
}

/// A Ctrl+Backspace key press, as CSI-u/kitty terminals report it.
fn ctrl_backspace() -> KeyEvent {
    KeyEvent::new_with_kind(
        KeyCode::Backspace,
        KeyModifiers::CONTROL,
        KeyEventKind::Press,
    )
}

#[test]
pub(super) fn ctrl_backspace_erases_a_whole_word() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Buffer " A = -a": each Ctrl+Backspace erases one whitespace run
    // plus one word run
    double_click(&mut app, &mut sim, 60, 6);
    app.handle_key(ctrl_backspace(), &mut sim);
    assert_eq!(" A = ", app.comment_editor.buffer());
    app.handle_key(ctrl_backspace(), &mut sim);
    assert_eq!(" A ", app.comment_editor.buffer());
    app.handle_key(ctrl_backspace(), &mut sim);
    assert_eq!(" ", app.comment_editor.buffer());
    app.handle_key(ctrl_backspace(), &mut sim);
    assert_eq!("", app.comment_editor.buffer());
    // On an empty buffer it is a no-op
    app.handle_key(ctrl_backspace(), &mut sim);
    assert_eq!("", app.comment_editor.buffer());

    // "hello world  |" loses the spaces and the word in one press
    type_str(&mut app, &mut sim, "hello world  ");
    app.handle_key(ctrl_backspace(), &mut sim);
    assert_eq!("hello ", app.comment_editor.buffer());

    // The raw control-char variant (0x7F + CONTROL) does the same
    type_str(&mut app, &mut sim, "abc");
    app.handle_key(
        KeyEvent::new_with_kind(
            KeyCode::Char('\u{7f}'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
        &mut sim,
    );
    assert_eq!("hello ", app.comment_editor.buffer());

    // Terminals sending Ctrl+Backspace as byte 0x08 report it as
    // 'h' + CONTROL (0x08 is Ctrl+H): it must erase the word, not type 'h'
    type_str(&mut app, &mut sim, "xy");
    app.handle_key(
        KeyEvent::new_with_kind(
            KeyCode::Char('h'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
        &mut sim,
    );
    assert_eq!("hello ", app.comment_editor.buffer());

    // Other Ctrl+letter combos have no binding: ignored, not typed
    app.handle_key(
        KeyEvent::new_with_kind(
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Press,
        ),
        &mut sim,
    );
    assert_eq!("hello ", app.comment_editor.buffer());

    // Plain Backspace still deletes a single character
    press(&mut app, &mut sim, KeyCode::Backspace);
    assert_eq!("hello", app.comment_editor.buffer());
}

#[test]
pub(super) fn left_right_arrows_move_the_cursor_and_editing_happens_there() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Word 1: buffer " A = -a", cursor starts at the end
    double_click(&mut app, &mut sim, 60, 6);
    assert_eq!(" A = -a", app.comment_editor.buffer());
    assert_eq!(7, app.comment_editor.cursor());

    // Left moves the cursor left, Right moves it right (clamped at the ends)
    for _ in 0..3 {
        press(&mut app, &mut sim, KeyCode::Left);
    }
    assert_eq!(4, app.comment_editor.cursor());
    for _ in 0..10 {
        press(&mut app, &mut sim, KeyCode::Right);
    }
    assert_eq!(7, app.comment_editor.cursor(), "clamped at the end");
    for _ in 0..10 {
        press(&mut app, &mut sim, KeyCode::Left);
    }
    assert_eq!(0, app.comment_editor.cursor(), "clamped at the start");

    // Typing inserts at the cursor; Backspace deletes before the cursor
    for _ in 0..4 {
        press(&mut app, &mut sim, KeyCode::Right);
    }
    type_str(&mut app, &mut sim, "Z");
    assert_eq!(" A =Z -a", app.comment_editor.buffer());
    assert_eq!(5, app.comment_editor.cursor());
    for _ in 0..100 {
        press(&mut app, &mut sim, KeyCode::Left);
    }
    assert_eq!(0, app.comment_editor.cursor());
    // Backspace at the start of the line is a no-op
    press(&mut app, &mut sim, KeyCode::Backspace);
    assert_eq!(" A =Z -a", app.comment_editor.buffer());
    assert_eq!(0, app.comment_editor.cursor());
    // Backspace mid-line deletes the character before the cursor
    for _ in 0..5 {
        press(&mut app, &mut sim, KeyCode::Right);
    }
    press(&mut app, &mut sim, KeyCode::Backspace);
    assert_eq!(" A = -a", app.comment_editor.buffer());
    assert_eq!(4, app.comment_editor.cursor());
}

#[test]
pub(super) fn up_down_edit_the_comment_above_or_below_at_the_same_cursor() {
    let mut sim = Simulator::new();
    Arc::make_mut(&mut sim.store.comments)[2] = Some("aaaaaa".into());
    Arc::make_mut(&mut sim.store.comments)[3] = Some("bb".into());
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Word 2 is on screen row 7; move the cursor two cells left
    double_click(&mut app, &mut sim, 60, 7);
    assert!(app.comment_editor.is_open_for(2));
    press(&mut app, &mut sim, KeyCode::Left);
    press(&mut app, &mut sim, KeyCode::Left);
    assert_eq!(4, app.comment_editor.cursor());

    // Down: the edit is committed and the editor opens on word 3, whose
    // shorter comment clamps the cursor to its end
    press(&mut app, &mut sim, KeyCode::Down);
    assert!(app.comment_editor.is_open_for(3));
    assert_eq!("bb", app.comment_editor.buffer());
    assert_eq!(2, app.comment_editor.cursor(), "clamped to the line end");
    assert_eq!(Some("aaaaaa".to_string()), sim.store.comments[2]);

    // Type at that position, then Up: back on word 2, same cursor column
    type_str(&mut app, &mut sim, "X");
    assert_eq!("bbX", app.comment_editor.buffer());
    press(&mut app, &mut sim, KeyCode::Up);
    assert!(app.comment_editor.is_open_for(2));
    assert_eq!("aaaaaa", app.comment_editor.buffer());
    assert_eq!(3, app.comment_editor.cursor());
    // The word 3 edit was committed by the move
    assert_eq!(Some("bbX".to_string()), sim.store.comments[3]);
}

#[test]
pub(super) fn up_on_the_first_word_and_down_on_the_last_are_noops() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Word 0 is on screen row 5
    double_click(&mut app, &mut sim, 60, 5);
    assert!(app.comment_editor.is_open_for(0));
    press(&mut app, &mut sim, KeyCode::Up);
    assert!(app.comment_editor.is_open_for(0), "Up on word 0 stays");
    type_str(&mut app, &mut sim, "top");
    press(&mut app, &mut sim, KeyCode::Down);
    assert!(app.comment_editor.is_open_for(1), "Down moves to word 1");
    assert_eq!(Some("top".to_string()), sim.store.comments[0]);

    // Word 31 is on screen row 36
    double_click(&mut app, &mut sim, 60, 36);
    assert!(app.comment_editor.is_open_for(31));
    press(&mut app, &mut sim, KeyCode::Down);
    assert!(app.comment_editor.is_open_for(31), "Down on word 31 stays");
    assert_eq!("", app.comment_editor.buffer());
}

#[test]
pub(super) fn the_cursor_is_drawn_mid_text_when_moved_left() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Word 1, buffer " A = -a": move the cursor between "= " and "-a"
    double_click(&mut app, &mut sim, 60, 6);
    press(&mut app, &mut sim, KeyCode::Left);
    press(&mut app, &mut sim, KeyCode::Left);
    assert_eq!(5, app.comment_editor.cursor());

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 100, 40);
    let col = comment_column(app.flow);
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(" ; A = _-a"), "row1: {row1:?}");
}

#[test]
pub(super) fn a_long_buffer_keeps_the_cursor_visible_when_moving_left() {
    let mut sim = sim_with_comments(); // word 1: " A = -a"
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    // Width 70 leaves 10 cells for the comment column
    let _ = render(&mut app, &snap, 70, 40);

    double_click(&mut app, &mut sim, 60, 6);
    type_str(&mut app, &mut sim, &"0123456789".repeat(2)); // 27 chars, cursor at 27
    for _ in 0..5 {
        press(&mut app, &mut sim, KeyCode::Left);
    }
    assert_eq!(22, app.comment_editor.cursor());

    let snap = Snapshot::from_sim(&sim);
    let buffer = render(&mut app, &snap, 70, 40);
    let col = comment_column(app.flow);
    // The window slides so the cursor sits mid-box, with text on both
    // sides: chars[18..22] + cursor + chars[22..27]
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(" ;1234_56789"), "row1: {row1:?}");
}

#[test]
pub(super) fn home_end_and_del_edit_at_the_cursor() {
    let mut sim = sim_with_comments();
    let mut app = comment_edit_app();
    let snap = Snapshot::from_sim(&sim);
    let _ = render(&mut app, &snap, 100, 40);

    // Word 1: buffer " A = -a", cursor at the end
    double_click(&mut app, &mut sim, 60, 6);
    assert_eq!(7, app.comment_editor.cursor());

    // Del at the end of the line deletes nothing; End is a no-op there
    press(&mut app, &mut sim, KeyCode::End);
    assert_eq!(7, app.comment_editor.cursor());
    press(&mut app, &mut sim, KeyCode::Delete);
    assert_eq!(" A = -a", app.comment_editor.buffer());
    assert_eq!(7, app.comment_editor.cursor());

    // Home jumps to the start; Del there removes the leading space
    press(&mut app, &mut sim, KeyCode::Home);
    assert_eq!(0, app.comment_editor.cursor());
    press(&mut app, &mut sim, KeyCode::Delete);
    assert_eq!("A = -a", app.comment_editor.buffer());
    assert_eq!(0, app.comment_editor.cursor());

    // Typing at Home inserts at the start; End + typing appends
    type_str(&mut app, &mut sim, "x");
    assert_eq!("xA = -a", app.comment_editor.buffer());
    press(&mut app, &mut sim, KeyCode::End);
    assert_eq!(7, app.comment_editor.cursor());
    type_str(&mut app, &mut sim, "y");
    assert_eq!("xA = -ay", app.comment_editor.buffer());
    assert_eq!(8, app.comment_editor.cursor());

    // Del mid-line removes the character under the cursor, cursor stays put
    press(&mut app, &mut sim, KeyCode::Home);
    press(&mut app, &mut sim, KeyCode::Right);
    press(&mut app, &mut sim, KeyCode::Delete); // deletes 'A'
    assert_eq!("x = -ay", app.comment_editor.buffer());
    assert_eq!(1, app.comment_editor.cursor());

    // Enter commits the edited comment
    press(&mut app, &mut sim, KeyCode::Enter);
    assert_eq!(Some("x = -ay".to_string()), sim.store.comments[1]);
}
