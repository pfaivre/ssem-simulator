use super::*;

/// A store where every line is STP (no control-flow edges), except the
/// given (line, word) entries.
pub(super) fn graph_store(entries: &[(usize, &str)]) -> [i32; 32] {
    let mut store = [Store::parse_word("STP").unwrap(); 32];
    for &(line, text) in entries {
        store[line] = Store::parse_word(text).unwrap();
    }
    store
}

/// Screen column of the graph gutter's separator (heatmap off).
pub(super) const G_SEP: usize = STORE_TEXT_WIDTH;

/// Screen column of lane `l`'s arrowhead / vertical in the graph gutter.
pub(super) fn g_head(l: usize) -> usize {
    STORE_TEXT_WIDTH + 1 + 3 * l
}

pub(super) fn g_vert(l: usize) -> usize {
    STORE_TEXT_WIDTH + 3 + 3 * l
}

#[test]
pub(super) fn cfg_edges_are_empty_when_every_line_is_stp() {
    let store = graph_store(&[]);
    assert!(control_flow_edges(&store).is_empty());
}

#[test]
pub(super) fn cfg_edges_fall_through_to_the_next_line() {
    let store = graph_store(&[(1, "LDN 24")]);
    let edges = control_flow_edges(&store);
    assert_eq!(
        edges,
        vec![CfgEdge {
            source: 1,
            target: 2,
            kind: EdgeKind::FallThrough,
        }]
    );
}

#[test]
pub(super) fn cfg_edges_jmp_lands_on_operand_value_plus_one() {
    // JMP copies store[d] into CI; CI is incremented before the next
    // fetch, so the next executed line is (store[d] + 1) & 31.
    // NUM -23 & 31 = 9, so JMP 24 at line 2 lands on line 10.
    let store = graph_store(&[(2, "JMP 24"), (24, "NUM -23")]);
    let edges = control_flow_edges(&store);
    assert!(edges.contains(&CfgEdge {
        source: 2,
        target: 10,
        kind: EdgeKind::Jump,
    }));
}

#[test]
pub(super) fn cfg_edges_jrp_adds_the_operand_to_the_executed_line() {
    // JRP adds store[d] to CI (= the executed line); the next fetch is
    // one further: 8 + (-4) + 1 = 5.
    let store = graph_store(&[(8, "JRP 20"), (20, "NUM -4")]);
    let edges = control_flow_edges(&store);
    assert!(edges.contains(&CfgEdge {
        source: 8,
        target: 5,
        kind: EdgeKind::Jump,
    }));
}

#[test]
pub(super) fn cfg_edges_cmp_has_a_fall_through_and_a_skip_edge() {
    let store = graph_store(&[(7, "CMP")]);
    let edges = control_flow_edges(&store);
    assert!(edges.contains(&CfgEdge {
        source: 7,
        target: 8,
        kind: EdgeKind::FallThrough,
    }));
    assert!(edges.contains(&CfgEdge {
        source: 7,
        target: 9,
        kind: EdgeKind::Skip,
    }));
}

#[test]
pub(super) fn cfg_edges_wrap_around_the_store_end() {
    let store = graph_store(&[(31, "LDN 0")]);
    let edges = control_flow_edges(&store);
    assert!(edges.contains(&CfgEdge {
        source: 31,
        target: 0,
        kind: EdgeKind::FallThrough,
    }));
}

#[test]
pub(super) fn cfg_edges_support_self_loops() {
    // JRP 30 at line 3 with store[30] = -1: 3 - 1 + 1 = 3
    let store = graph_store(&[(3, "JRP 30"), (30, "NUM -1")]);
    let edges = control_flow_edges(&store);
    assert!(edges.contains(&CfgEdge {
        source: 3,
        target: 3,
        kind: EdgeKind::Jump,
    }));
}

