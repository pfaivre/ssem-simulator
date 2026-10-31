use super::*;

#[test]
pub(super) fn pointer_arrow_sto_points_from_accumulator_to_word() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 24").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The arrow leaves the accumulator (row 3), one space after its
    // value, connected by a run of dashes, and turns down into the gutter
    let source = row_text(&buffer, 3);
    assert_eq!(
        ' ',
        source.chars().nth(49).unwrap(),
        "source row: {source:?}"
    );
    assert_eq!(
        '─',
        source.chars().nth(50).unwrap(),
        "source row: {source:?}"
    );
    assert_eq!(
        '─',
        source.chars().nth(ARROWHEAD_COLUMN + 1).unwrap(),
        "source row: {source:?}"
    );
    assert_eq!('╮', source.chars().nth(GUTTER_COLUMN).unwrap());
    assert_eq!(Color::Magenta, buffer[(GUTTER_COLUMN as u16, 3)].fg);

    // Intermediate rows hold the vertical segment (including the blank row 4)
    for row in 4..29 {
        let text = row_text(&buffer, row);
        assert_eq!(
            '│',
            text.chars().nth(GUTTER_COLUMN).unwrap(),
            "row {row}: {text:?}"
        );
    }

    // The destination word (row 29) receives the arrowhead, one cell
    // left of the gutter corner and connected to it by a dash
    let target = row_text(&buffer, 29);
    assert_eq!(
        '◀',
        target.chars().nth(ARROWHEAD_COLUMN).unwrap(),
        "target row: {target:?}"
    );
    assert_eq!(
        '─',
        target.chars().nth(ARROWHEAD_COLUMN + 1).unwrap(),
        "target row: {target:?}"
    );
    assert_eq!('╯', target.chars().nth(GUTTER_COLUMN).unwrap());
    assert_eq!(Color::Yellow, buffer[(5, 29)].fg);

    // The accumulator is the source, not the destination: it stays green
    assert_eq!(Color::Green, buffer[(5, 3)].fg);
}

#[test]
pub(super) fn pointer_arrow_ldn_points_from_word_to_accumulator() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("LDN 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The source word (row 15) leaves the text and turns up
    let source = row_text(&buffer, 15);
    assert_eq!(
        '─',
        source.chars().nth(59).unwrap(),
        "source row: {source:?}"
    );
    assert_eq!('╯', source.chars().nth(GUTTER_COLUMN).unwrap());
    assert_eq!(Color::Magenta, buffer[(GUTTER_COLUMN as u16, 15)].fg);

    // Intermediate rows hold the vertical segment
    for row in 4..15 {
        let text = row_text(&buffer, row);
        assert_eq!(
            '│',
            text.chars().nth(GUTTER_COLUMN).unwrap(),
            "row {row}: {text:?}"
        );
    }

    // The arrow arrives at the accumulator (row 3), connected to its
    // text by a run of dashes, and the accumulator is highlighted
    let target = row_text(&buffer, 3);
    assert_eq!(
        '◀',
        target.chars().nth(50).unwrap(),
        "target row: {target:?}"
    );
    assert_eq!(
        '─',
        target.chars().nth(55).unwrap(),
        "target row: {target:?}"
    );
    assert_eq!('╮', target.chars().nth(GUTTER_COLUMN).unwrap());
    assert_eq!(Color::Yellow, buffer[(5, 3)].fg);

    // The source word is not the destination: it stays green
    assert_eq!(Color::Green, buffer[(5, 15)].fg);
}

#[test]
pub(super) fn pointer_arrow_sub_uses_a_different_color() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // Like LDN, but the arrow is cyan instead of magenta
    assert_eq!(Color::Cyan, buffer[(GUTTER_COLUMN as u16, 15)].fg);
    assert_eq!(Color::Cyan, buffer[(GUTTER_COLUMN as u16, 10)].fg);
    // The accumulator is still the highlighted destination
    assert_eq!(Color::Yellow, buffer[(5, 3)].fg);
}

