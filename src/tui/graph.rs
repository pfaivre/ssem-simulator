//! The static control-flow graph between store lines, drawn in the right
//! margin in the "control" flow mode.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::layout::{CONTENT_HEIGHT, STORE_ROW_OFFSET};
use crate::ssem::{Opcode, Store};

/// Maximum number of edge lanes drawn side by side in the graph gutter.
pub(super) const MAX_EDGE_LANES: usize = 7;

/// Columns in the graph gutter: a separator, then per lane an arrowhead
/// column, a dash column, and the corner/vertical column.
pub(super) const GUTTER_COLS: usize = 1 + 3 * MAX_EDGE_LANES;

/// Width of the overflow label ("⇢ 31") drawn after the lanes for an edge
/// that finds no free lane.
#[cfg(test)]
pub(super) const OVERFLOW_LABEL_WIDTH: usize = 4;

/// Extra screen width required by the control-flow graph view.
#[cfg(test)]
pub(super) const GRAPH_EXTRA_WIDTH: u16 = (GUTTER_COLS + OVERFLOW_LABEL_WIDTH) as u16;

/// The kind of a control-flow edge, which decides its color in the graph view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EdgeKind {
    /// Straight-line execution to the next line
    /// (LDN, STO, SUB, SUB2, and CMP when not taken).
    FallThrough,
    /// JMP or JRP: control moves to the jump's target line.
    Jump,
    /// CMP taken: control skips the next line (i → i+2).
    Skip,
}

/// A static control-flow edge from one store line to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CfgEdge {
    pub(super) source: usize,
    pub(super) target: usize,
    pub(super) kind: EdgeKind,
}

/// The static control-flow edges of the program held in `store`.
///
/// CI is incremented before each fetch, so the line executed after a jump is
/// the target value plus one: JMP `d` lands on `(store[d] + 1) & 31`, and a
/// JRP `d` at line `i` lands on `(i + store[d] + 1) & 31`. CMP has two
/// successors (fall-through and skip); STP has none.
pub(super) fn control_flow_edges(store: &[i32; 32]) -> Vec<CfgEdge> {
    const MASK: i32 = Store::INDEX_MASK;
    let mut edges = Vec::with_capacity(32);
    for i in 0..32 {
        let (opcode, data) = Store::decode_word(store[i]);
        let line = i as i32;
        match opcode {
            Opcode::JMP => {
                let target = ((store[data as usize] & MASK) + 1) & MASK;
                edges.push(CfgEdge {
                    source: i,
                    target: target as usize,
                    kind: EdgeKind::Jump,
                });
            }
            Opcode::JRP => {
                let ci = line.wrapping_add(store[data as usize]) & MASK;
                edges.push(CfgEdge {
                    source: i,
                    target: ((ci + 1) & MASK) as usize,
                    kind: EdgeKind::Jump,
                });
            }
            Opcode::CMP => {
                edges.push(CfgEdge {
                    source: i,
                    target: ((line + 1) & MASK) as usize,
                    kind: EdgeKind::FallThrough,
                });
                edges.push(CfgEdge {
                    source: i,
                    target: ((line + 2) & MASK) as usize,
                    kind: EdgeKind::Skip,
                });
            }
            Opcode::STP => {}
            _ => edges.push(CfgEdge {
                source: i,
                target: ((line + 1) & MASK) as usize,
                kind: EdgeKind::FallThrough,
            }),
        }
    }
    edges
}

/// The store line executed first on a fresh machine: CI starts at 0 and is
/// incremented before the first fetch, so execution starts at line 1.
const ENTRY_LINE: usize = 1;