#[test]
pub(super) fn lanes_are_shared_when_edges_do_not_touch() {
    // 1→2 ends on line 2's row and 4→5 starts on line 4's row: with a gap
    // between them, the second edge reuses lane 0
    let edges = vec![
        CfgEdge {
            source: 1,
            target: 2,
            kind: EdgeKind::FallThrough,
        },
        CfgEdge {
            source: 4,
            target: 5,
            kind: EdgeKind::FallThrough,
        },
    ];
    assert_eq!(assign_lanes(&edges), vec![Some(0), Some(0)]);
}

#[test]
pub(super) fn touching_edges_get_different_lanes() {
    // 1→2 ends on line 2's row, where 2→3 starts: sharing that row would
    // merge the two arrows into one continuous-looking line, so the second
    // edge gets its own lane
    let edges = vec![
        CfgEdge {
            source: 1,
            target: 2,
            kind: EdgeKind::FallThrough,
        },
        CfgEdge {
            source: 2,
            target: 3,
            kind: EdgeKind::FallThrough,
        },
    ];
    assert_eq!(assign_lanes(&edges), vec![Some(0), Some(1)]);
}

#[test]
pub(super) fn overlapping_edges_get_different_lanes() {
    let edges = vec![
        CfgEdge {
            source: 1,
            target: 10,
            kind: EdgeKind::Jump,
        },
        CfgEdge {
            source: 5,
            target: 6,
            kind: EdgeKind::FallThrough,
        },
    ];
    let lanes = assign_lanes(&edges);
    assert_eq!(lanes[0], Some(0));
    assert_eq!(lanes[1], Some(1));
}

#[test]
pub(super) fn more_overlapping_edges_than_lanes_overflow_to_none() {
    let edges: Vec<CfgEdge> = (0..=MAX_EDGE_LANES)
        .map(|i| CfgEdge {
            source: i,
            target: 31,
            kind: EdgeKind::Jump,
        })
        .collect();
    let lanes = assign_lanes(&edges);
    assert_eq!(lanes.iter().filter(|l| l.is_none()).count(), 1);
    assert_eq!(lanes.iter().filter(|l| l.is_some()).count(), MAX_EDGE_LANES);
}

#[test]
pub(super) fn reachable_edges_prune_unreachable_lines() {
    // The entry is line 1; with line 1 an STP, nothing else is reachable,
    // so the JMP at line 2 produces no edge even though it decodes fine
    let store = graph_store(&[(2, "JMP 24"), (24, "NUM -23")]);
    let (edges, reachable) = reachable_edges(&store);
    assert!(edges.is_empty());
    assert!(reachable[1]);
    assert!(!reachable[2]);

    // With a fall-through at line 1, lines 1, 2 and 10 become reachable
    let store = graph_store(&[(1, "LDN 0"), (2, "JMP 24"), (24, "NUM -23")]);
    let (edges, reachable) = reachable_edges(&store);
    assert_eq!(edges.len(), 2);
    assert!(reachable[1] && reachable[2] && reachable[10]);
    assert!(!reachable[3]);
}

/// Render the graph view of a store and return the buffer plus helpers
/// reading the glyph at (screen row, column) and the styled cell.
pub(super) fn render_graph(store: [i32; 32]) -> (Buffer, App) {
    let snap = Snapshot {
        store,
        ..snapshot_with_history(0)
    };
    let mut app = App::new();
    app.flow = FlowMode::Control;
    let buffer = render(&mut app, &snap, 100, 40);
    (buffer, app)
}

#[test]
pub(super) fn graph_view_hides_fall_through_but_marks_stops() {
    // 1 → 2 → 3 (STP): the fall-through chain draws nothing...
    let store = graph_store(&[(1, "LDN 0"), (2, "LDN 0"), (3, "STP")]);
    let (buffer, _) = render_graph(store);

    // Store line i is drawn on screen row 5 + i
    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    let cell = |row: u16, col: usize| buffer[(col as u16, row)].clone();

    // ... the gutter of lines 1 and 2 (rows 6-7) stays empty ...
    for row in [6, 7] {
        for col in G_SEP..G_SEP + GUTTER_COLS {
            assert_eq!(' ', glyph(row, col), "row {row} col {col}");
        }
    }
    // ... but the STP at line 3 (row 8) is marked with a red ■
    assert_eq!('■', glyph(8, G_SEP));
    assert_eq!(Color::Red, cell(8, G_SEP).fg);
}