#[test]
pub(super) fn pointer_arrow_jmp_points_from_word_to_ci() {
    // JMP copies the operand word into CI: the arrow is a data movement
    // from that word up to the CI register, not from the instruction.
    let mut store = [0; 32];
    store[2] = Store::parse_word("JMP 24").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The source word (screen row 29) leaves the text and turns up
    let source = row_text(&buffer, 29);
    assert_eq!(
        '─',
        source.chars().nth(59).unwrap(),
        "source row: {source:?}"
    );
    assert_eq!('╯', source.chars().nth(GUTTER_COLUMN).unwrap());
    assert_eq!(Color::Magenta, buffer[(GUTTER_COLUMN as u16, 29)].fg);

    // The vertical crosses the blank row and the accumulator row
    for row in 3..29 {
        let text = row_text(&buffer, row);
        assert_eq!(
            '│',
            text.chars().nth(GUTTER_COLUMN).unwrap(),
            "row {row}: {text:?}"
        );
    }
    // The accumulator is crossed, not an endpoint: it stays green
    assert_eq!(Color::Green, buffer[(5, 3)].fg);

    // The arrow arrives at the CI row (screen row 2), the highlighted
    // destination; the source word is not the destination
    let target = row_text(&buffer, 2);
    assert_eq!('◀', target.chars().nth(50).unwrap(), "CI row: {target:?}");
    assert_eq!('╮', target.chars().nth(GUTTER_COLUMN).unwrap());
    assert_eq!(Color::Yellow, buffer[(5, 2)].fg);
    assert_eq!(Color::Green, buffer[(5, 29)].fg);
}

#[test]
pub(super) fn pointer_arrow_jrp_adds_the_word_to_ci_in_cyan() {
    // JRP adds the word to CI: the same data flow as JMP (word to CI),
    // but cyan like SUB, marking an arithmetic combination.
    let mut store = [0; 32];
    store[10] = Store::parse_word("JRP 3").unwrap();
    let snap = pointer_snapshot(10, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The source word (screen row 8) leaves the text and turns up
    let source = row_text(&buffer, 8);
    assert_eq!(
        '╯',
        source.chars().nth(GUTTER_COLUMN).unwrap(),
        "source row: {source:?}"
    );

    // The vertical crosses the accumulator row (screen row 3)
    for row in 3..8 {
        let text = row_text(&buffer, row);
        assert_eq!(
            '│',
            text.chars().nth(GUTTER_COLUMN).unwrap(),
            "row {row}: {text:?}"
        );
    }
    assert_eq!(Color::Cyan, buffer[(GUTTER_COLUMN as u16, 3)].fg);
    assert_eq!(Color::Cyan, buffer[(GUTTER_COLUMN as u16, 8)].fg);

    // The arrow arrives at the CI row, the highlighted destination
    let target = row_text(&buffer, 2);
    assert_eq!('◀', target.chars().nth(50).unwrap(), "CI row: {target:?}");
    assert_eq!(Color::Cyan, buffer[(GUTTER_COLUMN as u16, 2)].fg);
    assert_eq!(Color::Yellow, buffer[(5, 2)].fg);
}

#[test]
pub(super) fn pointer_arrow_aligns_with_the_heatmap() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 10").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.heatmap = HeatmapMode::Exec;
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // With the heatmap on, the gutter shifts right by the heatmap width
    let extra = HeatmapMode::Exec.extra_width() as usize;
    let gutter = GUTTER_COLUMN + extra;
    let arrowhead = ARROWHEAD_COLUMN + extra;

    // The arrow leaves the accumulator (row 3) and turns down
    let source = row_text(&buffer, 3);
    assert_eq!('╮', source.chars().nth(gutter).unwrap(), "row: {source:?}");

    // Intermediate rows hold the vertical segment
    for row in 4..15 {
        let text = row_text(&buffer, row);
        assert_eq!(
            '│',
            text.chars().nth(gutter).unwrap(),
            "row {row}: {text:?}"
        );
    }

    // The destination word (row 15) receives the arrowhead
    let target = row_text(&buffer, 15);
    assert_eq!(
        '◀',
        target.chars().nth(arrowhead).unwrap(),
        "target row: {target:?}"
    );
    assert_eq!('╯', target.chars().nth(gutter).unwrap());
}

