//! Interactive terminal interface for the SSEM simulator, built with ratatui.
//!
//! # Architecture: two threads
//!
//! The simulation and the interface run on separate threads, so that a high
//! target speed cannot starve the UI of CPU time:
//!
//! - **Machine thread**: runs the simulation at the target speed. It locks the
//!   simulator, executes a time-limited batch of cycles, releases the lock, and
//!   sleeps until the batch deadline. The short batch duration (~1 ms) keeps the
//!   lock available for the UI thread.
//! - **UI thread** (main): polls for keyboard input at ~60 fps, takes a state
//!   snapshot under a brief lock, and draws without holding the lock (so terminal
//!   I/O never blocks the machine thread).
//!
//! Shared state lives in an `Arc<Mutex<Simulator>>`; a separate
//! `Arc<AtomicBool>` signals the machine thread to exit.
//!
//! The interface is composed of four areas:
//! - A top bar showing status indicators (status, cycles, last instruction, speed, breakpoints)
//! - A main body showing the machine state (CI, accumulator, and store words)
//! - A timeline bar showing the retained execution history
//! - A bottom bar showing the keyboard shortcuts
//!
//! Two pop-ups are available: a help pop-up (H key) and a breakpoint editor (B key).
//! A third pop-up, the memory/register editor (E key), lets you modify CI, A, or store words.
//!
//! Mouse: double-click a line (CI, A, or a store word) to open the editor for it;
//! scroll wheel scrolls the store view.
//!
//! Time-travel: the timeline bar shows the retained execution history. Drag it
//! with the mouse (or use , . Home End, or the scroll wheel over it) to move
//! through the recorded cycles; the machine pauses and its state is restored to
//! the scrubbed cycle, and a small pop-up above the bar's position marker shows
//! the current cycle. Press P to resume from there — re-executing overwrites
//! the previously recorded future. Browsing the history resets the heatmap;
//! the last-instruction display follows the browsed cycle.
//!
//! Press M to cycle the heatmap shown next to each store word: off, the
//! execution heatmap (a colored bar shows how often each word has been
//! executed; green = cold, red = hot), or the read/write heatmap (two small
//! labeled bars per word: `r` for reads, `w` for writes).
//!
//! Press F to cycle the flow visualization shown in the right margin, like
//! the heatmap. The default mode, data, draws a pointer arrow following the
//! data flow of the last executed instruction (the line marked `>`) — STO points from the
//! accumulator to the destination word, LDN from the source word to the
//! accumulator, and the jumps from the word holding the target value to CI.
//! Magenta marks a load or store, cyan an arithmetic combination (SUB, JRP).
//! The destination is highlighted. After a jump, CI holds the target value
//! rather than the executed line, so the marker and the arrow follow the
//! executed instruction, not CI.
//!
//! The second mode, control, draws the static graph of the non-sequential
//! edges between the store lines — JMP/JRP jumps in yellow,
//! a taken CMP skip in white — with backward edges (loops) highlighted in
//! bold red and STP lines marked with a red `■`. Fall-through to the next
//! line is left implicit. Only lines reachable from the entry line (line 1)
//! get edges, so data words do not spawn spurious ones. Edges that would
//! need more lanes than available are labeled `⇢NN` on their source line.
//! The graph is computed from the current store contents, so it follows live
//! edits and history scrubbing. The third mode hides the visualization.
//!
//! Mnemonics are syntax-highlighted by category, reusing the arrows' palette
//! so the two stay coherent: magenta for a data movement (LDN, STO, JMP),
//! cyan for an arithmetic combination (SUB, SUB2, JRP), white for CMP (test)
//! and red for STP (halt). The chrome (top bar, shortcut bar, timeline) keeps
//! a single blue; the help pop-up documents the colors.
//!
//! When the machine is paused, the pointer column shows the execution
//! footprint of the current run: `>` marks the last executed line, `»` the
//! next line to execute (where P or F10 resumes), and `•` every other line
//! executed at least once. Like the heatmap, the footprint is reset when
//! browsing the history.

mod popups;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::{Frame, Terminal};

use crate::ssem::history::History;
use crate::ssem::simulator::{Breakpoint, LastInstruction, Simulator};
use crate::ssem::{Opcode, Store};
use popups::{BreakpointPopup, EditorPopup, HelpPopup};

/// Bit representations cycled through with the D key: (on_char, off_char).
const BIT_REPRESENTATIONS: [(char, char); 4] = [
    ('#', '.'),             // High contrast ASCII
    ('\u{2212}', '\u{b7}'), // Fancy characters (− and ·)
    ('_', '.'),             // ASCII compatible
    ('1', '0'),             // Binary digits
];

/// The keymap: (key, legend, help description).
/// An empty legend means the key does not appear in the bottom bar (e.g. arrow keys).
const KEYMAP: &[(&str, &str, &str)] = &[
    ("Q", "QUIT", "Quit the simulator"),
    ("P", "RUN/STOP", "Run the program or pause it"),
    ("F10", "STEP", "Execute one instruction"),
    (",", "BACK", "Step back one cycle in the execution history"),
    (
        ".",
        "FWD",
        "Step forward one cycle in the execution history",
    ),
    ("HOME/END", "", "Jump to the oldest / newest recorded cycle"),
    ("\u{2191}/\u{2193}", "", "Scroll the store view"),
    ("I", "SPEED UP", "Increase the execution speed"),
    ("K", "SPEED DOWN", "Decrease the execution speed"),
    ("D", "DISPLAY", "Cycle the bit representations"),
    (
        "E",
        "EDIT",
        "Edit a register or store word (or double-click a line)",
    ),
    (
        "M",
        "HEATMAP",
        "Cycle the heatmap: off, execution frequency, reads/writes",
    ),
    (
        "F",
        "FLOW",
        "Cycle the flow view: data flow, control flow, off",
    ),
    ("B", "BREAKPT", "Open the breakpoint pop-up"),
    ("H", "HELP", "Toggle this help pop-up"),
    (
        "MOUSE",
        "",
        "Drag the timeline bar to scrub through the history",
    ),
];

/// Minimum terminal width to display the store in full.
const MIN_WIDTH: u16 = 59;

/// Minimum terminal height (top bar + content + timeline + bottom bar).
const MIN_HEIGHT: u16 = 13;

/// Total height of the fixed bars: top bar + timeline + bottom bar.
const CHROME_HEIGHT: u16 = 3;

/// Maximum width of the timeline bar (in cells).
const MAX_TIMELINE_BAR_WIDTH: usize = 100;

/// Column and width of the timeline bar within the timeline area.
fn timeline_bar_geometry(area: Rect, start: u64, head: u64) -> (u16, u16) {
    const PREFIX: &str = " HISTORY [";
    let suffix_len = format!("] {start}..{head}").len();
    let width = (area.width as usize)
        .saturating_sub(PREFIX.len() + suffix_len)
        .min(MAX_TIMELINE_BAR_WIDTH);
    (area.x + PREFIX.len() as u16, width as u16)
}

/// Offset (in cells) of the current-position marker within the timeline bar:
/// the current cycle's position between the oldest retained cycle and the
/// tip. Returns None when there is no bar or no retained history.
fn timeline_marker_offset(
    bar_width: u16,
    start: u64,
    head: u64,
    cycle_count: u64,
) -> Option<usize> {
    if bar_width == 0 || head < start {
        return None;
    }
    let pos = cycle_count.clamp(start, head);
    let span = head - start;
    let offset = ((pos - start) * (u64::from(bar_width) - 1))
        .checked_div(span)
        .unwrap_or(0) as usize;
    Some(offset.min(usize::from(bar_width) - 1))
}

/// The syntax-highlight color of an instruction mnemonic, by category.
///
/// The palette matches the pointer arrows (see `App::current_arrow`), so a
/// mnemonic's color tells the same story as the arrow its instruction draws:
/// magenta for a data movement (LDN, STO, JMP), cyan for an arithmetic
/// combination (SUB, SUB2, JRP). CMP only tests the accumulator (white) and
/// STP halts the machine (red). NUM never reaches the display
/// (`Store::decode_word` always yields a real opcode); it is mapped anyway
/// to keep the match exhaustive.
fn opcode_color(opcode: Opcode) -> Color {
    match opcode {
        Opcode::JMP | Opcode::LDN | Opcode::STO => Color::Magenta,
        Opcode::JRP | Opcode::SUB | Opcode::SUB2 => Color::Cyan,
        Opcode::CMP => Color::White,
        Opcode::STP => Color::Red,
        Opcode::NUM => Color::DarkGray,
    }
}

