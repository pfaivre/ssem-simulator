//! Display theme: mnemonic colors, heatmap and flow modes, keymap, bit glyphs.

use ratatui::style::Color;

use crate::ssem::Opcode;

/// Bit representations cycled through with the D key: (on_char, off_char).
pub(super) const BIT_REPRESENTATIONS: [(char, char); 4] = [
    ('#', '.'),             // High contrast ASCII
    ('\u{2212}', '\u{b7}'), // Fancy characters (− and ·)
    ('_', '.'),             // ASCII compatible
    ('1', '0'),             // Binary digits
];

/// The keymap: (key, legend, help description).
/// An empty legend means the key does not appear in the bottom bar (e.g. arrow keys).
pub(super) const KEYMAP: &[(&str, &str, &str)] = &[
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
    (
        "C",
        "COMMENTS",
        "Toggle the source-comment column next to each store word",
    ),
    ("B", "BREAKPT", "Open the breakpoint pop-up"),
    ("H", "HELP", "Toggle this help pop-up"),
    (
        "MOUSE",
        "",
        "Drag the timeline bar to scrub through the history",
    ),
];

/// The syntax-highlight color of an instruction mnemonic, by category.
///
/// The palette matches the pointer arrows (see `App::current_arrow`), so a
/// mnemonic's color tells the same story as the arrow its instruction draws:
/// magenta for a data movement (LDN, STO, JMP), cyan for an arithmetic
/// combination (SUB, SUB2, JRP). CMP only tests the accumulator (white) and
/// STP halts the machine (red). NUM never reaches the display
/// (`Store::decode_word` always yields a real opcode); it is mapped anyway
/// to keep the match exhaustive.
pub(super) fn opcode_color(opcode: Opcode) -> Color {
    match opcode {
        Opcode::JMP | Opcode::LDN | Opcode::STO => Color::Magenta,
        Opcode::JRP | Opcode::SUB | Opcode::SUB2 => Color::Cyan,
        Opcode::CMP => Color::White,
        Opcode::STP => Color::Red,
        Opcode::NUM => Color::DarkGray,
    }
}

/// Width of the execution heatmap bar (in cells) shown next to each store word.
pub(super) const HEATMAP_BAR_WIDTH: usize = 6;

/// Width of each bar (reads, writes) in the read/write heatmap.
pub(super) const RW_BAR_WIDTH: usize = 3;

/// Extra width required by the execution heatmap (bar + surrounding spaces).
const EXEC_EXTRA_WIDTH: u16 = (HEATMAP_BAR_WIDTH + 2) as u16;

/// Extra width required by the read/write heatmap:
/// leading space + "r" + bar + space + "w" + bar + trailing space.
const RW_EXTRA_WIDTH: u16 = (2 + RW_BAR_WIDTH + 2 + RW_BAR_WIDTH + 1) as u16;

/// Which heatmap (if any) is shown next to each store word in the store view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HeatmapMode {
    /// No heatmap.
    Off,
    /// Execution frequency of each store word, as one bar per word.
    Exec,
    /// Per-word store reads vs writes, as two labeled bars per word.
    Rw,
}

impl HeatmapMode {
    /// The next mode when the heatmap key is pressed: off → exec → r/w → off.
    pub(super) fn cycle(self) -> Self {
        match self {
            HeatmapMode::Off => HeatmapMode::Exec,
            HeatmapMode::Exec => HeatmapMode::Rw,
            HeatmapMode::Rw => HeatmapMode::Off,
        }
    }

    /// Extra screen width (in cells) the mode adds next to each store word.
    pub(super) fn extra_width(self) -> u16 {
        match self {
            HeatmapMode::Off => 0,
            HeatmapMode::Exec => EXEC_EXTRA_WIDTH,
            HeatmapMode::Rw => RW_EXTRA_WIDTH,
        }
    }

    /// The label shown in the top bar, or None when the heatmap is off.
    pub(super) fn label(self) -> Option<&'static str> {
        match self {
            HeatmapMode::Off => None,
            HeatmapMode::Exec => Some("EXEC"),
            HeatmapMode::Rw => Some("R/W"),
        }
    }
}

/// Color ramps (low, mid, high) for the heatmap bars.
pub(super) const EXEC_RAMP: [Color; 3] = [Color::Green, Color::Yellow, Color::Red];
pub(super) const READ_RAMP: [Color; 3] = [Color::Blue, Color::Cyan, Color::LightCyan];
pub(super) const WRITE_RAMP: [Color; 3] = [Color::Yellow, Color::Red, Color::LightRed];

/// Which flow visualization (if any) is shown next to the store view,
/// cycled with the A key like the heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FlowMode {
    /// The control flow between the store lines, drawn as a static graph.
    Control,
    /// The data flow of the last executed instruction, drawn as a pointer arrow.
    Data,
    /// No flow visualization.
    Off,
}

impl FlowMode {
    /// The next mode when the key is pressed: data → control → off → data.
    pub(super) fn cycle(self) -> Self {
        match self {
            FlowMode::Control => FlowMode::Data,
            FlowMode::Data => FlowMode::Off,
            FlowMode::Off => FlowMode::Control,
        }
    }

    /// The label shown in the top bar, or None when the visualization is off.
    pub(super) fn label(self) -> Option<&'static str> {
        match self {
            FlowMode::Control => Some("CONTROL"),
            FlowMode::Data => Some("DATA"),
            FlowMode::Off => None,
        }
    }
}