#[test]
pub(super) fn pointer_arrow_jmp_zero_points_from_word_zero_to_ci() {
    // Line 0 holds JMP 0 (an empty store word): the target value comes
    // from word 0 itself, so the arrow runs from row 5 up to the CI row.
    let snap = pointer_snapshot(0, [0; 32]);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    let source = row_text(&buffer, 5);
    assert_eq!(
        '╯',
        source.chars().nth(GUTTER_COLUMN).unwrap(),
        "source row: {source:?}"
    );
    let target = row_text(&buffer, 2);
    assert_eq!('◀', target.chars().nth(50).unwrap(), "CI row: {target:?}");
}

#[test]
pub(super) fn pointer_arrow_and_marker_ignore_ci() {
    // The editor can set CI anywhere; the marker and the arrow follow
    // the last executed instruction, not CI. CI 99 (out of range)
    // changes nothing.
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 24").unwrap();
    let mut snap = pointer_snapshot(2, store);
    snap.ci = 99;

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The marker stays on the executed line (screen row 7)
    assert!(row_text(&buffer, 7).starts_with('>'));
    // The arrow still runs from the accumulator to word 24
    let source = row_text(&buffer, 3);
    assert_eq!(
        '╮',
        source.chars().nth(GUTTER_COLUMN).unwrap(),
        "source row: {source:?}"
    );
    let target = row_text(&buffer, 29);
    assert_eq!(
        '◀',
        target.chars().nth(ARROWHEAD_COLUMN).unwrap(),
        "target row: {target:?}"
    );
}

#[test]
pub(super) fn marker_and_arrow_follow_the_jump_not_its_target() {
    // After `JMP 2` at line 1 executes with store[2] = 20, CI = 20.
    // The word at line 20 is neither the last nor the next instruction
    // (CI is incremented before the next fetch), so the marker and the
    // arrow must describe the JMP at line 1.
    let mut store = [0; 32];
    store[1] = Store::parse_word("JMP 2").unwrap();
    store[2] = 20; // the value JMP loads into CI
    store[20] = Store::parse_word("STO 25").unwrap();
    let snap = Snapshot {
        ci: 20,
        ..pointer_snapshot(1, store)
    };

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The marker is on the JMP line (screen row 6), not on line 20 (row 25)
    assert!(row_text(&buffer, 6).starts_with('>'));
    assert!(!row_text(&buffer, 25).starts_with('>'));

    // The arrow is the JMP's data flow: from the word holding the target
    // value (line 2, row 7) up to the CI register (row 2)
    let source = row_text(&buffer, 7);
    assert_eq!(
        '╯',
        source.chars().nth(GUTTER_COLUMN).unwrap(),
        "source row: {source:?}"
    );
    let target = row_text(&buffer, 2);
    assert_eq!('◀', target.chars().nth(50).unwrap(), "CI row: {target:?}");
    assert_eq!(Color::Yellow, buffer[(5, 2)].fg, "CI is the destination");

    // The word CI points at gets no marker and no arrow: its STO would
    // otherwise draw an arrow from the accumulator to that word
    let ci_line = row_text(&buffer, 25);
    assert!(!has_arrow_glyph(&ci_line), "CI line row: {ci_line:?}");
    // The accumulator row only carries the arrow's vertical segment:
    // no arrowhead, and its bits stay green
    let accumulator = row_text(&buffer, 3);
    assert_eq!(
        '│',
        accumulator.chars().nth(GUTTER_COLUMN).unwrap(),
        "accumulator row: {accumulator:?}"
    );
    assert!(
        !accumulator.contains('◀'),
        "accumulator row: {accumulator:?}"
    );
    assert_eq!(Color::Green, buffer[(5, 3)].fg);
}

#[test]
pub(super) fn no_marker_or_arrow_before_the_first_cycle() {
    // Nothing has executed yet (fresh machine, or scrubbed back to
    // cycle 0): no store line is marked and no arrow is drawn.
    let snap = snapshot_with_history(0);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    for row in 5..=36 {
        let text = row_text(&buffer, row);
        assert!(
            !text.starts_with('>'),
            "row {row} should not be marked: {text:?}"
        );
        assert!(!has_arrow_glyph(&text), "row {row}: {text:?}");
    }
}