/// Total number of lines in the main body content:
/// 1 blank + CI + A + 1 blank + 32 store words.
const CONTENT_HEIGHT: usize = 36;

/// How often the displayed speed is refreshed.
const SPEED_DISPLAY_PERIOD: Duration = Duration::from_millis(500);

/// Smoothing factor (in seconds) for the speed display.
const SPEED_SMOOTHING: f64 = 0.5;

/// Frame rate target for the UI thread.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Target wall-clock duration for each batch of cycles in the machine thread.
/// Short enough (~1 ms) that the UI thread can acquire the lock between batches.
const BATCH_DURATION: f64 = 0.001;

/// Maximum number of cycles per batch, capping how long the machine thread
/// can hold the lock even at very high target speeds.
const MAX_BATCH_SIZE: u64 = 10_000;

/// How long the machine thread sleeps when the simulation is stopped,
/// to avoid busy-waiting.
const IDLE_SLEEP: Duration = Duration::from_millis(10);

/// Maximum time between two left-clicks for them to count as a double-click.
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(500);

/// Width of the execution heatmap bar (in cells) shown next to each store word.
const HEATMAP_BAR_WIDTH: usize = 6;

/// Width of each bar (reads, writes) in the read/write heatmap.
const RW_BAR_WIDTH: usize = 3;

/// Extra width required by the execution heatmap (bar + surrounding spaces).
const EXEC_EXTRA_WIDTH: u16 = (HEATMAP_BAR_WIDTH + 2) as u16;

/// Extra width required by the read/write heatmap:
/// leading space + "r" + bar + space + "w" + bar + trailing space.
const RW_EXTRA_WIDTH: u16 = (2 + RW_BAR_WIDTH + 2 + RW_BAR_WIDTH + 1) as u16;

/// Which heatmap (if any) is shown next to each store word in the store view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeatmapMode {
    /// No heatmap.
    Off,
    /// Execution frequency of each store word, as one bar per word.
    Exec,
    /// Per-word store reads vs writes, as two labeled bars per word.
    Rw,
}

impl HeatmapMode {
    /// The next mode when the heatmap key is pressed: off → exec → r/w → off.
    fn cycle(self) -> Self {
        match self {
            HeatmapMode::Off => HeatmapMode::Exec,
            HeatmapMode::Exec => HeatmapMode::Rw,
            HeatmapMode::Rw => HeatmapMode::Off,
        }
    }

    /// Extra screen width (in cells) the mode adds next to each store word.
    fn extra_width(self) -> u16 {
        match self {
            HeatmapMode::Off => 0,
            HeatmapMode::Exec => EXEC_EXTRA_WIDTH,
            HeatmapMode::Rw => RW_EXTRA_WIDTH,
        }
    }

    /// The label shown in the top bar, or None when the heatmap is off.
    fn label(self) -> Option<&'static str> {
        match self {
            HeatmapMode::Off => None,
            HeatmapMode::Exec => Some("EXEC"),
            HeatmapMode::Rw => Some("R/W"),
        }
    }
}

/// Color ramps (low, mid, high) for the heatmap bars.
const EXEC_RAMP: [Color; 3] = [Color::Green, Color::Yellow, Color::Red];
const READ_RAMP: [Color; 3] = [Color::Blue, Color::Cyan, Color::LightCyan];
const WRITE_RAMP: [Color; 3] = [Color::Yellow, Color::Red, Color::LightRed];

/// Which flow visualization (if any) is shown next to the store view,
/// cycled with the A key like the heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowMode {
    /// The data flow of the last executed instruction, drawn as a pointer arrow.
    Data,
    /// The control flow between the store lines, drawn as a static graph.
    Control,
    /// No flow visualization.
    Off,
}

impl FlowMode {
    /// The next mode when the key is pressed: data → control → off → data.
    fn cycle(self) -> Self {
        match self {
            FlowMode::Data => FlowMode::Control,
            FlowMode::Control => FlowMode::Off,
            FlowMode::Off => FlowMode::Data,
        }
    }

    /// The label shown in the top bar, or None when the visualization is off.
    fn label(self) -> Option<&'static str> {
        match self {
            FlowMode::Data => Some("DATA"),
            FlowMode::Control => Some("CONTROL"),
            FlowMode::Off => None,
        }
    }
}

/// Extra width required when pointer arrows are shown
/// (separator + horizontal segment + arrowhead + gutter column).
const ARROW_EXTRA_WIDTH: u16 = 4;

/// Maximum number of edge lanes drawn side by side in the graph gutter.
const MAX_EDGE_LANES: usize = 4;

/// Columns in the graph gutter: a separator, then per lane an arrowhead
/// column, a dash column, and the corner/vertical column.
const GUTTER_COLS: usize = 1 + 3 * MAX_EDGE_LANES;

/// Width of the overflow label ("⇢31") drawn after the lanes for an edge
/// that finds no free lane.
const OVERFLOW_LABEL_WIDTH: usize = 3;

/// Extra screen width required by the control-flow graph view.
const GRAPH_EXTRA_WIDTH: u16 = (GUTTER_COLS + OVERFLOW_LABEL_WIDTH) as u16;

/// Content row of the CI register in the main body
/// (0 blank, 1 CI, 2 A, 3 blank, 4..=35 store words 0..=31).
const CI_ROW: usize = 1;

/// Content row of the accumulator in the main body.
const A_ROW: usize = 2;

/// Content row of store word 0; word `i` is at content row `STORE_ROW_OFFSET + i`.
const STORE_ROW_OFFSET: usize = 4;

/// Width of a store line's text: pointer column (1) + bits (32) + suffix (25),
/// excluding the optional heatmap bar.
const STORE_TEXT_WIDTH: usize = 58;

/// Width of the CI and accumulator lines' text: bits with leading space (33)
/// + label (16), excluding the optional heatmap spacer.
const REGISTER_TEXT_WIDTH: usize = 49;

/// A snapshot of the simulator state, taken by the UI thread under a brief lock.
///
/// Drawing uses this snapshot instead of the live simulator so that the
/// (potentially slow) terminal I/O in `terminal.draw()` never holds the lock
/// and never blocks the machine thread.
struct Snapshot {
    ci: i32,
    a: i32,
    store: [i32; 32],
    cycle_count: u64,
    speed: u32,
    stop_flag: bool,
    last_instruction: Option<LastInstruction>,
    breakpoints: Vec<Breakpoint>,
    exec_counts: [u64; 32],
    read_counts: [u64; 32],
    write_counts: [u64; 32],
    history_capacity: u64,
    history_start: u64,
    history_head: u64,
    history_len: u64,
}

impl Snapshot {
    fn from_sim(sim: &Simulator) -> Self {
        let (history_capacity, history_start, history_head, history_len) = match &sim.history {
            Some(h) => (h.capacity(), h.start(), h.head(), h.len()),
            None => (0, 0, 0, 0),
        };
        Snapshot {
            ci: sim.ci,
            a: sim.a,
            store: sim.store.words,
            cycle_count: sim.cycle_count,
            speed: sim.speed,
            stop_flag: sim.stop_flag,
            last_instruction: sim.last_instruction,
            breakpoints: sim.breakpoints.clone(),
            exec_counts: sim.exec_counts,
            read_counts: sim.read_counts,
            write_counts: sim.write_counts,
            history_capacity,
            history_start,
            history_head,
            history_len,
        }
    }
}

/// A pointer arrow: where it starts and ends (in content rows of the main
/// body) and its color.
struct PointerArrow {
    source: usize,
    target: usize,
    color: Color,
}

/// The kind of a control-flow edge, which decides its color in the graph view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeKind {
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
struct CfgEdge {
    source: usize,
    target: usize,
    kind: EdgeKind,
}

