//! The Dracula theme: dark background with vivid accents, after the
//! official Dracula palette.
//!
//! The base colors are Dracula's background/foreground, the accents its
//! purple/cyan/orange/green/yellow/pink/red, and dimmed text a darker
//! shade of its comment gray. The zebra stripe is the background blended a
//! faint step toward the chrome color. Adjust them here to change the
//! theme's appearance.

use super::{Palette, blend, rgb};

/// The theme's display name, shown in the top bar.
pub const LABEL: &str = "Dracula";

pub const PALETTE: Palette = Palette {
    // Base background, default text color, and chrome (the comment gray,
    // so the cyan stays free for memory accesses)
    fg: rgb(0xF8F8F2),
    bg: rgb(0x282A36),
    chrome: rgb(0x6272A4),
    status_running: rgb(0x50FA7B),
    status_stopped: rgb(0xF8F8F2),
    status_rewind: rgb(0xF1FA8C),
    status_fg: rgb(0xF8F8F2),
    hover: rgb(0x44475A),
    hover_fg: rgb(0xF8F8F2),
    menu_bg: rgb(0x282A36),
    menu_block_bg: rgb(0x282A36),
    status_bg: rgb(0x21222C),
    status_block_bg: rgb(0x21222C),
    key_hint: rgb(0x6272A4),
    selection: rgb(0x6272A4),

    // Main body
    value: rgb(0x50FA7B),
    highlight: rgb(0xF1FA8C),
    next_marker: rgb(0x6272A4),
    dim: rgb(0x4A5578),
    zebra_bg: blend(rgb(0x282A36), rgb(0x6272A4), 1, 16),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in purple, memory accesses in cyan, arithmetic in orange; red is
    // reserved for STP and breakpoints
    op_jump: rgb(0xBD93F9),
    op_memory: rgb(0x8BE9FD),
    op_arithmetic: rgb(0xFFB86C),
    op_test: rgb(0xF8F8F2),
    op_halt: rgb(0xFF5555),
    op_num: rgb(0x4A5578),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump purple family, reads in the memory cyan
    // family, writes in the destination yellow family
    exec_ramp: [rgb(0x8F6FC9), rgb(0xB79BF0), rgb(0xDCCBFF)],
    read_ramp: [rgb(0x66B3D1), rgb(0x9FEBFC), rgb(0xD3F6FE)],
    write_ramp: [rgb(0xC8C46E), rgb(0xEAE58B), rgb(0xF8F6C6)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x4A5578),
    edge_jump: rgb(0xBD93F9),
    edge_skip: rgb(0xF8F8F2),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on yellow; the bar's fill/track are bg blended to chrome at the
    // fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0x282A36), rgb(0x6272A4), 1, 2),
    timeline_track: blend(rgb(0x282A36), rgb(0x6272A4), 1, 4),
    timeline_marker: rgb(0xF1FA8C),
    scrub_popup_fg: rgb(0x282A36),
    scrub_popup_bg: rgb(0xF1FA8C),

    // Pop-ups (red for the breakpoint "stop", yellow for the editor, pink
    // for save; the preview is the value green)
    breakpoint_border: rgb(0xFF5555),
    editor_border: rgb(0xF1FA8C),
    save_border: rgb(0xFF79C6),
    preview: rgb(0x50FA7B),
};
