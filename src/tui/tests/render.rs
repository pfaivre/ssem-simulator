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
    // pointer-arrow palette: jumps (JMP, JRP) together, memory accesses
    // (LDN, STO) together, then arithmetic, test, and halt.
    let expected = [
        (0, "JMP", DARK.op_jump),
        (1, "JRP", DARK.op_jump),
        (2, "LDN", DARK.op_memory),
        (3, "STO", DARK.op_memory),
        (4, "SUB", DARK.op_arithmetic),
        (5, "SUB2", DARK.op_arithmetic),
        (6, "CMP", DARK.op_test),
        (7, "STP", DARK.op_halt),
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
pub(super) fn main_body_lines_have_an_alternating_background() {
    // The main body starts at screen row 1 (the top bar takes row 0), and
    // content row r is drawn at screen row r + 1 while unscrolled. Odd
    // content rows — the spacers, CI, and every other store word — carry
    // the zebra background across the full width of the body...
    let snap = snapshot_with_history(0);
    let mut app = App::new();
    // One row taller than the body needs, so the area below the machine's
    // lines includes an odd (stripe-eligible) row
    let buffer = render(&mut app, &snap, 80, 41);

    for content_row in 0..CONTENT_HEIGHT {
        let screen_row = 1 + content_row as u16;
        let expected = if content_row % 2 == 1 {
            DARK.zebra_bg
        } else {
            DARK.bg
        };
        for x in [0, 1, 40, 79] {
            assert_eq!(
                expected,
                buffer[(x, screen_row)].bg,
                "content row {content_row}, column {x}"
            );
        }
    }

    // Below the machine's own lines the background stays plain (the theme's
    // base background, not striped), and so do the timeline, bottom bar, and
    // top bar
    for row in (1 + CONTENT_HEIGHT as u16)..buffer.area.height {
        for x in 0..buffer.area.width {
            assert_eq!(DARK.bg, buffer[(x, row)].bg, "row below the body {row}");
        }
    }
    for x in 0..buffer.area.width {
        assert_eq!(DARK.bg, buffer[(x, 0)].bg, "top bar column {x}");
    }

    // The stripes are a background only: the CI bits keep their green
    // foreground on the striped CI row (content row 1 = screen row 2)
    let ci_screen_row = 1 + CI_ROW as u16;
    let cell = &buffer[(2, ci_screen_row)];
    assert_eq!(DARK.value, cell.fg);
    assert_eq!(DARK.zebra_bg, cell.bg);

    // Plain (unstyled) text carries the theme's default foreground
    // (screen row 1 is the blank first line of the body)
    assert_eq!(DARK.fg, buffer[(0, 1)].fg);
}

#[test]
pub(super) fn top_bar_stays_blue_when_stopped() {
    // The chrome keeps a single color, the last instruction's mnemonic
    // included: the syntax highlighting lives in the store view. STOPPED
    // keeps the chrome color too.
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);

    let row = row_text(&buffer, 0);
    let pos = row.find("SUB").expect("last instruction in the top bar");
    assert_eq!(DARK.chrome, buffer[(pos as u16, 0)].fg);
    for x in 0..buffer.area.width {
        let cell = &buffer[(x, 0)];
        if cell.symbol() != " " {
            assert_eq!(DARK.chrome, cell.fg, "top bar column {x}");
        }
    }
}

#[test]
pub(super) fn status_word_is_colored_by_state() {
    // RUNNING is green while the machine runs, REWIND is yellow while
    // scrubbing the timeline; the rest of the bar keeps the chrome blue.
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    let mut snap = pointer_snapshot(2, store);
    snap.stop_flag = false;

    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);
    let row = row_text(&buffer, 0);
    let pos = row.find("RUNNING").expect("status in the top bar");
    for i in 0.."RUNNING".len() {
        assert_eq!(
            DARK.status_running,
            buffer[((pos + i) as u16, 0)].fg,
            "RUNNING char {i}"
        );
    }
    let pos = row.find("CYCLES").expect("cycles in the top bar");
    assert_eq!(DARK.chrome, buffer[(pos as u16, 0)].fg);

    let mut app = App::new();
    app.scrubbing = true;
    let buffer = render(&mut app, &snap, 80, 40);
    let row = row_text(&buffer, 0);
    let pos = row.find("REWIND").expect("status in the top bar");
    for i in 0.."REWIND".len() {
        assert_eq!(
            DARK.status_rewind,
            buffer[((pos + i) as u16, 0)].fg,
            "REWIND char {i}"
        );
    }

    let snap = pointer_snapshot(2, store); // stop_flag = true
    let mut app = App::new();
    let buffer = render(&mut app, &snap, 80, 40);
    let row = row_text(&buffer, 0);
    let pos = row.find("STOPPED").expect("status in the top bar");
    assert_eq!(DARK.chrome, buffer[(pos as u16, 0)].fg);
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
    let title = format!("SSEM SIMULATOR v{}", env!("CARGO_PKG_VERSION"));
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
    assert!(all.contains("SSEM SIMULATOR"), "title missing");
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
    // thumb is 8 cells: round(17*17/36)), in the chrome color over the dim track
    assert_eq!(DARK.chrome, buffer[(col, 1)].fg, "thumb color");
    assert_eq!(DARK.chrome, buffer[(col, 8)].fg, "thumb bottom edge");
    assert_eq!(DARK.dim, buffer[(col, 9)].fg, "track color");
    assert_eq!(DARK.dim, buffer[(col, 17)].fg, "track at the last body row");
}

