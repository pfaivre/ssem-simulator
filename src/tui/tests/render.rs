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

    // Store word i is drawn on screen row 5 + i, its mnemonic in the
    // mnemonic column, bold
    let expected = ["JMP", "JRP", "LDN", "STO", "SUB", "SUB2", "CMP", "STP"];
    for (line, mnemonic) in expected.iter().enumerate() {
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
        assert!(
            cell.modifier.contains(Modifier::BOLD),
            "mnemonic on line {line} should be bold"
        );
    }
}

#[test]
pub(super) fn cmp_and_stp_show_no_data_value_on_the_margin() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("LDN 7").unwrap();
    // Words decoding to CMP/STP with a non-zero data field
    store[6] = Store::parse_word("CMP 5").unwrap();
    store[7] = Store::parse_word("STP 31").unwrap();
    let snap = Snapshot {
        store,
        ..snapshot_with_history(0)
    };

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    // The data value sits at the right margin of the store text, right
    // after the 4-wide mnemonic (column 51): columns 55..58. LDN shows its
    // operand; CMP and STP ignore their data field, so the margin is blank.
    let margin = |line: u16| -> String {
        row_text(&buffer, 5 + line)
            .chars()
            .skip(55)
            .take(3)
            .collect()
    };
    assert_eq!(" 07", margin(2), "LDN row: {:?}", row_text(&buffer, 7));
    assert_eq!("   ", margin(6), "CMP row: {:?}", row_text(&buffer, 11));
    assert_eq!("   ", margin(7), "STP row: {:?}", row_text(&buffer, 12));
}

#[test]
pub(super) fn status_bar_blocks_have_no_brackets() {
    // The status bar (bottom row) draws its blocks without brackets.
    // (Colors are theme properties and intentionally not asserted here.)
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);
    let status_row = buffer.area.height - 1;
    let row = row_text(&buffer, status_row);

    assert!(!row.contains('['), "status bar row: {row:?}");
    assert!(!row.contains(']'), "status bar row: {row:?}");
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
    assert!(all.contains("memory access"), "color legend missing");
    assert!(
        all.contains("Markers when paused:"),
        "marker legend missing"
    );
    assert!(all.contains("Heatmap"), "heatmap legend missing");
}

#[test]
pub(super) fn help_popup_shows_the_plain_footer_when_everything_fits() {
    let mut app = App::new();
    app.help_popup.open();
    let snap = snapshot_with_history(0);
    // Tall enough for the whole pop-up: no scroll hint in the footer
    let buffer = render(&mut app, &snap, 100, 50);

    let all: String = (0..buffer.area.height)
        .map(|y| row_text(&buffer, y))
        .collect();
    assert!(all.contains("Press H to close"), "footer missing");
    assert!(
        !all.contains("scroll \u{b7} H close"),
        "unexpected scroll hint"
    );
    assert!(
        all.contains("Control-flow graph:"),
        "bottom of the text clipped"
    );
}

#[test]
pub(super) fn help_popup_title_shows_the_program_version() {
    let mut app = App::new();
    app.help_popup.open();
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 100, 50);

    let all: String = (0..buffer.area.height)
        .map(|y| row_text(&buffer, y))
        .collect();
    let title = format!("SSEM simulator v{}", env!("CARGO_PKG_VERSION"));
    assert!(all.contains(&title), "version missing from the title");
}

#[test]
pub(super) fn help_popup_scrolls_when_the_window_is_too_small() {
    let mut app = App::new();
    app.help_popup.open();
    let snap = snapshot_with_history(0);
    let mut sim = looping_sim(0);

    let all_text = |buffer: &Buffer| -> String {
        (0..buffer.area.height)
            .map(|y| row_text(buffer, y))
            .collect()
    };

    // Small window: the pop-up is clamped, the tail of the text is hidden
    let buffer = render(&mut app, &snap, 100, 20);
    let all = all_text(&buffer);
    assert!(all.contains("SSEM simulator"), "title missing");
    assert!(
        all.contains("Quit the simulator"),
        "top of the text missing"
    );
    assert!(
        !all.contains("Control-flow graph:"),
        "tail should be hidden at scroll 0"
    );
    // The footer stays pinned just above the bottom border and advertises scrolling
    let footer_row = buffer.area.height - 2;
    let footer_text = row_text(&buffer, footer_row);
    assert!(
        footer_text.contains("scroll \u{b7} H close"),
        "pinned scroll footer missing: {footer_text:?}"
    );

    // End jumps to the bottom: the tail becomes visible, the footer stays
    app.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &mut sim);
    let buffer = render(&mut app, &snap, 100, 20);
    let all = all_text(&buffer);
    assert!(
        all.contains("Control-flow graph:"),
        "tail missing after End"
    );
    assert!(
        row_text(&buffer, buffer.area.height - 2).contains("scroll \u{b7} H close"),
        "footer should stay pinned at the bottom"
    );

    // Down past the end stays clamped at the bottom
    app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut sim);
    let buffer = render(&mut app, &snap, 100, 20);
    assert!(all_text(&buffer).contains("Control-flow graph:"));

    // Home scrolls back to the top
    app.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &mut sim);
    let buffer = render(&mut app, &snap, 100, 20);
    let all = all_text(&buffer);
    assert!(all.contains("Quit the simulator"), "top missing after Home");
    assert!(
        !all.contains("Control-flow graph:"),
        "tail visible after Home"
    );

    // A few Down presses scroll line by line
    for _ in 0..3 {
        app.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut sim);
    }
    let buffer = render(&mut app, &snap, 100, 20);
    assert!(!all_text(&buffer).contains("Control-flow graph:"));
}

