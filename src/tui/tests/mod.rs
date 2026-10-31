//! Tests for the TUI, split by concern; the helpers shared by all of them
//! live in this file.

mod arrows;
mod graph;
mod heatmap;
mod render;
mod timeline;

use super::app::App;
use super::draw::ARROW_EXTRA_WIDTH;
use super::graph::{
    CfgEdge, EdgeKind, GRAPH_EXTRA_WIDTH, GUTTER_COLS, GraphGutter, MAX_EDGE_LANES,
    OVERFLOW_LABEL_WIDTH, assign_lanes, control_flow_edges, line_row, reachable_edges,
};
use super::layout::{
    CONTENT_HEIGHT, MIN_WIDTH, STORE_TEXT_WIDTH, timeline_bar_geometry, timeline_marker_offset,
};
use super::snapshot::Snapshot;
use super::theme::{FlowMode, HeatmapMode};
use crate::ssem::Store;
use crate::ssem::simulator::{LastInstruction, Simulator};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// A simulator that never halts: an empty store executes `JMP 0` forever.
fn looping_sim(cycles: u32) -> Simulator {
    let mut sim = Simulator::new();
    sim.enable_history(100);
    sim.run(cycles);
    sim
}

/// A snapshot with history enabled, as the UI would see it while paused.
fn snapshot_with_history(cycle_count: u64) -> Snapshot {
    Snapshot {
        ci: 0,
        a: 0,
        store: [0; 32],
        cycle_count,
        speed: 700,
        stop_flag: true,
        last_instruction: None,
        breakpoints: Vec::new(),
        exec_counts: [0; 32],
        read_counts: [0; 32],
        write_counts: [0; 32],
        history_capacity: 100,
        history_start: 0,
        history_head: 99,
        history_len: 100,
    }
}

/// Render the app into a test terminal and return the resulting buffer.
fn render(app: &mut App, snap: &Snapshot, width: u16, height: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| app.draw(f, snap)).unwrap();
    terminal.backend().buffer().clone()
}

/// The text of one screen row, one character per cell.
fn row_text(buffer: &Buffer, row: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, row)].symbol())
        .collect()
}

/// A snapshot whose last executed instruction is the word at `line`,
/// for pointer-arrow tests. CI is set to `line`, as the machine leaves
/// it after executing a non-jump instruction.
fn pointer_snapshot(line: i32, store: [i32; 32]) -> Snapshot {
    let (opcode, data) = Store::decode_word(store[line as usize]);
    Snapshot {
        ci: line,
        last_instruction: Some(LastInstruction {
            address: line,
            opcode,
            data,
        }),
        store,
        ..snapshot_with_history(0)
    }
}

/// Screen column of the pointer arrow's gutter (right after the 58-cell
/// line text, a separator, and the horizontal segment). Store line `index`
/// is drawn on screen row `5 + index`, the accumulator on row 3.
const GUTTER_COLUMN: usize = 61;

const ARROWHEAD_COLUMN: usize = 59;

/// Whether a pointer-arrow glyph is drawn anywhere in the given row text.
fn has_arrow_glyph(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c, '╮' | '╯' | '│' | '◀' | '↺'))
}