/// The static control-flow edges of the program held in `store`.
///
/// CI is incremented before each fetch, so the line executed after a jump is
/// the target value plus one: JMP `d` lands on `(store[d] + 1) & 31`, and a
/// JRP `d` at line `i` lands on `(i + store[d] + 1) & 31`. CMP has two
/// successors (fall-through and skip); STP has none.
fn control_flow_edges(store: &[i32; 32]) -> Vec<CfgEdge> {
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
fn reachable_edges(store: &[i32; 32]) -> (Vec<CfgEdge>, [bool; 32]) {
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
fn line_row(line: usize) -> usize {
    STORE_ROW_OFFSET + line
}

/// Assign each edge a gutter lane so that overlapping edges use different
/// lanes.
///
/// An edge occupies its lane from its source row to its target row; a lane is
/// free for a new edge when the previous one ends at or before the new one
/// starts (an arriving and a leaving edge may share a row). Edges are packed
/// earliest-start first, longest span first, so short edges fill the gaps
/// between long ones. Returns `None` for edges that find no free lane; those
/// are drawn as a source-row label instead.
fn assign_lanes(edges: &[CfgEdge]) -> Vec<Option<usize>> {
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
        if let Some(lane) = lane_end.iter().position(|&end_at| end_at <= start) {
            lane_end[lane] = end;
            lanes[i] = Some(lane);
        }
    }
    lanes
}

/// Merge two glyphs drawn into the same gutter cell. Corners and arrowheads
/// win over verticals, verticals over horizontals; a horizontal crossing a
/// vertical becomes a crossing, two opposite corners a straight line.
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
        ('│', '─') | ('─', '│') => '┼',
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
struct GraphGutter {
    cells: Vec<Vec<Option<(char, Style)>>>,
    overflow: Vec<Option<(usize, Style)>>,
}

impl GraphGutter {
    fn new(store: &[i32; 32]) -> Self {
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
    fn row_spans(&self, row: usize) -> Vec<Span<'static>> {
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
            spans.push(Span::styled(format!("⇢{target:02}"), style));
        }
        spans
    }
}

/// The application state for the TUI (UI thread only).
struct App {
    bit_representation: usize,
    store_scroll: u16,
    last_area: Rect,

    // Speed display: exponentially smoothed
    current_speed: f64,
    displayed_speed: u32,
    last_speed_refresh: Instant,
    last_refresh_time: Instant,
    last_cycles: u64,

    help_popup: HelpPopup,
    breakpoint_popup: BreakpointPopup,
    editor_popup: EditorPopup,

    /// The last left-click, for double-click detection: (time, column, row).
    last_click: Option<(Instant, u16, u16)>,

    /// Which heatmap is shown in the store view (cycled with the M key).
    heatmap: HeatmapMode,

    /// Which flow visualization (if any) is shown next to the store view
    /// (cycled with the F key).
    flow: FlowMode,

    /// Whether the user is currently dragging the timeline scrubber.
    scrubbing: bool,

