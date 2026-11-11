//! The Gruvbox Light theme: light with retro warm colors, after the
//! official Gruvbox Light (medium contrast) palette.
//!
//! The base colors are Gruvbox's bg0/fg1, the accents its darker light-mode
//! accent set (the ones readable on a light background), and chrome its
//! fg4 gray (dimmed text its neutral gray). The zebra stripe is the
//! background blended a faint step toward the chrome color. Adjust them
//! here to change the theme's appearance.

use super::{Palette, blend, rgb};

/// The theme's display name, shown in the top bar.
pub const LABEL: &str = "Gruvbox Light";

pub const PALETTE: Palette = Palette {
    // Base background, default text color, and chrome
    fg: rgb(0x3C3836),
    bg: rgb(0xFBF1C7),
    chrome: rgb(0x7C6F64),
    status_running: rgb(0x79740E),
    status_stopped: rgb(0x282828),
    status_rewind: rgb(0xB57614),
    status_fg: rgb(0x282828),
    hover: rgb(0xEBDBB2),
    hover_fg: rgb(0x3C3836),
    menu_bg: rgb(0xFBF1C7),
    menu_block_bg: rgb(0xFBF1C7),
    status_bg: rgb(0xD5C4A1),
    status_block_bg: rgb(0xD5C4A1),
    key_hint: rgb(0x7C6F64),
    selection: rgb(0x7C6F64),

    // Main body (the zebra is bg blended a faint step toward chrome, so
    // slightly darker on this light background)
    value: rgb(0x79740E),
    highlight: rgb(0xAF3A03),
    next_marker: rgb(0x7C6F64),
    dim: rgb(0x928374),
    zebra_bg: blend(rgb(0xFBF1C7), rgb(0x7C6F64), 1, 16),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in purple, memory accesses in aqua, arithmetic in blue; red is
    // reserved for STP and breakpoints
    op_jump: rgb(0x8F3F71),
    op_memory: rgb(0x427B58),
    op_arithmetic: rgb(0x076678),
    op_test: rgb(0x3C3836),
    op_halt: rgb(0x9D0006),
    op_num: rgb(0x928374),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump purple family, reads in the memory aqua
    // family, writes in the destination orange family. On a light background
    // the ramps run the other way round from the dark themes: the darkest
    // shade marks the highest count
    exec_ramp: [rgb(0xE8D3E0), rgb(0xC08BB0), rgb(0x8F3F71)],
    read_ramp: [rgb(0xCDE0D2), rgb(0x6FA98B), rgb(0x427B58)],
    write_ramp: [rgb(0xF5D9C8), rgb(0xD98A54), rgb(0xAF3A03)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x928374),
    edge_jump: rgb(0x8F3F71),
    edge_skip: rgb(0x3C3836),

    // Timeline and scrub pop-up (the bar's fill/track are bg blended to
    // chrome at the fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0xFBF1C7), rgb(0x7C6F64), 1, 2),
    timeline_track: blend(rgb(0xFBF1C7), rgb(0x7C6F64), 1, 4),
    timeline_marker: rgb(0xB57614),
    scrub_popup_fg: rgb(0x3C3836),
    scrub_popup_bg: rgb(0xD79921),

    // Pop-ups (red for the breakpoint "stop", orange for the editor, blue
    // for save; the preview is the value green)
    breakpoint_border: rgb(0x9D0006),
    editor_border: rgb(0xAF3A03),
    save_border: rgb(0x076678),
    preview: rgb(0x79740E),
};