#[test]
pub(super) fn graph_view_draws_a_forward_jump() {
    // 1 → 2, then JMP 20 at line 2 lands on line 10 (NUM -23 & 31 = 9, +1)
    let store = graph_store(&[(1, "LDN 0"), (2, "JMP 20"), (20, "NUM -23")]);
    let (buffer, _) = render_graph(store);

    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    let cell = |row: u16, col: usize| buffer[(col as u16, row)].clone();

    // The fall-through 1→2 draws nothing (row 6 stays empty)
    assert_eq!(' ', glyph(6, g_vert(0)));
    // The jump 2→10 leaves line 2 (row 7) in lane 0, yellow...
    assert_eq!('╮', glyph(7, g_vert(0)));
    assert_eq!(Color::Yellow, cell(7, g_vert(0)).fg);
    // ... runs down lane 0 from row 8 to row 14 ...
    for row in 8..15 {
        assert_eq!('│', glyph(row, g_vert(0)), "row {row}");
        assert_eq!(Color::Yellow, cell(row, g_vert(0)).fg, "row {row}");
    }
    // ... and arrives at line 10 (row 15): arrowhead, a dash spacing it
    // from the corner
    assert_eq!('◀', glyph(15, g_head(0)));
    assert_eq!('─', glyph(15, g_head(0) + 1));
    assert_eq!('╯', glyph(15, g_vert(0)));
    assert_eq!(Color::Yellow, cell(15, g_head(0)).fg);
}

#[test]
pub(super) fn graph_view_draws_only_the_cmp_skip() {
    // 1→2→3→4, and CMP at line 4 branches to 5 (fall-through) and 6 (skip)
    let store = graph_store(&[(1, "LDN 0"), (2, "LDN 0"), (3, "LDN 0"), (4, "CMP")]);
    let (buffer, _) = render_graph(store);

    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    let cell = |row: u16, col: usize| buffer[(col as u16, row)].clone();

    // Only the skip 4→6 is drawn, in lane 0, white: corner leaving line 4
    // (row 9), vertical through row 10, arrowhead arriving at line 6 (row 11)
    assert_eq!('╮', glyph(9, g_vert(0)));
    assert_eq!(Color::White, cell(9, g_vert(0)).fg);
    assert_eq!('│', glyph(10, g_vert(0)));
    assert_eq!(Color::White, cell(10, g_vert(0)).fg);
    assert_eq!('◀', glyph(11, g_head(0)));
    assert_eq!(Color::White, cell(11, g_head(0)).fg);
    // The not-taken branch to line 5 (row 10) draws nothing
    assert_eq!(' ', glyph(10, g_head(1)));
}

#[test]
pub(super) fn graph_view_highlights_backward_jumps_as_loops() {
    // 1→2→3→4→5, and JRP 30 at line 5 jumps back to line 3 (5 + NUM -3 + 1)
    let store = graph_store(&[
        (1, "LDN 0"),
        (2, "LDN 0"),
        (3, "LDN 0"),
        (4, "LDN 0"),
        (5, "JRP 30"),
        (30, "NUM -3"),
    ]);
    let (buffer, _) = render_graph(store);

    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    let cell = |row: u16, col: usize| buffer[(col as u16, row)].clone();

    // The backward edge leaves line 5 (row 10) with an upward corner in
    // lane 0, bold red ...
    assert_eq!('╯', glyph(10, g_vert(0)));
    assert_eq!(Color::Red, cell(10, g_vert(0)).fg);
    assert!(cell(10, g_vert(0)).modifier.contains(Modifier::BOLD));
    // ... runs up through row 9 ...
    assert_eq!('│', glyph(9, g_vert(0)));
    assert_eq!(Color::Red, cell(9, g_vert(0)).fg);
    // ... and arrives at line 3 (row 8)
    assert_eq!('◀', glyph(8, g_head(0)));
    assert_eq!(Color::Red, cell(8, g_head(0)).fg);
}

