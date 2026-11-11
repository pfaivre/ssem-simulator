//! The terminal theme: defer to the terminal's own color scheme.

use ratatui::style::Color;

use super::{Palette, blend};

pub const TERMINAL: Palette = Palette {
    // Base: the terminal's own default foreground/background
    fg: Color::Reset,
    bg: Color::Reset,
    chrome: Color::Blue,
    status_running: Color::Black,
    status_stopped: Color::Black,
    status_rewind: Color::Yellow,
    status_fg: Color::Black,
    hover: Color::DarkGray,
    hover_fg: Color::White,
    menu_bg: Color::Reset,
    menu_block_bg: Color::Reset,
    status_bg: Color::LightCyan,
    status_block_bg: Color::LightCyan,
    key_hint: Color::Blue,
    selection: Color::Blue,

    // Main body
    value: Color::Green,
    highlight: Color::Yellow,
    next_marker: Color::Blue,
    dim: Color::DarkGray,
    zebra_bg: Color::Reset,

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in magenta, memory accesses in cyan, arithmetic in light green; red
    // is reserved for STP and breakpoints
    op_jump: Color::Magenta,
    op_memory: Color::Cyan,
    op_arithmetic: Color::LightGreen,
    op_test: Color::White,
    op_halt: Color::Red,
    op_num: Color::DarkGray,

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump magenta family, reads in the memory cyan
    // family, writes in the destination yellow family
    exec_ramp: [Color::Magenta, Color::LightMagenta, Color::White],
    read_ramp: [Color::Cyan, Color::LightCyan, Color::White],
    write_ramp: [Color::Yellow, Color::LightYellow, Color::White],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: Color::DarkGray,
    edge_jump: Color::Magenta,
    edge_skip: Color::White,

    // Timeline and scrub pop-up (named colors have no fractional blends of
    // the default background and blue: the fill collapses to solid blue,
    // the track is the theme's dim gray)
    timeline_fill: blend(Color::Reset, Color::Blue, 1, 2),
    timeline_track: Color::DarkGray,
    timeline_marker: Color::Yellow,
    scrub_popup_fg: Color::Black,
    scrub_popup_bg: Color::Yellow,

    // Pop-ups
    breakpoint_border: Color::Red,
    editor_border: Color::Yellow,
    save_border: Color::LightBlue,
    preview: Color::Green,
};