/// Restrict the control-flow edges to the lines reachable from the entry
/// line.
///
/// Data words decode as arbitrary instructions, so without this filter every
/// NUM line would spawn a spurious edge. Returns the edges whose source is
/// reachable, along with the reachable set itself (used to mark STP lines).
pub(super) fn reachable_edges(store: &[i32; 32]) -> (Vec<CfgEdge>, [bool; 32]) {
    let edges = control_flow_edges(store);
    let mut reachable = [false; 32];
    reachable[ENTRY_LINE] = true;
    let mut stack = vec![ENTRY_LINE];
    while let Some(line) = stack.pop() {
        for edge in &edges {
            if edge.source == line && !reachable[edge.target] {
                reachable[edge.target] = true;
                stack.push(edge.target);
            }
        }
    }
    let edges = edges
        .into_iter()
        .filter(|edge| reachable[edge.source])
        .collect();
    (edges, reachable)
}

/// The style of a control-flow edge: a backward edge (target at or above the
/// source) closes a loop and is highlighted in bold red; forward edges take
/// the color of their kind.
fn edge_style(edge: &CfgEdge) -> Style {
    if edge.target <= edge.source {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        match edge.kind {
            EdgeKind::FallThrough => Style::default().fg(Color::DarkGray),
            EdgeKind::Jump => Style::default().fg(Color::Yellow),
            EdgeKind::Skip => Style::default().fg(Color::White),
        }
    }
}

/// The content row of store line `line` in the main body.
pub(super) fn line_row(line: usize) -> usize {
    STORE_ROW_OFFSET + line
}

/// Assign each edge a gutter lane so that no two edges share a lane, not
/// even at a single row.
///
/// An edge occupies its lane from its source row through its target row
/// inclusive: a lane is free for a new edge only when the previous one ends
/// on an earlier row. Letting an arriving and a leaving edge share a row
/// would merge their corners into one continuous-looking line. Edges are
/// packed earliest-start first, longest span first, so short edges fill the
/// gaps between long ones. Returns `None` for edges that find no free lane;
/// those are drawn as a source-row label instead.
pub(super) fn assign_lanes(edges: &[CfgEdge]) -> Vec<Option<usize>> {
    let span = |e: &CfgEdge| {
        let (s, t) = (line_row(e.source), line_row(e.target));
        (s.min(t), s.max(t))
    };
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|&i| {
        let (start, end) = span(&edges[i]);
        (start, std::cmp::Reverse(end - start))
    });
    let mut lane_end = [0; MAX_EDGE_LANES];
    let mut lanes = vec![None; edges.len()];
    for i in order {
        let (start, end) = span(&edges[i]);
        if let Some(lane) = lane_end.iter().position(|&end_at| end_at < start) {
            lane_end[lane] = end;
            lanes[i] = Some(lane);
        }
    }
    lanes
}

/// Merge two glyphs drawn into the same gutter cell. Corners and arrowheads
/// win over verticals, verticals over horizontals — so where a horizontal
/// meets a vertical, only the vertical is printed and the horizontal passes
/// underneath (the vertical edge reads as drawn on top). Two opposite
/// corners become a straight line.
fn merge_glyph(existing: char, new: char) -> char {
    fn rank(c: char) -> u8 {
        match c {
            '╮' | '╯' => 4,
            '◀' | '↺' | '■' => 3,
            '│' => 2,
            _ => 1, // ─ and the rest
        }
    }
    match (existing, new) {
        ('╮', '╯') | ('╯', '╮') => '│',
        _ => {
            if rank(new) > rank(existing) {
                new
            } else {
                existing
            }
        }
    }
}

/// Draw one glyph into a gutter cell, merging with whatever is already there.
/// When the merged glyph is the vertical of the pair, the vertical edge's
/// style is kept (`merged == glyph` when the new glyph is the vertical,
/// `merged == existing` when it already was), so the cell shows the
/// on-top edge's color.
fn set_cell(
    cells: &mut [Vec<Option<(char, Style)>>],
    row: usize,
    col: usize,
    glyph: char,
    style: Style,
) {
    let cell = &mut cells[row][col];
    *cell = Some(match *cell {
        None => (glyph, style),
        Some((existing, existing_style)) => {
            let merged = merge_glyph(existing, glyph);
            let style = if merged == glyph && merged != existing {
                style
            } else {
                existing_style
            };
            (merged, style)
        }
    });
}