#[test]
pub(super) fn scrollbar_thumb_follows_the_store_scroll() {
    let snap = snapshot_with_history(0);
    let mut app = App::new();
    // Maximum scroll at 80x20: 36 - 17 = 19
    app.store_scroll = 19;
    let buffer = render(&mut app, &snap, 80, 20);
    let col = 79u16;

    // The thumb reaches the bottom of the track (rows 10..=17)
    assert_eq!(DARK.chrome, buffer[(col, 17)].fg, "thumb at the bottom");
    assert_eq!(DARK.chrome, buffer[(col, 10)].fg, "thumb top edge");
    assert_eq!(DARK.dim, buffer[(col, 9)].fg, "track above the thumb");
    assert_eq!(DARK.dim, buffer[(col, 1)].fg, "track at the first body row");
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

    // Press on the bottom arrow row jumps the thumb to the bottom (max
    // scroll 19 at 80x20: track 15, thumb 7)
    app.handle_mouse(click(MouseEventKind::Down(MouseButton::Left), 17), &mut sim);
    assert_eq!(19, app.store_scroll);

    // Dragging up to the top row scrolls back to 0: the thumb follows the
    // cursor 1:1 (grabbed at its bottom, so the cursor stays on the thumb)
    app.handle_mouse(click(MouseEventKind::Drag(MouseButton::Left), 1), &mut sim);
    assert_eq!(0, app.store_scroll);

    // After releasing, a drag no longer scrolls
    app.handle_mouse(click(MouseEventKind::Up(MouseButton::Left), 1), &mut sim);
    app.handle_mouse(click(MouseEventKind::Drag(MouseButton::Left), 10), &mut sim);
    assert_eq!(0, app.store_scroll, "drag after release must not scroll");

    // Press mid-track: row 6 puts the thumb's top on row 6 (scroll 11)
    app.handle_mouse(click(MouseEventKind::Down(MouseButton::Left), 6), &mut sim);
    assert_eq!(11, app.store_scroll);
    // Moving the cursor two rows down moves the thumb two rows down
    app.handle_mouse(click(MouseEventKind::Drag(MouseButton::Left), 8), &mut sim);
    assert_eq!(15, app.store_scroll);
    let buffer = render(&mut app, &snap, 80, 20);
    assert_eq!(
        DARK.chrome,
        buffer[(79, 8)].fg,
        "thumb top followed the cursor"
    );
    assert_eq!(DARK.dim, buffer[(79, 6)].fg, "track where the thumb was");

    // While the button is held, drags off the scrollbar column (left or
    // right of it) keep moving the thumb, like the timeline scrubber
    app.handle_mouse(drag_at(40, 1), &mut sim);
    assert_eq!(0, app.store_scroll, "drag left of the bar still scrolls");
    app.handle_mouse(drag_at(10, 19), &mut sim);
    assert_eq!(19, app.store_scroll, "drag off-column clamps to the bottom");

    // The wheel over the scrollbar column still scrolls line by line
    app.handle_mouse(click(MouseEventKind::ScrollUp, 5), &mut sim);
    assert_eq!(18, app.store_scroll);

    // Releasing ends the drag: a later drag anywhere does not scroll
    app.handle_mouse(click(MouseEventKind::Up(MouseButton::Left), 19), &mut sim);
    app.handle_mouse(drag_at(40, 8), &mut sim);
    assert_eq!(18, app.store_scroll, "drag after release must not scroll");
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
