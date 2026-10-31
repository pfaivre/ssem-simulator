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
    assert_eq!(0, comment_column_width(&comments), "no comments, no column");
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
    assert!(app.show_comments, "the comment column is on by default");
    // Wide enough that the top-bar label is not truncated by the screen edge
    let buffer = render(&mut app, &snap, 120, 40);

    // Store line i is drawn on screen row 5 + i; the comment starts after
    // the store text and the (empty) arrow gutter, dimmed, indentation kept
    let col = comment_column(app.flow);
    let row1: String = row_text(&buffer, 6).chars().skip(col).collect();
    assert!(row1.starts_with(" ; A = -a"), "row1: {row1:?}");
    assert_eq!(Color::DarkGray, buffer[(col as u16, 6)].fg);
    let row4: String = row_text(&buffer, 9).chars().skip(col).collect();
    assert!(row4.starts_with(" ;   x"), "row4: {row4:?}");
    // A word without a comment gets blank padding, keeping the column aligned
    let row2: String = row_text(&buffer, 7).chars().skip(col).collect();
    assert!(
        row2.starts_with(&" ".repeat(" A = -a".len() + 2)),
        "row2: {row2:?}"
    );
    // The top bar advertises the column
    assert!(row_text(&buffer, 0).contains("[ COMMENTS ]"));
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
    assert!(!row_text(&buffer, 0).contains("COMMENTS"));

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
pub(super) fn no_comments_means_no_column_even_when_enabled() {
    let snap = snapshot_with_history(0); // all comments None
    let mut app = App::new();
    assert!(app.show_comments);
    let buffer = render(&mut app, &snap, 100, 40);

    assert!(!row_text(&buffer, 0).contains("COMMENTS"));
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
    // (MIN_WIDTH leaves a single cell past the store text, less than " ;")
    let buffer = render(&mut app, &snap, MIN_WIDTH, 40);
    assert!(!row_text(&buffer, 5).contains(';'));
    assert!(!row_text(&buffer, 0).contains("COMMENTS"));

    // Wide enough: the full comment and the top-bar label are drawn
    let buffer = render(&mut app, &snap, (full + 20) as u16, 40);
    assert!(row_text(&buffer, 5).contains("; a comment"));
    assert!(row_text(&buffer, 0).contains("[ COMMENTS ]"));
}

#[test]
pub(super) fn comment_crop_accounts_for_the_heatmap_width() {
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[0] = Some(" a comment".into()); // 10 cells with the leading space
    let snap = comment_snapshot(comments);

    let mut app = App::new();
    app.flow = FlowMode::Off;
    app.heatmap = HeatmapMode::Rw;

    // The r/w heatmap fits at this width (>= MIN_WIDTH + its extra width)
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

    let mut app = App::new(); // flow: Data
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