#[test]
pub(super) fn graph_view_crossing_prints_the_vertical_over_the_horizontal() {
    // CMP at line 10 skips to line 12 (white edge, lane 1); JRP 30 at line
    // 12 with store[30] = -9 jumps back to line 4 (bold red loop, lane 0).
    // The skip's horizontal dashes on line 10's row cross the loop's
    // vertical in lane 0.
    let store = graph_store(&[
        (1, "LDN 0"),
        (2, "LDN 0"),
        (3, "LDN 0"),
        (4, "LDN 0"),
        (5, "LDN 0"),
        (6, "LDN 0"),
        (7, "LDN 0"),
        (8, "LDN 0"),
        (9, "LDN 0"),
        (10, "CMP"),
        (11, "LDN 0"),
        (12, "JRP 30"),
        (30, "NUM -9"),
    ]);
    let (buffer, _) = render_graph(store);

    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    let cell = |row: u16, col: usize| buffer[(col as u16, row)].clone();

    // Store line i is drawn on screen row 5 + i; the crossing sits where
    // the skip's horizontal (line 10's row) meets the loop's vertical
    // (lane 0's column)
    let (row, col) = ((line_row(10) + 1) as u16, g_vert(0));
    // Only the vertical is printed, in the loop edge's bold red: the
    // vertical arrow reads as passing over the horizontal one
    assert_eq!('│', glyph(row, col));
    assert_eq!(Color::Red, cell(row, col).fg);
    assert!(cell(row, col).modifier.contains(Modifier::BOLD));
    assert_eq!(Color::Reset, cell(row, col).bg);
    // The horizontal edge is present on both sides of the crossing...
    for side in [col - 1, col + 1] {
        assert_eq!('─', glyph(row, side), "col {side}");
        assert_eq!(Color::White, cell(row, side).fg, "col {side}");
    }
}

#[test]
pub(super) fn graph_view_draws_self_loops_as_a_circular_arrow() {
    // Line 1 jumps to line 3 (NUM -30 & 31 = 2, +1 = 3), and JRP 30 at
    // line 3 with store[30] = -1 jumps back to line 3 itself
    let store = graph_store(&[(1, "JMP 2"), (2, "NUM -30"), (3, "JRP 30"), (30, "NUM -1")]);
    let (buffer, _) = render_graph(store);

    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    let cell = |row: u16, col: usize| buffer[(col as u16, row)].clone();

    // Line 3 (row 8): ↺ in the separator column, bold red; the incoming
    // jump still arrives in lane 0
    assert_eq!('↺', glyph(8, G_SEP));
    assert_eq!(Color::Red, cell(8, G_SEP).fg);
    assert!(cell(8, G_SEP).modifier.contains(Modifier::BOLD));
    assert_eq!('╯', glyph(8, g_vert(0)));
    // Line 2 decodes as STP but is unreachable: no halt marker
    assert_eq!(' ', glyph(7, G_SEP));
}

#[test]
pub(super) fn graph_gutter_renders_the_overflow_label_after_the_lanes() {
    let mut gutter = GraphGutter {
        cells: vec![vec![None; GUTTER_COLS]; CONTENT_HEIGHT],
        overflow: vec![None; CONTENT_HEIGHT],
    };
    gutter.overflow[line_row(4)] = Some((31, Style::default().fg(Color::Red)));
    let spans = gutter.row_spans(line_row(4));
    let text: String = spans.iter().flat_map(|span| span.content.chars()).collect();
    assert_eq!(GUTTER_COLS + OVERFLOW_LABEL_WIDTH, text.chars().count());
    assert!(text.ends_with("⇢ 31"));
}