#[test]
pub(super) fn scrollbar_appears_on_the_right_edge_when_the_window_is_too_short() {
    // 80x20: the main body is 17 rows, less than the 36 content rows, so a
    // scrollbar is drawn on the right edge (column 79) of the body rows 1..=17
    let snap = snapshot_with_history(0);
    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 20);
    let col = 79u16;

    // A solid track over the whole body, no begin/end arrows
    for row in 1..=17 {
        assert_eq!("█", buffer[(col, row)].symbol(), "solid track on row {row}");
    }
    // At scroll 0 the thumb sits at the top of the track (rows 1..=8, the
    // thumb is 8 cells: round(17*17/36))
}

#[test]
pub(super) fn no_scrollbar_when_the_whole_body_fits() {
    // 80x40: the main body is 37 rows, enough for the 36 content rows
    let snap = snapshot_with_history(0);
    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    for row in 1..38 {
        assert_eq!(
            " ",
            buffer[(79, row)].symbol(),
            "no scrollbar on body row {row}"
        );
    }
}

#[test]
pub(super) fn renders_at_any_terminal_size() {
    // There is no minimum terminal size: even a tiny window renders the
    // chrome, with the body cropped by the screen edges
    let snap = snapshot_with_history(0);
    let mut app = App::new();

    for (width, height) in [(1u16, 1u16), (10, 4), (30, 8)] {
        let buffer = render(&mut app, &snap, width, height);
        assert_eq!(Rect::new(0, 0, width, height), buffer.area);
    }

    // At 30x8 the menu bar is on row 0 and the status bar on the last row
    let buffer = render(&mut app, &snap, 30, 8);
    assert!(row_text(&buffer, 0).contains("Quit"));
    assert!(row_text(&buffer, 7).contains("Status"));
}

#[test]
pub(super) fn popups_and_mouse_work_at_tiny_terminal_sizes() {
    // No minimum terminal size: every pop-up, an active scrub, and mouse
    // handling all work — and never panic — even in a 1x1 window
    let snap = snapshot_with_history(0);
    let mut sim = looping_sim(10);

    for (width, height) in [(1u16, 1u16), (5, 3), (10, 4)] {
        let mut app = App::new();
        app.help_popup.open();
        app.breakpoint_popup.open();
        app.editor_popup.open_for_word(0);
        app.save_popup.open(None);
        app.settings_popup.open();
        app.comment_editor.open_for(0, None);
        app.picker.open_for_mnemonic(0, 0, 5);
        app.scrubbing = true;
        let buffer = render(&mut app, &snap, width, height);
        assert_eq!(Rect::new(0, 0, width, height), buffer.area);

        // Sweep every cell with clicks, drags, moves, and wheel scrolls
        for row in 0..height {
            for column in 0..width {
                for kind in [
                    MouseEventKind::Down(MouseButton::Left),
                    MouseEventKind::Drag(MouseButton::Left),
                    MouseEventKind::Moved,
                    MouseEventKind::ScrollUp,
                    MouseEventKind::ScrollDown,
                ] {
                    app.handle_mouse(
                        MouseEvent {
                            column,
                            row,
                            kind,
                            modifiers: KeyModifiers::NONE,
                        },
                        &mut sim,
                    );
                }
            }
        }
        // Whatever state the sweep left the app in, it still renders
        render(&mut app, &snap, width, height);
    }
}