    /// Screen area of the timeline bar, for mouse hit-testing.
    timeline_area: Rect,

    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let now = Instant::now();
        App {
            bit_representation: 0,
            store_scroll: 0,
            last_area: Rect::default(),
            current_speed: 0.0,
            displayed_speed: 0,
            last_speed_refresh: now,
            last_refresh_time: now,
            last_cycles: 0,
            help_popup: HelpPopup::new(),
            breakpoint_popup: BreakpointPopup::new(),
            editor_popup: EditorPopup::new(),
            last_click: None,
            heatmap: HeatmapMode::Off,
            flow: FlowMode::Data,
            scrubbing: false,
            timeline_area: Rect::default(),
            should_quit: false,
        }
    }

    /// Update the smoothed speed display value from the latest cycle count.
    fn update_speed_display(&mut self, is_running: bool, cycle_count: u64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refresh_time);
        self.last_refresh_time = now;

        if is_running {
            // saturating_sub: rewinding the history makes cycle_count jump backward
            let cycles_delta = cycle_count.saturating_sub(self.last_cycles);
            let measured = cycles_delta as f64 / elapsed.as_secs_f64().max(1e-9);
            let alpha = 1.0 - (-elapsed.as_secs_f64() / SPEED_SMOOTHING).exp();
            self.current_speed += alpha * (measured - self.current_speed);

            if now.duration_since(self.last_speed_refresh) >= SPEED_DISPLAY_PERIOD {
                self.displayed_speed = self.current_speed as u32;
                self.last_speed_refresh = now;
            }
        } else {
            self.current_speed = 0.0;
            self.displayed_speed = 0;
        }

        self.last_cycles = cycle_count;
    }

    /// Handle a keyboard event, mutating the simulator as needed.
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Holding a scrub key repeats it, for continuous time-travel.
        // Pop-ups keep priority over the scrub keys.
        if key.kind == KeyEventKind::Repeat {
            if !self.breakpoint_popup.is_open()
                && !self.editor_popup.is_open()
                && !self.help_popup.is_open()
            {
                self.handle_scrub_key(key.code, sim);
            }
            return;
        }

        // Only handle key press events (ignore release on platforms that report them)
        if key.kind != KeyEventKind::Press {
            return;
        }

        // While the breakpoint pop-up is open, keys go to it
        if self.breakpoint_popup.is_open() {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('b')
                || key.code == KeyCode::Char('B')
            {
                self.breakpoint_popup.close();
            } else {
                self.breakpoint_popup.handle_key(key, &mut sim.breakpoints);
            }
            return;
        }

        // While the editor pop-up is open, keys go to it
        if self.editor_popup.is_open() {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('e')
                || key.code == KeyCode::Char('E')
            {
                self.editor_popup.close();
            } else {
                self.editor_popup.handle_key(key, sim);
            }
            return;
        }

        // While the help pop-up is open, only H or Esc closes it
        if self.help_popup.is_open() {
            if key.code == KeyCode::Char('h')
                || key.code == KeyCode::Char('H')
                || key.code == KeyCode::Esc
            {
                self.help_popup.close();
            }
            return;
        }

        match key.code {
            KeyCode::Char(c) => match c.to_ascii_lowercase() {
                'q' => self.should_quit = true,
                'p' => sim.stop_flag = !sim.stop_flag,
                'i' => Self::speed_up(sim),
                'k' => Self::speed_down(sim),
                'd' => {
                    self.bit_representation =
                        (self.bit_representation + 1) % BIT_REPRESENTATIONS.len();
                }
                'b' => self.breakpoint_popup.open(),
                'e' => self.editor_popup.open(),
                'm' => self.heatmap = self.heatmap.cycle(),
                'f' => self.flow = self.flow.cycle(),
                'h' => self.help_popup.open(),
                ',' | '.' => self.handle_scrub_key(key.code, sim),
                _ => {}
            },
            KeyCode::F(10) => {
                // Step: stop the machine and execute one instruction
                sim.stop_flag = true;
                sim.instruction_cycle();
                sim.check_breakpoints();
            }
            KeyCode::Home | KeyCode::End => self.handle_scrub_key(key.code, sim),
            KeyCode::Up => {
                self.store_scroll = self.store_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let visible_height = self.last_area.height.saturating_sub(CHROME_HEIGHT) as usize;
                let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
                self.store_scroll = (self.store_scroll as usize + 1).min(max_scroll) as u16;
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    /// Handle a history-scrubbing key: `,` `.` Home End.
    fn handle_scrub_key(&mut self, code: KeyCode, sim: &mut Simulator) {
        match code {
            KeyCode::Char(',') => self.scrub_relative(-1, sim),
            KeyCode::Char('.') => self.scrub_relative(1, sim),
            KeyCode::Home => {
                if let Some(target) = sim.history.as_ref().map(History::start)
                    && target != sim.cycle_count
                {
                    sim.rewind_to(target);
                }
            }
            KeyCode::End => {
                if let Some(target) = sim.history.as_ref().map(History::head)
                    && target != sim.cycle_count
                {
                    sim.rewind_to(target);
                }
            }
            _ => {}
        }
    }

    /// Scrub the history by `delta` cycles (negative goes back in time).
    fn scrub_relative(&mut self, delta: i64, sim: &mut Simulator) {
        let Some(history) = &sim.history else {
            return;
        };
        if history.is_empty() {
            return;
        }
        let target = (sim.cycle_count as i64 + delta)
            .clamp(history.start() as i64, history.head() as i64) as u64;
        if target != sim.cycle_count {
            sim.rewind_to(target);
        }
    }

    /// Rewind to the cycle under the given screen column on the timeline bar.
    fn scrub_to_column(&mut self, column: u16, sim: &mut Simulator) {
        let Some(history) = &sim.history else {
            return;
        };
        if history.is_empty() {
            return;
        }
        let (start, head) = (history.start(), history.head());
        let (bar_x, bar_width) = timeline_bar_geometry(self.timeline_area, start, head);
        let cycle = if bar_width <= 1 {
            start
        } else {
            let offset = (column as i32 - bar_x as i32).clamp(0, bar_width as i32 - 1) as u64;
            start + offset * (head - start) / (bar_width as u64 - 1)
        };
        if cycle != sim.cycle_count {
            sim.rewind_to(cycle);
        }
    }

    /// Handle a mouse event: scrub the timeline, double-click to edit,
    /// scroll wheel to scroll.
    fn handle_mouse(&mut self, mouse: MouseEvent, sim: &mut Simulator) {
        // Releasing the left button ends a scrub, wherever the cursor is
        if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
            self.scrubbing = false;
            return;
        }

        // While scrubbing, drags anywhere on screen move the cursor
        // (scrub_to_column clamps the column to the bar)
        if self.scrubbing && matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left)) {
            self.scrub_to_column(mouse.column, sim);
            return;
        }

        // The timeline row: click to scrub, scroll wheel to step
        if self.timeline_area.height > 0 && mouse.row == self.timeline_area.y {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Scrubbing requires a non-empty history
                    if sim.history.as_ref().is_some_and(|h| !h.is_empty()) {
                        self.scrubbing = true;
                        self.scrub_to_column(mouse.column, sim);
                    }
                }
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                    let direction = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                        -1
                    } else {
                        1
                    };
                    let step = sim
                        .history
                        .as_ref()
                        .map(|h| (h.len() / 100).max(1))
                        .unwrap_or(0);
                    if step > 0 {
                        self.scrub_relative(direction * step as i64, sim);
                    }
                }
                _ => {}
            }
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let now = Instant::now();
                let is_double_click = self
                    .last_click
                    .map(|(time, col, row)| {
                        now.duration_since(time) < DOUBLE_CLICK_THRESHOLD
                            && col == mouse.column
                            && row == mouse.row
                    })
                    .unwrap_or(false);

                self.last_click = Some((now, mouse.column, mouse.row));

                if is_double_click {
                    self.try_open_editor_at(mouse.row);
                }
            }
            MouseEventKind::ScrollUp => {
                self.store_scroll = self.store_scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                let visible_height = self.last_area.height.saturating_sub(CHROME_HEIGHT) as usize;
                let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
                self.store_scroll = (self.store_scroll as usize + 1).min(max_scroll) as u16;
            }
            _ => {}
        }
    }

    /// Open the editor pop-up if the given screen row corresponds to CI, A, or a store word.
    ///
    /// The main body starts at row 1 (below the top bar). Content rows:
    /// 0 = blank, 1 = CI, 2 = A, 3 = blank, 4..=35 = store words 0..=31.
    /// The store view is scrolled by `store_scroll` lines.
    fn try_open_editor_at(&mut self, screen_row: u16) {
        // Ignore clicks outside the main body or when the terminal is too small
        if self.last_area.width < MIN_WIDTH || self.last_area.height < MIN_HEIGHT {
            return;
        }
        let body_top = 1u16;
        let body_height = self.last_area.height.saturating_sub(CHROME_HEIGHT);
        if screen_row < body_top || screen_row >= body_top + body_height {
            return;
        }

        let content_row =
            (screen_row as usize).saturating_sub(body_top as usize) + self.store_scroll as usize;

        match content_row {
            1 => self.editor_popup.open_for_ci(),
            2 => self.editor_popup.open_for_a(),
            row if (4..=35).contains(&row) => {
                self.editor_popup.open_for_word(row - 4);
            }
            _ => {}
        }
    }

    /// Increase the execution speed.
    fn speed_up(sim: &mut Simulator) {
        let speed = sim.speed;
        sim.speed = if speed < 10 {
            speed + 1
        } else if speed < 100 {
            speed + 10
        } else {
            (speed + 100).min(10_000_000)
        };
    }

    /// Decrease the execution speed.
    fn speed_down(sim: &mut Simulator) {
        let speed = sim.speed;
        sim.speed = if speed <= 10 {
            speed.saturating_sub(1).max(1)
        } else if speed <= 100 {
            speed - 10
        } else {
            speed - 100
        };
    }

    /// Format a 32-bit word as bits in SSEM order (least significant bit first).
    fn format_bits(&self, word: i32) -> String {
        let (on, off) = BIT_REPRESENTATIONS[self.bit_representation];
        let mut s = String::with_capacity(32);
        for i in 0..32 {
            s.push(if (word >> i) & 1 == 1 { on } else { off });
        }
        s
    }

    /// Compute one heatmap bar: `count` relative to `max`, `width` cells wide,
    /// filled proportionally and colored on the given 3-step `ramp`.
    /// A zero count (or an all-zero heatmap) is dim.
    fn heat_bar(count: u64, max: u64, width: usize, ramp: [Color; 3]) -> (String, Color) {
        if count == 0 || max == 0 {
            return ("\u{b7}".repeat(width), Color::DarkGray);
        }

        let ratio = count as f64 / max as f64;
        let filled = ((ratio * width as f64).ceil() as usize).clamp(1, width);

        let bar = format!(
            "{}{}",
            "\u{2588}".repeat(filled),
            "\u{b7}".repeat(width - filled),
        );

        let color = if ratio > 0.75 {
            ramp[2]
        } else if ratio > 0.25 {
            ramp[1]
        } else {
            ramp[0]
        };

        (bar, color)
    }

    /// The spans of the heatmap area for the store word at `index`, in the
    /// current mode. Empty when the heatmap is off.
    ///
    /// Exec mode draws one bar (how often the word has been executed);
    /// r/w mode draws two labeled bars: `r` for operand reads and `w` for
    /// writes, each scaled against the hottest word of its own category.
    fn heatmap_spans(&self, index: usize, snap: &Snapshot) -> Vec<Span<'static>> {
        match self.heatmap {
            HeatmapMode::Off => Vec::new(),
            HeatmapMode::Exec => {
                let max = snap.exec_counts.iter().max().copied().unwrap_or(0);
                let (bar, color) =
                    Self::heat_bar(snap.exec_counts[index], max, HEATMAP_BAR_WIDTH, EXEC_RAMP);
                vec![Span::styled(format!(" {bar} "), Style::default().fg(color))]
            }
            HeatmapMode::Rw => {
                let read_max = snap.read_counts.iter().max().copied().unwrap_or(0);
                let write_max = snap.write_counts.iter().max().copied().unwrap_or(0);
                let (rbar, rcolor) =
                    Self::heat_bar(snap.read_counts[index], read_max, RW_BAR_WIDTH, READ_RAMP);
                let (wbar, wcolor) = Self::heat_bar(
                    snap.write_counts[index],
                    write_max,
                    RW_BAR_WIDTH,
                    WRITE_RAMP,
                );
                vec![
                    Span::raw(" r"),
                    Span::styled(rbar, Style::default().fg(rcolor)),
                    Span::raw(" w"),
                    Span::styled(wbar, Style::default().fg(wcolor)),
                    Span::raw(" "),
                ]
            }
        }
    }

    /// The pointer arrow for the last executed instruction, following its
    /// data flow:
    ///
    /// - STO: from the accumulator to the destination word
    /// - LDN: from the source word to the accumulator
    /// - SUB/SUB2: like LDN, in a different color
    /// - JMP: from the word holding the target value to CI (a copy)
    /// - JRP: like JMP, in a different color (the value is added to CI)
    ///
    /// Every arrow is a data movement, so the color matches the operation:
    /// magenta when a value is loaded or stored, cyan when it is
    /// arithmetically combined. The arrow follows the executed instruction
    /// rather than CI: after a jump, CI holds the target value, so the word
    /// at CI is neither the last nor the next instruction to execute (CI is
    /// incremented before each fetch). Returns `None` when no instruction
    /// has executed yet, or when the instruction moves no store data (CMP
    /// and STP ignore their data field).
    fn current_arrow(snap: &Snapshot) -> Option<PointerArrow> {
        let li = snap.last_instruction?;
        let word_row = STORE_ROW_OFFSET + li.data as usize;
        let (source, target) = match li.opcode {
            Opcode::STO => (A_ROW, word_row),
            Opcode::LDN | Opcode::SUB | Opcode::SUB2 => (word_row, A_ROW),
            Opcode::JMP | Opcode::JRP => (word_row, CI_ROW),
            // CMP and STP ignore their data field
            _ => return None,
        };
        Some(PointerArrow {
            source,
            target,
            color: opcode_color(li.opcode),
        })
    }

    /// Draw the entire interface from a state snapshot.
    fn draw(&mut self, f: &mut Frame, snap: &Snapshot) {
        let area = f.area();
        self.last_area = area;

        // Clamp scroll to keep the content within the visible area
        let visible_height = area.height.saturating_sub(CHROME_HEIGHT) as usize;
        let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
        self.store_scroll = (self.store_scroll as usize).min(max_scroll) as u16;

        // Check minimum terminal size (wider when a heatmap or a flow
        // visualization is shown)
        let mut min_width = MIN_WIDTH + self.heatmap.extra_width();
        match self.flow {
            FlowMode::Control => min_width += GRAPH_EXTRA_WIDTH,
            FlowMode::Data => min_width += ARROW_EXTRA_WIDTH,
            FlowMode::Off => {}
        }
        if area.width < min_width || area.height < MIN_HEIGHT {
            let msg = format!(
                "Terminal too small: {}x{}, needs at least {}x{}",
                area.width, area.height, min_width, MIN_HEIGHT
            );
            let paragraph = Paragraph::new(msg);
            f.render_widget(paragraph, area);
            self.timeline_area = Rect::default();
            return;
        }

        // Layout: top bar (1 line), main body (flexible), timeline (1 line), bottom bar (1 line)
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        self.timeline_area = chunks[2];
        self.draw_top_bar(f, chunks[0], snap);
        self.draw_main_body(f, chunks[1], snap);
        self.draw_timeline(f, chunks[2], snap);
        self.draw_bottom_bar(f, chunks[3]);

        // Draw pop-ups on top
        if self.help_popup.is_open() {
            self.help_popup.draw(f, area);
        }
        if self.breakpoint_popup.is_open() {
            self.breakpoint_popup.draw(f, area, &snap.breakpoints);
        }
        if self.editor_popup.is_open() {
            self.editor_popup
                .draw(f, area, snap.ci, snap.a, &snap.store);
        }

        // While scrubbing the timeline, show the current cycle above the cursor
        if self.scrubbing {
            self.draw_scrub_popup(f, area, snap);
        }
    }

    /// Draw the top bar with status indicators.
    fn draw_top_bar(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let status = if self.scrubbing {
            "REWIND"
        } else if snap.stop_flag {
            "STOPPED"
        } else {
            "RUNNING"
        };

        // The chrome keeps a single color, the last instruction included:
        // the syntax highlighting lives in the store view
        let last_instr = snap
            .last_instruction
            .map(|li| li.to_string())
            .unwrap_or_else(|| "-- -- --".to_string());

        let mut text = format!(
            "[ STATUS: {status} ]  [ CYCLES: {} ]  [ {last_instr} ]  [ SPEED: {:2}/{} ips ] ",
            snap.cycle_count, self.displayed_speed, snap.speed
        );

        if !snap.breakpoints.is_empty() {
            text.push_str(&format!(" [ BREAKPOINTS: {} ] ", snap.breakpoints.len()));
        }

        if let Some(label) = self.heatmap.label() {
            text.push_str(&format!(" [ HEATMAP: {label} ] "));
        }

        if let Some(label) = self.flow.label() {
            text.push_str(&format!(" [ FLOW: {label} ] "));
        }

        let line = Line::from(Span::styled(text, Style::default().fg(Color::Blue)));
        f.render_widget(Paragraph::new(line), area);
    }

    /// Draw the main body: CI, accumulator, and store words.
    fn draw_main_body(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(CONTENT_HEIGHT);

        // Spacer to align CI/A bits with store-word bits when a heatmap is on
        let heatmap_spacer = " ".repeat(self.heatmap.extra_width() as usize);

        // Row 0: blank
        lines.push(Line::from(""));

        // In graph mode the gutter draws the static control-flow edges
        // between the store lines instead of the data-flow arrow
        let graph = (self.flow == FlowMode::Control).then(|| GraphGutter::new(&snap.store));

        // Pointer arrow: gutter cells appended after a line's text, following
        // the data flow of the last executed instruction
        let arrow = if self.flow == FlowMode::Data {
            Self::current_arrow(snap)
        } else {
            None
        };
        let arrow_target = arrow.as_ref().map(|a| a.target);

        // Gutter cells for the rows whose text spans the full store width
        // (store words, and the blank row the arrow's vertical may cross).
        // The register rows' cells (CI, accumulator) are built separately,
        // since their text is shorter and the arrow must connect back to it.
        let heatmap_extra = self.heatmap.extra_width() as usize;
        let mut gutter: Vec<Option<Span<'static>>> = vec![None; CONTENT_HEIGHT];
        if let Some(arrow) = &arrow {
            let style = Style::default().fg(arrow.color);
            let (top, bottom) = (
                arrow.source.min(arrow.target),
                arrow.source.max(arrow.target),
            );
            // The corner leaving the source line turns toward the target;
            // the corner at the target line turns toward the arrowhead
            let (src_corner, dst_corner) = if arrow.target > arrow.source {
                ('\u{256e}', '\u{256f}') // ╮ ╯
            } else {
                ('\u{256f}', '\u{256e}') // ╯ ╮
            };
            if !matches!(arrow.source, CI_ROW | A_ROW) {
                gutter[arrow.source] = Some(Span::styled(format!(" ──{src_corner}"), style));
            }
            for cell in gutter.iter_mut().take(bottom).skip(top + 1) {
                *cell = Some(Span::styled("   \u{2502}".to_string(), style));
            }
            if !matches!(arrow.target, CI_ROW | A_ROW) {
                gutter[arrow.target] = Some(Span::styled(
                    format!(" \u{25c0}\u{2500}{dst_corner}"),
                    style,
                ));
            }
        }

        // A register row's (CI, accumulator) gutter cell: a run of dashes
        // connects the arrow back to the register's text. When the arrow's
        // vertical crosses a register row that is not an endpoint (an arrow
        // to CI crosses the accumulator), that row carries the vertical.
        let register_gutter = |row: usize| -> Option<Span<'static>> {
            let arrow = arrow.as_ref()?;
            let style = Style::default().fg(arrow.color);
            // Cells between the register's text and the store gutter
            let pad = STORE_TEXT_WIDTH - REGISTER_TEXT_WIDTH;
            if arrow.source == row {
                // The arrow leaves the register (STO)
                let corner = if arrow.target > arrow.source {
                    '\u{256e}'
                } else {
                    '\u{256f}'
                };
                Some(Span::styled(
                    format!(" {}{corner}", "\u{2500}".repeat(pad + 2)),
                    style,
                ))
            } else if arrow.target == row {
                // The arrow arrives at the register (LDN/SUB to A, JMP/JRP to CI)
                let corner = if arrow.target > arrow.source {
                    '\u{256f}'
                } else {
                    '\u{256e}'
                };
                Some(Span::styled(
                    format!(" \u{25c0}{}{corner}", "\u{2500}".repeat(pad + 1)),
                    style,
                ))
            } else if arrow.source.min(arrow.target) < row && row < arrow.source.max(arrow.target) {
                // The arrow's vertical crosses this register row
                Some(Span::styled(
                    format!("{}   \u{2502}", " ".repeat(pad)),
                    style,
                ))
            } else {
                None
            }
        };

        // Row 1: CI
        let ci_bits = self.format_bits(snap.ci);
        let ci_style = if arrow_target == Some(CI_ROW) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let mut ci_spans = vec![
            Span::styled(format!("{heatmap_spacer} {ci_bits}"), ci_style),
            Span::raw(format!(" CI: {:>11}", snap.ci)),
        ];
        if let Some(cell) = &register_gutter(CI_ROW) {
            ci_spans.push(cell.clone());
        }
        lines.push(Line::from(ci_spans));

        // Row 2: Accumulator
        let a_bits = self.format_bits(snap.a);
        let a_style = if arrow_target == Some(A_ROW) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let mut a_spans = vec![
            Span::styled(format!("{heatmap_spacer} {a_bits}"), a_style),
            Span::raw(format!(" A : {:>11}", snap.a)),
        ];
        if let Some(cell) = &register_gutter(A_ROW) {
            a_spans.push(cell.clone());
        }
        lines.push(Line::from(a_spans));

        // Row 3: blank (carries the arrow's vertical segment when it crosses)
        let mut blank_spans: Vec<Span<'static>> = Vec::new();
        if let Some(g) = &graph {
            blank_spans.extend(g.row_spans(3));
        } else if let Some(cell) = &gutter[3] {
            blank_spans.push(Span::raw(" ".repeat(STORE_TEXT_WIDTH + heatmap_extra)));
            blank_spans.push(cell.clone());
        }
        lines.push(Line::from(blank_spans));

        // The line of the last executed instruction, marked `>`: after a
        // jump CI holds the target value, so the marker follows the executed
        // line rather than CI. Nothing is marked before the first cycle.
        let current_line = snap
            .last_instruction
            .map(|li| (li.address & Store::INDEX_MASK) as usize);

        // When paused, the pointer column also shows the execution footprint
        // of the current run: `»` on the next line to execute (CI is
        // incremented before each fetch, so the next fetch is CI + 1), and
        // `•` on every other line executed at least once.
        let paused = snap.stop_flag;
        let next_line = paused.then(|| ((snap.ci + 1) & Store::INDEX_MASK) as usize);

        // Rows 4-35: store words
        for (index, word) in snap.store.iter().enumerate() {
            let row = STORE_ROW_OFFSET + index;
            let is_current = current_line == Some(index);
            let is_target = arrow_target == Some(row);
            let (pointer, pointer_style) = if is_current {
                (">", Style::default().add_modifier(Modifier::BOLD))
            } else if next_line == Some(index) {
                (
                    "\u{bb}",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )
            } else if paused && snap.exec_counts[index] > 0 {
                ("\u{2022}", Style::default().fg(Color::DarkGray))
            } else {
                (" ", Style::default())
            };
            let bits = self.format_bits(*word);
            let (opcode, data) = Store::decode_word(*word);

            let bits_style = if is_current {
                Style::default().add_modifier(Modifier::BOLD)
            } else if is_target {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            };

            // Heatmap bars (no span when the heatmap is off)
            let heatmap = self.heatmap_spans(index, snap);

            let mut spans = vec![
                Span::styled(pointer.to_string(), pointer_style),
                Span::styled(bits, bits_style),
                Span::raw(format!(" {:02}: {:>11}  ", index, word)),
                Span::styled(
                    format!("{:<4}", opcode.to_string()),
                    Style::default()
                        .fg(opcode_color(opcode))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {:02}", data)),
            ];
            spans.splice(1..1, heatmap);
            if let Some(g) = &graph {
                spans.extend(g.row_spans(row));
            } else if let Some(cell) = &gutter[row] {
                spans.push(cell.clone());
            }
            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines).scroll((self.store_scroll, 0));
        f.render_widget(paragraph, area);
    }

    /// Draw the timeline bar: how much history is retained (filled bar) and
    /// where the machine currently is (marker). Dragging it scrubs the history.
    fn draw_timeline(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        if snap.history_capacity == 0 {
            let line = Line::from(Span::styled(
                " HISTORY [disabled]",
                Style::default().fg(Color::DarkGray),
            ));
            f.render_widget(Paragraph::new(line), area);
            return;
        }

        let (_, bar_width) = timeline_bar_geometry(area, snap.history_start, snap.history_head);

        // Filled portion = fraction of the capacity still retained
        let filled = (snap.history_len * u64::from(bar_width) / snap.history_capacity) as usize;

        // Marker at the current position (the tip, or the scrub position)
        let marker = timeline_marker_offset(
            bar_width,
            snap.history_start,
            snap.history_head,
            snap.cycle_count,
        );

        let blue = Style::default().fg(Color::Blue);
        let mut spans = vec![Span::styled(" HISTORY [".to_string(), blue)];
        if bar_width > 0 {
            let segment = |from: usize, to: usize| -> String {
                (from..to)
                    .map(|i| if i < filled { '\u{2588}' } else { '\u{b7}' })
                    .collect()
            };
            let marker = marker.unwrap_or(0);
            spans.push(Span::styled(segment(0, marker), blue));
            spans.push(Span::styled(
                ">".to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                segment(marker + 1, usize::from(bar_width)),
                blue,
            ));
        }
        spans.push(Span::styled(
            format!("] {}..{}", snap.history_start, snap.history_head),
            blue,
        ));

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Draw a small pop-up above the timeline cursor, showing the current cycle.
    fn draw_scrub_popup(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        if self.timeline_area.height == 0 {
            return;
        }

        // Center the pop-up on the bar's position marker, so it stays above
        // the marker even when the mouse is dragged past the bar's ends
        // (only the screen edges clamp it).
        let (bar_x, bar_width) =
            timeline_bar_geometry(self.timeline_area, snap.history_start, snap.history_head);
        let Some(offset) = timeline_marker_offset(
            bar_width,
            snap.history_start,
            snap.history_head,
            snap.cycle_count,
        ) else {
            return;
        };
        let center = bar_x + offset as u16;

        let text = format!(" cycle {} ", snap.cycle_count);
        let width = text.len() as u16;
        let x = center
            .saturating_sub(width / 2)
            .clamp(area.x, area.x + area.width.saturating_sub(width));
        let popup_area = Rect {
            x,
            y: self.timeline_area.y.saturating_sub(1),
            width,
            height: 1,
        };

        f.render_widget(Clear, popup_area);
        f.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::Black).bg(Color::Yellow)),
            popup_area,
        );
    }

    /// Draw the bottom bar with keyboard shortcuts.
    fn draw_bottom_bar(&self, f: &mut Frame, area: Rect) {
        let key_style = Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::REVERSED);
        let normal_style = Style::default();

        let mut spans: Vec<Span<'static>> = Vec::new();
        for (key, legend, _) in KEYMAP {
            if legend.is_empty() {
                continue;
            }
            let key_text = if key.chars().count() == 1 {
                format!(" {key} ")
            } else {
                (*key).to_string()
            };
            spans.push(Span::styled(key_text, key_style));
            spans.push(Span::styled(format!(" {legend}  "), normal_style));
        }

        let line = Line::from(spans);
        f.render_widget(Paragraph::new(line), area);
    }
}

