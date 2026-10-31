use super::*;

/// Screen column where a store line's mnemonic starts:
/// pointer (1) + bits (32) + " NN: " + value (11) + "  " = 51.
pub(super) const MNEMONIC_COLUMN: usize = 51;

#[test]
pub(super) fn mnemonics_are_colored_by_category() {
    let mut store = [0; 32];
    store[0] = Store::parse_word("JMP 0").unwrap();
    store[1] = Store::parse_word("JRP 0").unwrap();
    store[2] = Store::parse_word("LDN 0").unwrap();
    store[3] = Store::parse_word("STO 0").unwrap();
    store[4] = Store::parse_word("SUB 0").unwrap();
    store[5] = Store::parse_word("SUB2 0").unwrap();
    store[6] = Store::parse_word("CMP").unwrap();
    store[7] = Store::parse_word("STP").unwrap();
    let snap = Snapshot {
        store,
        ..snapshot_with_history(0)
    };

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    // Store word i is drawn on screen row 5 + i. The colors match the
    // pointer-arrow palette: magenta for a data movement, cyan for an
    // arithmetic combination.
    let expected = [
        (0, "JMP", Color::Magenta),
        (1, "JRP", Color::Cyan),
        (2, "LDN", Color::Magenta),
        (3, "STO", Color::Magenta),
        (4, "SUB", Color::Cyan),
        (5, "SUB2", Color::Cyan),
        (6, "CMP", Color::White),
        (7, "STP", Color::Red),
    ];
    for (line, mnemonic, color) in expected {
        let row = 5 + line as u16;
        let text = row_text(&buffer, row);
        // Slice by chars, not bytes: the pointer column holds multi-byte
        // glyphs (» •) that would shift a byte offset
        let suffix: String = text.chars().skip(MNEMONIC_COLUMN).collect();
        assert!(
            suffix.starts_with(mnemonic),
            "line {line} should show {mnemonic}: {text:?}"
        );
        let cell = &buffer[(MNEMONIC_COLUMN as u16, row)];
        assert_eq!(color, cell.fg, "mnemonic color on line {line}");
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "mnemonic on line {line} should be bold"
        );
    }
}

#[test]
pub(super) fn top_bar_stays_monochrome_blue() {
    // The chrome keeps a single color, the last instruction's mnemonic
    // included: the syntax highlighting lives in the store view
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    let row = row_text(&buffer, 0);
    let pos = row.find("SUB").expect("last instruction in the top bar");
    assert_eq!(Color::Blue, buffer[(pos as u16, 0)].fg);
    for x in 0..buffer.area.width {
        let cell = &buffer[(x, 0)];
        if cell.symbol() != " " {
            assert_eq!(Color::Blue, cell.fg, "top bar column {x}");
        }
    }
}

#[test]
pub(super) fn executed_footprint_markers_shown_when_paused() {
    let mut store = [0; 32];
    store[3] = Store::parse_word("STO 10").unwrap();
    let mut snap = pointer_snapshot(3, store); // ci = 3, last executed line 3
    snap.exec_counts[3] = 5;
    snap.exec_counts[7] = 1;
    assert!(snap.stop_flag, "the machine is paused");

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    // `>` stays on the last executed line (screen row 8)
    assert!(row_text(&buffer, 8).starts_with('>'));
    // `»` marks the next line to execute: (ci + 1) & 31 = 4 (row 9)
    assert!(row_text(&buffer, 9).starts_with('\u{bb}'));
    // `•` marks the other executed line (row 12); untouched lines stay blank
    assert!(row_text(&buffer, 12).starts_with('\u{2022}'));
    assert!(row_text(&buffer, 10).starts_with(' '));
}

#[test]
pub(super) fn executed_footprint_markers_hidden_while_running() {
    let mut store = [0; 32];
    store[3] = Store::parse_word("STO 10").unwrap();
    let mut snap = pointer_snapshot(3, store);
    snap.exec_counts[3] = 5;
    snap.exec_counts[7] = 1;
    snap.stop_flag = false;

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    // Only `>` remains; no `»` or `•` while the machine runs
    assert!(row_text(&buffer, 8).starts_with('>'));
    assert!(row_text(&buffer, 9).starts_with(' '));
    assert!(row_text(&buffer, 12).starts_with(' '));
}

#[test]
pub(super) fn next_marker_points_at_the_first_fetch_on_a_fresh_machine() {
    // Fresh machine, paused before the first cycle: CI = 0, and the
    // first fetch executes line 1 (CI is incremented before the fetch)
    let snap = snapshot_with_history(0);

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    assert!(row_text(&buffer, 6).starts_with('\u{bb}'), "line 1 (row 6)");
    for row in 5..=36 {
        let text = row_text(&buffer, row);
        assert!(!text.starts_with('\u{2022}'), "row {row}: {text:?}");
        if row != 6 {
            assert!(!text.starts_with('\u{bb}'), "row {row}: {text:?}");
        }
    }
}

#[test]
pub(super) fn help_popup_documents_the_color_code_and_paused_markers() {
    let mut app = App::new();
    app.help_popup.open();
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 100, 40);

    let all: String = (0..buffer.area.height)
        .map(|y| row_text(&buffer, y))
        .collect();
    assert!(all.contains("Mnemonic colors:"), "color legend missing");
    assert!(all.contains("data movement"), "color legend missing");
    assert!(
        all.contains("Markers when paused:"),
        "marker legend missing"
    );
    assert!(all.contains("Heatmap"), "heatmap legend missing");
}