/// The control-flow graph gutter: a cell grid of `CONTENT_HEIGHT` rows by
/// `GUTTER_COLS` columns, plus an overflow label per row for edges that found
/// no lane. Rebuilt from the current store contents on every frame.
pub(super) struct GraphGutter {
    pub(super) cells: Vec<Vec<Option<(char, Style)>>>,
    pub(super) overflow: Vec<Option<(usize, Style)>>,
}

impl GraphGutter {
    pub(super) fn new(store: &[i32; 32]) -> Self {
        let (mut edges, reachable) = reachable_edges(store);
        // Fall-through to the next line is the default flow and is left
        // implicit; only jumps, CMP skips, and backward edges (loops,
        // including the 31→0 wrap) are drawn.
        edges.retain(|edge| edge.kind != EdgeKind::FallThrough || edge.target <= edge.source);
        let lanes = assign_lanes(&edges);
        let mut cells = vec![vec![None; GUTTER_COLS]; CONTENT_HEIGHT];
        let mut overflow = vec![None; CONTENT_HEIGHT];

        for (i, edge) in edges.iter().enumerate() {
            let style = edge_style(edge);
            let source_row = line_row(edge.source);
            let target_row = line_row(edge.target);
            let Some(lane) = lanes[i] else {
                // No free lane: label the source row with the target line
                overflow[source_row] = Some((edge.target, style));
                continue;
            };
            let head_col = 1 + 3 * lane;
            let vert_col = 3 + 3 * lane;
            if source_row == target_row {
                // Self-loop: a circular arrow in the separator column
                set_cell(&mut cells, source_row, 0, '↺', style);
                continue;
            }
            let (top, bottom) = (source_row.min(target_row), source_row.max(target_row));
            // The edge leaves the source line and turns into its lane
            for col in 1..vert_col {
                set_cell(&mut cells, source_row, col, '─', style);
            }
            set_cell(
                &mut cells,
                source_row,
                vert_col,
                if target_row > source_row {
                    '╮'
                } else {
                    '╯'
                },
                style,
            );
            // The vertical segment between the two endpoints
            for row in (top + 1)..bottom {
                set_cell(&mut cells, row, vert_col, '│', style);
            }
            // The edge arrives at the target line: arrowhead, a dash
            // spacing it from the corner
            set_cell(&mut cells, target_row, head_col, '◀', style);
            set_cell(&mut cells, target_row, head_col + 1, '─', style);
            set_cell(
                &mut cells,
                target_row,
                vert_col,
                if target_row > source_row {
                    '╯'
                } else {
                    '╮'
                },
                style,
            );
        }

        // Reachable STP lines halt the flow: mark them in the separator column
        for (line, word) in store.iter().enumerate() {
            if reachable[line] && Store::decode_word(*word).0 == Opcode::STP {
                set_cell(
                    &mut cells,
                    line_row(line),
                    0,
                    '■',
                    Style::default().fg(Color::Red),
                );
            }
        }

        GraphGutter { cells, overflow }
    }

    /// The spans of one row's gutter: fixed-width cells grouped by style,
    /// plus the overflow label when the row has one.
    pub(super) fn row_spans(&self, row: usize) -> Vec<Span<'static>> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut run = String::new();
        let mut run_style = Style::default();
        for col in 0..GUTTER_COLS {
            let (glyph, style) = self.cells[row][col].unwrap_or((' ', Style::default()));
            if style != run_style && !run.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut run), run_style));
            }
            if run.is_empty() {
                run_style = style;
            }
            run.push(glyph);
        }
        if !run.is_empty() {
            spans.push(Span::styled(run, run_style));
        }
        if let Some((target, style)) = self.overflow[row] {
            spans.push(Span::styled(format!("⇢ {target:02}"), style));
        }
        spans
    }
}