/// The machine thread: runs the simulation at the target speed.
///
/// Executes cycles in time-limited batches (~1 ms), releasing the lock between
/// batches so the UI thread can acquire it. Sleeps until the batch deadline to
/// maintain the target speed. Uses absolute deadlines so that `sleep()` overshoot
/// does not accumulate over time.
fn machine_thread(sim: Arc<Mutex<Simulator>>, quit: Arc<AtomicBool>, max_cycles: u64) {
    let mut next_deadline = Instant::now();
    let mut last_speed: u32 = 0;

    loop {
        if quit.load(Ordering::Relaxed) {
            break;
        }

        // Read the control state under a brief lock
        let (should_run, speed) = {
            let s = sim.lock().unwrap();
            (!s.stop_flag, s.speed)
        };

        if !should_run {
            next_deadline = Instant::now();
            last_speed = 0;
            thread::sleep(IDLE_SLEEP);
            continue;
        }

        // Resynchronize when the speed changes
        if speed != last_speed {
            next_deadline = Instant::now();
            last_speed = speed;
        }

        // Calculate batch size: enough cycles to fill ~1 ms at the target speed
        let speed = speed.max(1);
        let period = 1.0 / speed as f64;
        let batch_size = ((BATCH_DURATION / period) as u64).clamp(1, MAX_BATCH_SIZE);
        let deadline = next_deadline + Duration::from_secs_f64(batch_size as f64 * period);

        // Execute the batch (lock held for the duration of the batch)
        {
            let mut s = sim.lock().unwrap();
            for _ in 0..batch_size {
                if s.stop_flag {
                    break;
                }
                if s.cycle_count >= max_cycles {
                    s.stop_flag = true;
                    break;
                }
                s.instruction_cycle();
                s.check_breakpoints();
            }
        }

        // Sleep until the absolute deadline, so sleep() overshoot does not
        // accumulate. If execution fell behind schedule (e.g. speed set beyond
        // what the machine can deliver), resynchronize instead of catching up
        // in a burst.
        let now = Instant::now();
        if deadline > now {
            thread::sleep(deadline - now);
            next_deadline = deadline;
        } else {
            next_deadline = now + Duration::from_secs_f64(batch_size as f64 * period);
        }
    }
}

