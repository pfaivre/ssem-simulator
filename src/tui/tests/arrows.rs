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

    // The accumulator is the source, not the destination: it stays green
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

    // The source word is not the destination: it stays green
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

    // The arrow arrives at the CI row (screen row 2), the highlighted
    // destination; the source word is not the destination
    let target = row_text(&buffer, 2);
    assert_eq!('◀', target.chars().nth(50).unwrap(), "CI row: {target:?}");
    assert_eq!('╮', target.chars().nth(GUTTER_COLUMN).unwrap());
}

#[test]
pub(super) fn pointer_arrow_jrp_adds_the_word_to_ci_like_jmp() {
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

    // The arrow arrives at the CI row, the highlighted destination
    let target = row_text(&buffer, 2);
    assert_eq!('◀', target.chars().nth(50).unwrap(), "CI row: {target:?}");
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
pub(super) fn flow_mode_cycles_with_the_f_key() {
    let mut sim = Simulator::new();
    let mut app = App::new();
    assert_eq!(FlowMode::Control, app.flow);

    // data → control → off → data
    for expected in [FlowMode::Data, FlowMode::Off, FlowMode::Control] {
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
    assert_eq!(FlowMode::Data, app.flow);
}

/// The text drawn after the arrow's top-row gutter cell: the assignment
/// label starts right after the corner column, one space in.
fn margin_label(buffer: &Buffer, row: u16) -> String {
    row_text(buffer, row)
        .chars()
        .skip(GUTTER_COLUMN + 1)
        .collect()
}

#[test]
pub(super) fn assignment_label_sto_is_on_the_accumulator_line() {
    // STO 24 with A = 54: the arrow runs from A down to word 24, and its
    // top line is the accumulator row, labeled "[24] = 54" in magenta
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 24").unwrap();
    let mut snap = pointer_snapshot(2, store);
    snap.a = 54;

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    // The label follows the corner on the accumulator row (screen row 3)
    let a_row = margin_label(&buffer, 3);
    assert!(
        a_row.starts_with(" [24] = 54"),
        "accumulator row: {a_row:?}"
    );

    // Only the top of the arrow carries the label: the CI row and the
    // destination word row do not
    assert!(!margin_label(&buffer, 2).contains('='), "CI row");
    assert!(!row_text(&buffer, 29).contains("[24]"), "target row");
}

#[test]
pub(super) fn assignment_label_ldn_is_absolute_on_the_accumulator_line() {
    // LDN 10 loads A = -store[10]: an absolute "A = ..." label
    let mut store = [0; 32];
    store[2] = Store::parse_word("LDN 10").unwrap();
    store[10] = -128;
    let mut snap = pointer_snapshot(2, store);
    snap.a = 128;

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    let a_row = margin_label(&buffer, 3);
    assert!(a_row.starts_with(" A = 128"), "accumulator row: {a_row:?}");
    assert!(!margin_label(&buffer, 2).contains('='), "CI row");
}

#[test]
pub(super) fn assignment_label_sub_is_relative_and_cyan() {
    // SUB 10 computes A -= store[10]: shown as the relative "A += -3"
    let mut store = [0; 32];
    store[2] = Store::parse_word("SUB 10").unwrap();
    store[10] = 3;
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    let a_row = margin_label(&buffer, 3);
    assert!(a_row.starts_with(" A += -3"), "accumulator row: {a_row:?}");
}

#[test]
pub(super) fn assignment_label_jmp_is_on_the_ci_line() {
    // JMP 2 with store[2] = 20 copies the word into CI: "CI = 20",
    // absolute, on the CI row (the arrow's top line)
    let mut store = [0; 32];
    store[1] = Store::parse_word("JMP 2").unwrap();
    store[2] = 20;
    let mut snap = pointer_snapshot(1, store);
    snap.ci = 20;

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    let ci_row = margin_label(&buffer, 2);
    assert!(ci_row.starts_with(" CI = 20"), "CI row: {ci_row:?}");
    assert!(!margin_label(&buffer, 3).contains('='), "accumulator row");
}

#[test]
pub(super) fn assignment_label_jrp_is_relative_on_the_ci_line() {
    // JRP 3 adds store[3] to CI: the relative "CI += -3", in cyan
    let mut store = [0; 32];
    store[10] = Store::parse_word("JRP 3").unwrap();
    store[3] = -3;
    let mut snap = pointer_snapshot(10, store);
    snap.ci = 7; // (10 - 3) & 31, as the machine leaves it

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);

    let ci_row = margin_label(&buffer, 2);
    assert!(ci_row.starts_with(" CI += -3"), "CI row: {ci_row:?}");
}

#[test]
pub(super) fn no_assignment_label_without_an_arrow() {
    // CMP and STP move no data: no arrow, so no label either
    let mut store = [0; 32];
    store[1] = Store::parse_word("CMP").unwrap();
    let snap = pointer_snapshot(1, store);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 80, 40);
    assert!(!margin_label(&buffer, 2).contains('='), "CI row");
    assert!(!margin_label(&buffer, 3).contains('='), "accumulator row");

    // Flow view off: the arrow is gone and so is the label
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 24").unwrap();
    let mut snap = pointer_snapshot(2, store);
    snap.a = 54;
    app.flow = FlowMode::Off;
    let buffer = render(&mut app, &snap, 80, 40);
    assert!(
        !margin_label(&buffer, 3).contains("[24]"),
        "accumulator row"
    );
}

#[test]
pub(super) fn assignment_label_keeps_the_comment_column_offset() {
    // The label overflows past the gutter on the register row only; the
    // comment column on the store rows keeps its offset
    let mut store = [0; 32];
    store[2] = Store::parse_word("STO 10").unwrap();
    let mut comments: [Option<String>; 32] = std::array::from_fn(|_| None);
    comments[10] = Some(" stored".into());
    let mut snap = pointer_snapshot(2, store);
    snap.a = 54;
    snap.comments = Arc::new(comments);

    let mut app = App::new();
    app.flow = FlowMode::Data;
    let buffer = render(&mut app, &snap, 100, 40);

    // The label is on the accumulator row, after the corner
    assert!(margin_label(&buffer, 3).starts_with(" [10] = 54"));
    // The comment of word 10 still starts right after the 4-cell gutter
    let col = STORE_TEXT_WIDTH + ARROW_EXTRA_WIDTH as usize;
    let comment: String = row_text(&buffer, 15).chars().skip(col).collect();
    assert!(comment.starts_with(" ; stored"), "row: {comment:?}");
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
