//! The Catppuccin Latte theme: a light pastel theme, after the official
//! Catppuccin Latte palette (the light sibling of Macchiato).
//!
//! The base colors are Latte's base/text, the accents its blue, mauve,
//! teal, green, yellow, red, and sky, and dimmed text its overlay gray.
//! The zebra stripe is the background blended a faint step toward the chrome
//! color. Adjust them here to change the theme's appearance.

use super::{Palette, blend, rgb};

/// The theme's display name, shown in the top bar.
pub const LABEL: &str = "Catppuccin Latte";

pub const PALETTE: Palette = Palette {
    // Base background, default text color, and chrome (subtext gray, so the
    // blue stays free for arithmetic, distinct from the memory teal)
    fg: rgb(0x4C4F69),
    bg: rgb(0xEFF1F5),
    chrome: rgb(0x6C6F85),
    status_running: rgb(0x40A02B),
    status_stopped: rgb(0x4C4F69),
    status_rewind: rgb(0xDF8E1D),
    status_fg: rgb(0x4C4F69),
    hover: rgb(0xBCC0CC),
    hover_fg: rgb(0x4C4F69),
    menu_bg: rgb(0xEFF1F5),
    menu_block_bg: rgb(0xEFF1F5),
    status_bg: rgb(0xDCE0E8),
    status_block_bg: rgb(0xDCE0E8),
    key_hint: rgb(0x6C6F85),
    selection: rgb(0x6C6F85),

    // Main body (the zebra is bg blended a faint step toward chrome, so
    // slightly darker on this light background)
    value: rgb(0x40A02B),
    highlight: rgb(0xDF8E1D),
    next_marker: rgb(0x6C6F85),
    dim: rgb(0x9CA0B0),
    zebra_bg: blend(rgb(0xEFF1F5), rgb(0x6C6F85), 1, 16),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in mauve, memory accesses in teal, arithmetic in blue; red is
    // reserved for STP and breakpoints
    op_jump: rgb(0x8839EF),
    op_memory: rgb(0x179299),
    op_arithmetic: rgb(0x1E66F5),
    op_test: rgb(0x4C4F69),
    op_halt: rgb(0xD20F39),
    op_num: rgb(0x9CA0B0),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump mauve family, reads in the memory teal
    // family, writes in the destination yellow family. On a light background
    // the ramps run the other way round from the dark themes: the darkest
    // shade marks the highest count
    exec_ramp: [rgb(0xE4D2F7), rgb(0xC39BEB), rgb(0x8839EF)],
    read_ramp: [rgb(0xC2E4E1), rgb(0x6FB3AE), rgb(0x179299)],
    write_ramp: [rgb(0xF6E3B8), rgb(0xE7BC66), rgb(0xDF8E1D)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x9CA0B0),
    edge_jump: rgb(0x8839EF),
    edge_skip: rgb(0x4C4F69),

    // Timeline and scrub pop-up (the bar's fill/track are bg blended to
    // chrome at the fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0xEFF1F5), rgb(0x6C6F85), 1, 2),
    timeline_track: blend(rgb(0xEFF1F5), rgb(0x6C6F85), 1, 4),
    timeline_marker: rgb(0xDF8E1D),
    scrub_popup_fg: rgb(0x4C4F69),
    scrub_popup_bg: rgb(0xDF8E1D),

    // Pop-ups (red for the breakpoint "stop", yellow for the editor, sky
    // for save; the preview is the value green)
    breakpoint_border: rgb(0xD20F39),
    editor_border: rgb(0xDF8E1D),
    save_border: rgb(0x04A5E5),
    preview: rgb(0x40A02B),
};