/// Run the interactive TUI.
///
/// Takes ownership of the simulator and returns it when the user quits,
/// so the caller can print the final state.
pub fn run(simulator: Simulator, max_cycles: u32) -> io::Result<Simulator> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run_app(&mut terminal, simulator, max_cycles);

    // Always restore the terminal, even on error
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    simulator: Simulator,
    max_cycles: u32,
) -> io::Result<Simulator> {
    // Shared state between the machine and UI threads
    let sim = Arc::new(Mutex::new(simulator));
    let quit = Arc::new(AtomicBool::new(false));

    // Start in the stopped state; the user presses P to run
    sim.lock().unwrap().stop_flag = true;

    // Spawn the machine thread
    let machine_sim = Arc::clone(&sim);
    let machine_quit = Arc::clone(&quit);
    let machine_handle = thread::spawn(move || {
        machine_thread(machine_sim, machine_quit, max_cycles as u64);
    });

    // UI thread (main thread)
    let mut app = App::new();

    loop {
        // Handle input (non-blocking with a short timeout)
        if event::poll(FRAME_INTERVAL)? {
            match event::read()? {
                Event::Key(key) => {
                    let mut s = sim.lock().unwrap();
                    app.handle_key(key, &mut s);
                }
                Event::Mouse(mouse) => {
                    let mut s = sim.lock().unwrap();
                    app.handle_mouse(mouse, &mut s);
                }
                _ => {}
            }
        }

        // Take a snapshot under a brief lock, then draw without holding it.
        // This keeps the lock available for the machine thread during terminal I/O.
        let snapshot = {
            let s = sim.lock().unwrap();
            app.update_speed_display(!s.stop_flag, s.cycle_count);
            Snapshot::from_sim(&s)
        };

        terminal.draw(|f| app.draw(f, &snapshot))?;

        if app.should_quit {
            quit.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Signal the machine thread to exit and wait for it
    quit.store(true, Ordering::Relaxed);
    machine_handle.join().unwrap();

    // Recover the simulator from the Arc<Mutex<>>
    let sim = Arc::try_unwrap(sim)
        .ok()
        .expect("machine thread should have released its Arc reference")
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    Ok(sim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

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

    #[test]
    fn timeline_column_maps_to_cycle() {
        let mut sim = looping_sim(10);
        let mut app = App::new();
        app.timeline_area = Rect::new(0, 10, 80, 1);

        let (bar_x, bar_width) = timeline_bar_geometry(app.timeline_area, 0, 10);
        assert_eq!(10, bar_x);
        assert!(bar_width > 1);

        // Far right of the bar: the tip
        app.scrub_to_column(bar_x + bar_width - 1, &mut sim);
        assert_eq!(10, sim.cycle_count);

        // Left edge: the oldest retained cycle
        app.scrub_to_column(bar_x, &mut sim);
        assert_eq!(0, sim.cycle_count);

        // Middle
        app.scrub_to_column(bar_x + bar_width / 2, &mut sim);
        assert_eq!(5, sim.cycle_count);

        // Columns past the bar's ends clamp to the retained range,
        // so the marker (and the pop-up centered on it) stays on the bar
        app.scrub_to_column(bar_x + bar_width + 50, &mut sim);
        assert_eq!(10, sim.cycle_count);
        app.scrub_to_column(0, &mut sim);
        assert_eq!(0, sim.cycle_count);
        app.scrub_to_column(bar_x.saturating_sub(50), &mut sim);
        assert_eq!(0, sim.cycle_count);
    }

    #[test]
    fn scrub_relative_clamps_to_history_range() {
        let mut sim = looping_sim(10);
        let mut app = App::new();

        app.scrub_relative(-1000, &mut sim);
        assert_eq!(0, sim.cycle_count);
        assert!(sim.stop_flag, "scrubbing pauses the machine");

        app.scrub_relative(1000, &mut sim);
        assert_eq!(10, sim.cycle_count);

        // At the tip, stepping forward is a no-op and does not pause
        sim.stop_flag = false;
        app.scrub_relative(1, &mut sim);
        assert_eq!(10, sim.cycle_count);
        assert!(!sim.stop_flag);
    }

    #[test]
    fn scrub_keys_step_through_history() {
        let mut sim = looping_sim(10);
        let mut app = App::new();

        app.handle_scrub_key(KeyCode::Char(','), &mut sim);
        assert_eq!(9, sim.cycle_count);
        app.handle_scrub_key(KeyCode::Char('.'), &mut sim);
        assert_eq!(10, sim.cycle_count);
        app.handle_scrub_key(KeyCode::Home, &mut sim);
        assert_eq!(0, sim.cycle_count);
        app.handle_scrub_key(KeyCode::End, &mut sim);
        assert_eq!(10, sim.cycle_count);
    }

    #[test]
    fn scrub_without_history_is_noop() {
        let mut sim = Simulator::new();
        sim.run(10);
        let mut app = App::new();

        app.handle_scrub_key(KeyCode::Char(','), &mut sim);
        assert_eq!(10, sim.cycle_count);
        assert!(!sim.stop_flag);
    }

    #[test]
    fn scrub_popup_centers_on_the_bar_marker() {
        // On an 80-column terminal the bar spans x=10, width 63
        let timeline = Rect::new(0, 18, 80, 1);
        let (bar_x, bar_width) = timeline_bar_geometry(timeline, 0, 99);
        assert_eq!(10, bar_x);
        assert_eq!(63, bar_width);

        for cycle_count in [0, 42, 99] {
            let mut app = App::new();
            app.scrubbing = true;
            let snap = snapshot_with_history(cycle_count);
            let buffer = render(&mut app, &snap, 80, 20);

            // The timeline row is 18 on a 20-row terminal; the pop-up sits on row 17
            let row = row_text(&buffer, 17);
            let text = format!(" cycle {cycle_count} ");
            let pos = row
                .find(&text)
                .unwrap_or_else(|| panic!("pop-up missing at cycle {cycle_count}, row: {row:?}"));
            let center = pos as u16 + text.len() as u16 / 2;
            let marker =
                bar_x + timeline_marker_offset(bar_width, 0, 99, cycle_count).unwrap() as u16;
            assert_eq!(
                marker, center,
                "pop-up should center on the marker at cycle {cycle_count}"
            );
        }
    }

    #[test]
    fn scrub_popup_hidden_when_not_scrubbing() {
        let mut app = App::new();
        let snap = snapshot_with_history(42);
        let buffer = render(&mut app, &snap, 80, 20);

        let all: String = (0..buffer.area.height)
            .map(|y| row_text(&buffer, y))
            .collect();
        assert!(!all.contains(" cycle 42 "), "no pop-up when not scrubbing");
    }

    #[test]
    fn scrub_popup_stays_on_screen_at_the_bar_ends() {
        // Narrowest supported terminal (wider with the default pointer
        // arrows), pop-up at both ends of the bar
        for cycle_count in [0, 99] {
            let mut app = App::new();
            app.scrubbing = true;
            let snap = snapshot_with_history(cycle_count);
            let buffer = render(&mut app, &snap, MIN_WIDTH + ARROW_EXTRA_WIDTH, 20);

            let row = row_text(&buffer, 17);
            let text = format!(" cycle {cycle_count} ");
            let pos = row
                .find(&text)
                .unwrap_or_else(|| panic!("pop-up missing at cycle {cycle_count}, row: {row:?}"));
            assert!(
                pos + text.len() <= buffer.area.width as usize,
                "pop-up must stay on screen at cycle {cycle_count}"
            );
        }
    }

    #[test]
    fn pointer_arrow_sto_points_from_accumulator_to_word() {
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
    fn pointer_arrow_ldn_points_from_word_to_accumulator() {
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
    fn pointer_arrow_sub_uses_a_different_color() {
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
    fn pointer_arrow_jmp_points_from_word_to_ci() {
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
    fn pointer_arrow_jrp_adds_the_word_to_ci_in_cyan() {
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
    fn pointer_arrow_aligns_with_the_heatmap() {
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
    fn pointer_arrow_jmp_zero_points_from_word_zero_to_ci() {
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
    fn pointer_arrow_and_marker_ignore_ci() {
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
    fn marker_and_arrow_follow_the_jump_not_its_target() {
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
    fn no_marker_or_arrow_before_the_first_cycle() {
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
    fn marker_and_arrow_survive_history_scrubbing() {
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
    fn pointer_arrow_hidden_for_operandless_instructions() {
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
    fn pointer_arrow_hidden_when_toggled_off() {
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
    fn pointer_arrows_shown_by_default() {
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
    fn flow_mode_cycles_with_the_f_key() {
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
    fn pointer_arrows_require_extra_width() {
        let snap = pointer_snapshot(0, [0; 32]);
        let mut app = App::new();
        app.flow = FlowMode::Data;

        // Too narrow: the "too small" message is shown instead of the store
        let buffer = render(&mut app, &snap, MIN_WIDTH, 40);
        assert!(row_text(&buffer, 0).contains("Terminal too small"));

        // Wide enough: the store and the arrow are drawn
        let buffer = render(&mut app, &snap, MIN_WIDTH + ARROW_EXTRA_WIDTH, 40);
        assert!(!row_text(&buffer, 0).contains("Terminal too small"));
        let row = row_text(&buffer, 5);
        assert_eq!('╯', row.chars().nth(GUTTER_COLUMN).unwrap(), "row: {row:?}");
    }

    /// Screen column where a store line's mnemonic starts:
    /// pointer (1) + bits (32) + " NN: " + value (11) + "  " = 51.
    const MNEMONIC_COLUMN: usize = 51;

    #[test]
    fn mnemonics_are_colored_by_category() {
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
    fn top_bar_stays_monochrome_blue() {
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
    fn executed_footprint_markers_shown_when_paused() {
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
    fn executed_footprint_markers_hidden_while_running() {
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
    fn next_marker_points_at_the_first_fetch_on_a_fresh_machine() {
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
    fn help_popup_documents_the_color_code_and_paused_markers() {
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

    #[test]
    fn heatmap_key_cycles_off_exec_rw() {
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
    fn top_bar_shows_the_heatmap_mode() {
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
    fn exec_heatmap_draws_one_bar_per_word() {
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
    const RW_R_COL: usize = 2;
    const RW_RBAR_COL: usize = 3;
    const RW_W_COL: usize = 7;
    const RW_WBAR_COL: usize = 8;

    #[test]
    fn rw_heatmap_draws_labeled_read_and_write_bars() {
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
    fn rw_heatmap_needs_its_extra_width() {
        let snap = snapshot_with_history(0);
        let mut app = App::new();
        app.heatmap = HeatmapMode::Rw;
        app.flow = FlowMode::Off;

        let needed = MIN_WIDTH + HeatmapMode::Rw.extra_width();
        let buffer = render(&mut app, &snap, needed - 1, 40);
        assert!(row_text(&buffer, 0).contains("Terminal too small"));

        let buffer = render(&mut app, &snap, needed, 40);
        assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    }

    #[test]
    fn rw_heatmap_shifts_the_pointer_arrows() {
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

    /// A store where every line is STP (no control-flow edges), except the
    /// given (line, word) entries.
    fn graph_store(entries: &[(usize, &str)]) -> [i32; 32] {
        let mut store = [Store::parse_word("STP").unwrap(); 32];
        for &(line, text) in entries {
            store[line] = Store::parse_word(text).unwrap();
        }
        store
    }

    /// Screen column of the graph gutter's separator (heatmap off).
    const G_SEP: usize = STORE_TEXT_WIDTH;
    /// Screen column of lane `l`'s arrowhead / vertical in the graph gutter.
    fn g_head(l: usize) -> usize {
        STORE_TEXT_WIDTH + 1 + 3 * l
    }
    fn g_vert(l: usize) -> usize {
        STORE_TEXT_WIDTH + 3 + 3 * l
    }

    #[test]
    fn cfg_edges_are_empty_when_every_line_is_stp() {
        let store = graph_store(&[]);
        assert!(control_flow_edges(&store).is_empty());
    }

    #[test]
    fn cfg_edges_fall_through_to_the_next_line() {
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
    fn cfg_edges_jmp_lands_on_operand_value_plus_one() {
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
    fn cfg_edges_jrp_adds_the_operand_to_the_executed_line() {
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
    fn cfg_edges_cmp_has_a_fall_through_and_a_skip_edge() {
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
    fn cfg_edges_wrap_around_the_store_end() {
        let store = graph_store(&[(31, "LDN 0")]);
        let edges = control_flow_edges(&store);
        assert!(edges.contains(&CfgEdge {
            source: 31,
            target: 0,
            kind: EdgeKind::FallThrough,
        }));
    }

    #[test]
    fn cfg_edges_support_self_loops() {
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
    fn lanes_are_shared_when_edges_do_not_overlap() {
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
        assert_eq!(assign_lanes(&edges), vec![Some(0), Some(0)]);
    }

    #[test]
    fn overlapping_edges_get_different_lanes() {
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
    fn more_overlapping_edges_than_lanes_overflow_to_none() {
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
    fn reachable_edges_prune_unreachable_lines() {
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
    fn render_graph(store: [i32; 32]) -> (Buffer, App) {
        let snap = Snapshot {
            store,
            ..snapshot_with_history(0)
        };
        let mut app = App::new();
        app.flow = FlowMode::Control;
        let buffer = render(&mut app, &snap, 80, 40);
        (buffer, app)
    }

    #[test]
    fn graph_view_hides_fall_through_but_marks_stops() {
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
    fn graph_view_draws_a_forward_jump() {
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
    fn graph_view_draws_only_the_cmp_skip() {
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
    fn graph_view_highlights_backward_jumps_as_loops() {
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
    fn graph_view_draws_self_loops_as_a_circular_arrow() {
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
    fn graph_gutter_renders_the_overflow_label_after_the_lanes() {
        let mut gutter = GraphGutter {
            cells: vec![vec![None; GUTTER_COLS]; CONTENT_HEIGHT],
            overflow: vec![None; CONTENT_HEIGHT],
        };
        gutter.overflow[line_row(4)] = Some((31, Style::default().fg(Color::Red)));
        let spans = gutter.row_spans(line_row(4));
        let text: String = spans.iter().flat_map(|span| span.content.chars()).collect();
        assert_eq!(GUTTER_COLS + OVERFLOW_LABEL_WIDTH, text.chars().count());
        assert!(text.ends_with("⇢31"));
    }

    #[test]
    fn graph_view_hides_the_data_flow_arrow() {
        let store = graph_store(&[(2, "STO 24")]);
        // Last instruction: STO 24 at line 2, which would draw a magenta
        // arrow from the accumulator to word 24
        let snap = pointer_snapshot(2, store);

        let mut app = App::new();
        app.flow = FlowMode::Control;
        let buffer = render(&mut app, &snap, 80, 40);

        // The gutter columns carry graph edges only: no magenta arrow
        for y in 0..buffer.area.height {
            for x in STORE_TEXT_WIDTH..(STORE_TEXT_WIDTH + GRAPH_EXTRA_WIDTH as usize) {
                assert_ne!(Color::Magenta, buffer[(x as u16, y)].fg, "cell ({x}, {y})");
            }
        }
    }

    #[test]
    fn graph_view_requires_extra_width() {
        let snap = snapshot_with_history(0);
        let mut app = App::new();
        app.flow = FlowMode::Control;

        let buffer = render(&mut app, &snap, MIN_WIDTH, 40);
        assert!(row_text(&buffer, 0).contains("Terminal too small"));

        let buffer = render(&mut app, &snap, MIN_WIDTH + GRAPH_EXTRA_WIDTH, 40);
        assert!(!row_text(&buffer, 0).contains("Terminal too small"));
    }

    #[test]
    fn flow_mode_is_shown_in_the_top_bar() {
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
    fn help_popup_documents_the_graph_view() {
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
}