#[test]
pub(super) fn graph_view_hides_the_data_flow_arrow() {
    let store = graph_store(&[(2, "STO 24")]);
    // Last instruction: STO 24 at line 2, which would draw a magenta
    // arrow from the accumulator to word 24
    let snap = pointer_snapshot(2, store);

    let mut app = App::new();
    app.flow = FlowMode::Control;
    let buffer = render(&mut app, &snap, 100, 40);

    // The gutter columns carry graph edges only: no magenta arrow
    for y in 0..buffer.area.height {
        for x in STORE_TEXT_WIDTH..(STORE_TEXT_WIDTH + GRAPH_EXTRA_WIDTH as usize) {
            assert_ne!(Color::Magenta, buffer[(x as u16, y)].fg, "cell ({x}, {y})");
        }
    }
}

#[test]
pub(super) fn graph_view_crops_the_right_side_when_narrow() {
    // JMP at line 2 lands on line 10 (NUM -23 & 31 = 9, +1); the STP at
    // line 10 is reachable and marks the gutter's separator column with ■
    let store = graph_store(&[(1, "LDN 0"), (2, "JMP 20"), (10, "STP"), (20, "NUM -23")]);
    let snap = Snapshot {
        store,
        ..snapshot_with_history(0)
    };
    let mut app = App::new();
    app.flow = FlowMode::Control;

    // Too narrow for the store text itself: the "too small" message is shown
    let buffer = render(&mut app, &snap, MIN_WIDTH - 1, 40);
    assert!(row_text(&buffer, 0).contains("Terminal too small"));

    // At MIN_WIDTH the store renders and only the separator column of the
    // gutter fits: the ■ of line 10's STP is at the right edge (col 58),
    // while the jump's lane column (col 61) is cropped off
    let buffer = render(&mut app, &snap, MIN_WIDTH, 40);
    assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    assert!(row_text(&buffer, 6).contains("01:"));
    assert_eq!('■', glyph(15, G_SEP));

    // Wide enough for the whole gutter: the 2→10 jump's lane 0 vertical
    // (col 61) is drawn from row 8 to row 14
    let buffer = render(&mut app, &snap, MIN_WIDTH + GRAPH_EXTRA_WIDTH, 40);
    let glyph = |row: u16, col: usize| row_text(&buffer, row).chars().nth(col).unwrap();
    for row in 8..15 {
        assert_eq!('│', glyph(row, g_vert(0)), "row {row}");
    }
}

#[test]
pub(super) fn flow_mode_is_shown_in_the_top_bar() {
    let snap = snapshot_with_history(0);
    let mut app = App::new();

    // Default mode: data
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(row_text(&buffer, 0).contains("[ FLOW: DATA ]"));

    app.flow = FlowMode::Control;
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(row_text(&buffer, 0).contains("[ FLOW: CONTROL ]"));
    assert!(!row_text(&buffer, 0).contains("DATA ]"));

    // Off: no flow label at all
    app.flow = FlowMode::Off;
    let buffer = render(&mut app, &snap, 120, 40);
    assert!(!row_text(&buffer, 0).contains("FLOW"));
}

#[test]
pub(super) fn help_popup_documents_the_graph_view() {
    let mut app = App::new();
    app.help_popup.open();
    let snap = snapshot_with_history(0);
    let buffer = render(&mut app, &snap, 100, 40);

    let all: String = (0..buffer.area.height)
        .map(|y| row_text(&buffer, y))
        .collect();
    assert!(all.contains("Control-flow graph:"), "graph legend missing");
    assert!(all.contains("CMP skip"), "graph legend missing");
    assert!(all.contains("loop"), "graph legend missing");
}
