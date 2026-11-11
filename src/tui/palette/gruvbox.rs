//! The Gruvbox theme: dark with retro warm colors, after the official
//! Gruvbox Dark (medium contrast) palette.
//!
//! The base colors are Gruvbox's bg0/fg1, the accents its purple/aqua/blue/
//! green/yellow/red, and chrome its light gray (dimmed text its neutral
//! gray). The zebra stripe is the background blended a faint step toward the
//! chrome color. Adjust them here to change the theme's appearance.

use super::{Palette, blend, rgb};

/// The theme's display name, shown in the top bar.
pub const LABEL: &str = "Gruvbox";

pub const PALETTE: Palette = Palette {
    // Base background, default text color, and chrome (the light gray, so
    // the aqua stays free for memory accesses)
    fg: rgb(0xEBDBB2),
    bg: rgb(0x282828),
    chrome: rgb(0xA89984),
    status_running: rgb(0xB8BB26),
    status_stopped: rgb(0xEBDBB2),
    status_rewind: rgb(0xFABD2F),
    status_fg: rgb(0xEBDBB2),
    hover: rgb(0x3C3836),
    hover_fg: rgb(0xEBDBB2),
    menu_bg: rgb(0x282828),
    menu_block_bg: rgb(0x282828),
    status_bg: rgb(0x1D2021),
    status_block_bg: rgb(0x1D2021),
    key_hint: rgb(0xA89984),
    selection: rgb(0xA89984),

    // Main body
    value: rgb(0xB8BB26),
    highlight: rgb(0xFABD2F),
    next_marker: rgb(0xA89984),
    dim: rgb(0x928374),
    zebra_bg: blend(rgb(0x282828), rgb(0xA89984), 1, 16),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in purple, memory accesses in aqua, arithmetic in blue; red is
    // reserved for STP and breakpoints
    op_jump: rgb(0xD3869B),
    op_memory: rgb(0x8EC07C),
    op_arithmetic: rgb(0x83A598),
    op_test: rgb(0xEBDBB2),
    op_halt: rgb(0xFB4934),
    op_num: rgb(0x928374),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump purple family, reads in the memory aqua
    // family, writes in the destination yellow family
    exec_ramp: [rgb(0xA97B8B), rgb(0xC49AAE), rgb(0xE0BBD0)],
    read_ramp: [rgb(0x6F9468), rgb(0x9DBE8D), rgb(0xC9E2B8)],
    write_ramp: [rgb(0xC9B458), rgb(0xE8D47E), rgb(0xF7EBB0)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x928374),
    edge_jump: rgb(0xD3869B),
    edge_skip: rgb(0xEBDBB2),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on yellow; the bar's fill/track are bg blended to chrome at the
    // fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0x282828), rgb(0xA89984), 1, 2),
    timeline_track: blend(rgb(0x282828), rgb(0xA89984), 1, 4),
    timeline_marker: rgb(0xFABD2F),
    scrub_popup_fg: rgb(0x282828),
    scrub_popup_bg: rgb(0xFABD2F),

    // Pop-ups (red for the breakpoint "stop", yellow for the editor, blue
    // for save; the preview is the value green)
    breakpoint_border: rgb(0xFB4934),
    editor_border: rgb(0xFABD2F),
    save_border: rgb(0x83A598),
    preview: rgb(0xB8BB26),
};
