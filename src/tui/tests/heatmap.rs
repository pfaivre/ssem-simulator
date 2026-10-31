use super::*;

#[test]
pub(super) fn heatmap_key_cycles_off_exec_rw() {
    let mut sim = Simulator::new();
    let mut app = App::new();
    assert_eq!(HeatmapMode::Off, app.heatmap);

    for expected in [HeatmapMode::Exec, HeatmapMode::Rw, HeatmapMode::Off] {
        app.handle_key(
            KeyEvent::new(KeyCode::Char('m'), KeyModifiers::NONE),
            &mut sim,
        );
        assert_eq!(expected, app.heatmap);
    }

    // Uppercase cycles too
    app.handle_key(
        KeyEvent::new(KeyCode::Char('M'), KeyModifiers::NONE),
        &mut sim,
    );
    assert_eq!(HeatmapMode::Exec, app.heatmap);
}

#[test]
pub(super) fn top_bar_shows_the_heatmap_mode() {
    let snap = snapshot_with_history(0);

    // Wide enough that the label is not truncated by the screen edge
    let mut app = App::new();
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(!row_text(&buffer, 0).contains("HEATMAP"));

    app.heatmap = HeatmapMode::Exec;
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(row_text(&buffer, 0).contains("[ HEATMAP: EXEC ]"));

    app.heatmap = HeatmapMode::Rw;
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(row_text(&buffer, 0).contains("[ HEATMAP: R/W ]"));
}

#[test]
pub(super) fn exec_heatmap_draws_one_bar_per_word() {
    let mut snap = snapshot_with_history(0);
    snap.exec_counts[5] = 4; // the hottest line
    snap.exec_counts[9] = 1;

    let mut app = App::new();
    app.heatmap = HeatmapMode::Exec;
    let buffer = render(&mut app, &snap, 80, 40);

    // Store word 5 is on screen row 10: pointer (1), space (1), then the
    // 6-cell bar (cols 2..8), then a space
    let chars: Vec<char> = row_text(&buffer, 10).chars().collect();
    assert_eq!(&['█', '█', '█', '█', '█', '█'], &chars[2..8]);
    assert_eq!(' ', chars[8]);
    assert_eq!(Color::Red, buffer[(2, 10)].fg, "hottest line is red");

    // Word 9 (row 14): ratio 1/4 = 0.25 -> filled = ceil(0.25*6) = 2, green
    let chars: Vec<char> = row_text(&buffer, 14).chars().collect();
    assert_eq!(&['█', '█', '·', '·', '·', '·'], &chars[2..8]);
    assert_eq!(Color::Green, buffer[(2, 14)].fg);

    // Untouched word 0 (row 5): all dim dots
    let chars: Vec<char> = row_text(&buffer, 5).chars().collect();
    assert_eq!(&['·', '·', '·', '·', '·', '·'], &chars[2..8]);
    assert_eq!(Color::DarkGray, buffer[(2, 5)].fg);
}

/// In r/w mode a store row reads: pointer, " r", read bar (3), " w",
/// write bar (3), " ", then the bits.
pub(super) const RW_R_COL: usize = 2;

pub(super) const RW_RBAR_COL: usize = 3;

pub(super) const RW_W_COL: usize = 7;

pub(super) const RW_WBAR_COL: usize = 8;

#[test]
pub(super) fn rw_heatmap_draws_labeled_read_and_write_bars() {
    let mut snap = snapshot_with_history(0);
    snap.read_counts[3] = 10; // hottest read
    snap.read_counts[7] = 2;
    snap.write_counts[8] = 5; // hottest write

    let mut app = App::new();
    app.heatmap = HeatmapMode::Rw;
    let buffer = render(&mut app, &snap, 80, 40);

    // Word 3 (screen row 8): full read bar, no writes
    let chars: Vec<char> = row_text(&buffer, 8).chars().collect();
    assert_eq!('r', chars[RW_R_COL]);
    assert_eq!(&['█', '█', '█'], &chars[RW_RBAR_COL..RW_RBAR_COL + 3]);
    assert_eq!(Color::LightCyan, buffer[(RW_RBAR_COL as u16, 8)].fg);
    assert_eq!('w', chars[RW_W_COL]);
    assert_eq!(&['·', '·', '·'], &chars[RW_WBAR_COL..RW_WBAR_COL + 3]);
    assert_eq!(Color::DarkGray, buffer[(RW_WBAR_COL as u16, 8)].fg);

    // Word 7 (row 12): read ratio 0.2 -> one filled cell, blue
    let chars: Vec<char> = row_text(&buffer, 12).chars().collect();
    assert_eq!(&['█', '·', '·'], &chars[RW_RBAR_COL..RW_RBAR_COL + 3]);
    assert_eq!(Color::Blue, buffer[(RW_RBAR_COL as u16, 12)].fg);

    // Word 8 (row 13): no reads, full write bar in the hottest write color
    let chars: Vec<char> = row_text(&buffer, 13).chars().collect();
    assert_eq!(&['·', '·', '·'], &chars[RW_RBAR_COL..RW_RBAR_COL + 3]);
    assert_eq!(&['█', '█', '█'], &chars[RW_WBAR_COL..RW_WBAR_COL + 3]);
    assert_eq!(Color::LightRed, buffer[(RW_WBAR_COL as u16, 13)].fg);

    // The bits still start right after the heatmap area (col 12)
    let chars: Vec<char> = row_text(&buffer, 8).chars().collect();
    assert_eq!('.', chars[12], "bits column shifted by the r/w bars");
}

#[test]
pub(super) fn rw_heatmap_is_dropped_when_it_does_not_fit() {
    let snap = snapshot_with_history(0);
    let mut app = App::new();
    app.heatmap = HeatmapMode::Rw;
    app.flow = FlowMode::Off;

    // One column short of the heatmap: the store still renders, without
    // the heatmap (it sits left of the bits and would push the store text
    // out of view), and without the top-bar label
    let needed = MIN_WIDTH + HeatmapMode::Rw.extra_width();
    let buffer = render(&mut app, &snap, needed - 1, 40);
    assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    assert!(!row_text(&buffer, 0).contains("HEATMAP"));
    let chars: Vec<char> = row_text(&buffer, 10).chars().collect();
    assert_eq!('.', chars[1], "bits right after the pointer, no heatmap");

    // Wide enough: the r/w bars are drawn
    let buffer = render(&mut app, &snap, needed, 40);
    assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    let chars: Vec<char> = row_text(&buffer, 10).chars().collect();
    assert_eq!('r', chars[2], "r/w heatmap bars present");

    // ... and the top-bar label shows too (needs a wider screen, the
    // label sits far right in the bar)
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(row_text(&buffer, 0).contains("[ HEATMAP: R/W ]"));
}

#[test]
pub(super) fn rw_heatmap_shifts_the_pointer_arrows() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.heatmap = HeatmapMode::Rw;
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The gutter shifts right by the r/w heatmap width
    let extra = HeatmapMode::Rw.extra_width() as usize;
    let gutter = GUTTER_COLUMN + extra;
    let arrowhead = ARROWHEAD_COLUMN + extra;

    let source = row_text(&buffer, 3);
    assert_eq!('╮', source.chars().nth(gutter).unwrap(), "row: {source:?}");
    let target = row_text(&buffer, 15);
    assert_eq!(
        '◀',
        target.chars().nth(arrowhead).unwrap(),
        "target row: {target:?}"
    );
}