#[test]
pub(super) fn scrollbar_can_be_pressed_and_dragged_with_the_mouse() {
    let snap = snapshot_with_history(0);
    let mut app = App::new();
    let mut sim = looping_sim(0);
    let click = |kind, row| MouseEvent {
        column: 79,
        row,
        kind,
        modifiers: KeyModifiers::NONE,
    };
    let drag_at = |column, row| MouseEvent {
        column,
        row,
        kind: MouseEventKind::Drag(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
    };

    // Render once so the app knows the terminal area
    let _ = render(&mut app, &snap, 80, 20);

    // At 80x20 the body is 17 rows: track 17, thumb 8, max scroll 19; at
    // scroll 0 the thumb spans rows 1..=8. Pressing on the thumb grabs it
    // without moving it: the thumb only moves when the mouse moves
    app.handle_mouse(click(MouseEventKind::Down(MouseButton::Left), 4), &mut sim);
    assert_eq!(
        0, app.store_scroll,
        "pressing the bar must not jump the thumb"
    );

    // Dragging two rows down moves the thumb two rows down: grabbed on row 4
    // (thumb top on row 1, offset 3), a drag to row 6 puts the thumb top on
    // row 3 (track start 2, scroll round(2*36/17) = 4)
    app.handle_mouse(click(MouseEventKind::Drag(MouseButton::Left), 6), &mut sim);
    assert_eq!(4, app.store_scroll);
    let _ = render(&mut app, &snap, 80, 20);

    // Two more rows down: thumb top on row 5 (scroll 8)
    app.handle_mouse(click(MouseEventKind::Drag(MouseButton::Left), 8), &mut sim);
    assert_eq!(8, app.store_scroll);

    // Pressing on the track below the thumb (row 17, the thumb is on rows
    // 5..=12) moves the content one paragraph (one visible body, 17 rows)
    // down, clamped to the max scroll 19, and does not grab the thumb
    app.handle_mouse(click(MouseEventKind::Up(MouseButton::Left), 8), &mut sim);
    app.handle_mouse(click(MouseEventKind::Down(MouseButton::Left), 17), &mut sim);
    assert_eq!(
        19, app.store_scroll,
        "track click below the thumb pages down one body"
    );
    // A later drag does not scroll: the track click did not grab the thumb
    app.handle_mouse(click(MouseEventKind::Drag(MouseButton::Left), 15), &mut sim);
    assert_eq!(
        19, app.store_scroll,
        "drag after a track click must not scroll"
    );

    // At scroll 19 the thumb spans rows 10..=17; pressing the track above
    // it (row 5) pages up one body: 19 - 17 = 2
    app.handle_mouse(click(MouseEventKind::Down(MouseButton::Left), 5), &mut sim);
    assert_eq!(2, app.store_scroll, "track click above the thumb pages up");

    // At scroll 2 the thumb spans rows 2..=9; pressing on it (row 5) grabs
    // without moving, and dragging off-column past the end of the track
    // clamps the thumb at the bottom (max scroll 19)
    app.handle_mouse(click(MouseEventKind::Down(MouseButton::Left), 5), &mut sim);
    assert_eq!(2, app.store_scroll, "pressing the thumb must not jump it");
    app.handle_mouse(drag_at(10, 19), &mut sim);
    assert_eq!(19, app.store_scroll, "drag off-column clamps to the bottom");

    // While the button is held, drags off the scrollbar column (left or
    // right of it) keep moving the thumb, like the timeline scrubber:
    // still grabbed from the row-5 press (offset 3), a drag at row 3
    // puts the thumb top back on row 1 (scroll 0)
    app.handle_mouse(drag_at(40, 3), &mut sim);
    assert_eq!(0, app.store_scroll, "drag left of the bar still scrolls");

    // The wheel over the scrollbar column still scrolls line by line
    app.handle_mouse(click(MouseEventKind::ScrollDown, 5), &mut sim);
    assert_eq!(1, app.store_scroll);

    // Releasing ends the drag: a later drag anywhere does not scroll
    app.handle_mouse(click(MouseEventKind::Up(MouseButton::Left), 19), &mut sim);
    app.handle_mouse(drag_at(40, 8), &mut sim);
    assert_eq!(1, app.store_scroll, "drag after release must not scroll");
}

#[test]
pub(super) fn help_popup_scrolls_with_the_mouse_wheel_over_it() {
    let mut app = App::new();
    app.help_popup.open();
    let snap = snapshot_with_history(0);
    let mut sim = looping_sim(0);

    let all_text = |buffer: &Buffer| -> String {
        (0..buffer.area.height)
            .map(|y| row_text(buffer, y))
            .collect()
    };
    let wheel = |kind, column, row| MouseEvent {
        column,
        row,
        kind,
        modifiers: KeyModifiers::NONE,
    };

    // Small window; render once so the pop-up rect and max scroll are known
    let buffer = render(&mut app, &snap, 100, 20);
    assert!(!all_text(&buffer).contains("Control-flow graph:"));

    // Wheel down over the pop-up (screen center) scrolls the text to the bottom
    for _ in 0..50 {
        app.handle_mouse(wheel(MouseEventKind::ScrollDown, 50, 10), &mut sim);
    }
    let buffer = render(&mut app, &snap, 100, 20);
    assert!(
        all_text(&buffer).contains("Control-flow graph:"),
        "wheel down over the pop-up should reveal the tail"
    );

    // Wheel up past the top stays clamped at scroll 0
    for _ in 0..100 {
        app.handle_mouse(wheel(MouseEventKind::ScrollUp, 50, 10), &mut sim);
    }
    let buffer = render(&mut app, &snap, 100, 20);
    let all = all_text(&buffer);
    assert!(
        all.contains("Quit the simulator"),
        "top missing after wheel up"
    );
    assert!(
        !all.contains("Control-flow graph:"),
        "wheel up should be clamped at the top"
    );

    // Wheel outside the pop-up (left of it) does not scroll the help text
    app.handle_mouse(wheel(MouseEventKind::ScrollDown, 0, 10), &mut sim);
    let buffer = render(&mut app, &snap, 100, 20);
    let all = all_text(&buffer);
    assert!(
        all.contains("Quit the simulator"),
        "wheel outside changed the scroll"
    );
    assert!(!all.contains("Control-flow graph:"));
}