#[test]
pub(super) fn marker_and_arrow_survive_history_scrubbing() {
    // A jump is recorded in the history; rewinding to the post-jump
    // cycle restores the last instruction, so the marker and the arrow
    // still describe the JMP instead of the word CI points at.
    let mut sim = Simulator::new();
    sim.store.words[1] = Store::parse_word("JMP 2").unwrap();
    sim.store.words[2] = 5; // CI becomes 5; the next fetch is line 6
    sim.store.words[5] = Store::parse_word("STO 25").unwrap();
    sim.store.words[6] = Store::parse_word("STP").unwrap();
    sim.enable_history(100);
    sim.run(2); // cycle 1: JMP at line 1, CI → 5; cycle 2: STP at line 6

    assert!(sim.rewind_to(1));
    assert_eq!(5, sim.ci);
    let snap = Snapshot::from_sim(&sim);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The marker is on the JMP line (screen row 6), not on line 5 (row 10)
    assert!(row_text(&buffer, 6).starts_with('>'));
    assert!(!row_text(&buffer, 10).starts_with('>'));
    // The arrow is the JMP's data flow: from word 2 (row 7) up to the
    // CI register (row 2)
    assert_eq!(
        '╯',
        row_text(&buffer, 7).chars().nth(GUTTER_COLUMN).unwrap()
    );
    assert_eq!('◀', row_text(&buffer, 2).chars().nth(50).unwrap());
}

#[test]
pub(super) fn pointer_arrow_hidden_for_operandless_instructions() {
    let mut store = [0; 32];
    store[1] = Store::parse_word("STP").unwrap();
    let snap = pointer_snapshot(1, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // No arrow glyph anywhere in the main body
    for row in 1..=37 {
        let text = row_text(&buffer, row);
        assert!(!has_arrow_glyph(&text), "row {row}: {text:?}");
    }
}

#[test]
pub(super) fn pointer_arrow_hidden_when_toggled_off() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 24").unwrap();
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Off;
    let buffer = render(&mut app, &snap, 80, 40);

    for row in 1..=37 {
        let text = row_text(&buffer, row);
        assert!(!has_arrow_glyph(&text), "row {row}: {text:?}");
    }
}

#[test]
pub(super) fn pointer_arrows_shown_by_default() {
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 24").unwrap();
    let snap = pointer_snapshot(2, store);

    // No toggle needed: the arrow is drawn from the start
    let mut app = App::new();
    assert_eq!(FlowMode::Data, app.flow);
    let buffer = render(&mut app, &snap, 80, 40);
    let source = row_text(&buffer, 3);
    assert_eq!(
        '╮',
        source.chars().nth(GUTTER_COLUMN).unwrap(),
        "source row: {source:?}"
    );
}

#[test]
pub(super) fn flow_mode_cycles_with_the_f_key() {
    let mut sim = Simulator::new();
    let mut app = App::new();
    assert_eq!(FlowMode::Data, app.flow);

    // data → control → off → data
    for expected in [FlowMode::Control, FlowMode::Off, FlowMode::Data] {
        app.handle_key(
            KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE),
            &mut sim,
        );
        assert_eq!(expected, app.flow);
    }

    // Uppercase cycles too
    app.handle_key(
        KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE),
        &mut sim,
    );
    assert_eq!(FlowMode::Control, app.flow);
}

#[test]
pub(super) fn pointer_arrows_crop_the_right_side_when_narrow() {
    let snap = pointer_snapshot(0, [0; 32]);
    let mut app = App::new();
    app.flow = FlowMode::Data;

    // Too narrow for the store text: the "too small" message is shown
    let buffer = render(&mut app, &snap, MIN_WIDTH - 1, 40);
    assert!(row_text(&buffer, 0).contains("Terminal too small"));

    // At MIN_WIDTH the store renders; the arrow gutter at the right edge
    // (cols 59+) is cropped off
    let buffer = render(&mut app, &snap, MIN_WIDTH, 40);
    assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    assert!(row_text(&buffer, 5).contains("00:"));

    // Wide enough: the arrow is fully drawn
    let buffer = render(&mut app, &snap, MIN_WIDTH + ARROW_EXTRA_WIDTH, 40);
    assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    let row = row_text(&buffer, 5);
    assert_eq!('╯', row.chars().nth(GUTTER_COLUMN).unwrap(), "row: {row:?}");
}
