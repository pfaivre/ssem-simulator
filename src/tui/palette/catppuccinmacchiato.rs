//! The Catppuccin Macchiato theme: a dark pastel theme, after the official
//! Catppuccin Macchiato palette.
//!
//! The base colors are Macchiato's base/text, the accents its lavender,
//! blue, green, yellow, mauve, teal, sky, and red, and dimmed text its
//! overlay gray.
//! The zebra stripe is the background blended a faint step toward the chrome
//! color. Adjust them here to change the theme's appearance.

use super::{Palette, blend, rgb};

/// The theme's display name, shown in the top bar.
pub const LABEL: &str = "Catppuccin Macchiato";

pub const PALETTE: Palette = Palette {
    // Base background, default text color, and chrome (lavender, so the
    // blue stays free for arithmetic, distinct from the memory teal)
    fg: rgb(0xCAD3F5),
    bg: rgb(0x24273A),
    chrome: rgb(0xB7BDF8),
    status_running: rgb(0xA6DA95),
    status_stopped: rgb(0xCAD3F5),
    status_rewind: rgb(0xEED49F),
    status_fg: rgb(0xCAD3F5),
    hover: rgb(0xB7BDF8),
    hover_fg: rgb(0x11111B),
    menu_bg: rgb(0x181926),
    menu_block_bg: rgb(0x181926),
    status_bg: rgb(0x181926),
    status_block_bg: rgb(0x181926),
    key_hint: rgb(0xB7BDF8),
    selection: rgb(0xB7BDF8),

    // Main body
    value: rgb(0xA6DA95),
    highlight: rgb(0xEED49F),
    next_marker: rgb(0xB7BDF8),
    dim: rgb(0x6E738D),
    zebra_bg: blend(rgb(0x24273A), rgb(0xB7BDF8), 1, 16),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in mauve, memory accesses in teal, arithmetic in blue (clearly
    // apart from the teal); red is reserved for STP and breakpoints
    op_jump: rgb(0xC6A0F6),
    op_memory: rgb(0x8BD5CA),
    op_arithmetic: rgb(0x8AADF4),
    op_test: rgb(0xCAD3F5),
    op_halt: rgb(0xED8796),
    op_num: rgb(0x6E738D),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump mauve family, reads in the memory teal
    // family, writes in the destination yellow family
    exec_ramp: [rgb(0x9678C8), rgb(0xB794E8), rgb(0xE3D4F9)],
    read_ramp: [rgb(0x63A89D), rgb(0x7FC6B8), rgb(0xC2EAE2)],
    write_ramp: [rgb(0xC4B078), rgb(0xDCC887), rgb(0xF6ECC0)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x6E738D),
    edge_jump: rgb(0xC6A0F6),
    edge_skip: rgb(0xCAD3F5),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on yellow; the bar's fill/track are bg blended to chrome at the
    // fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0x24273A), rgb(0xB7BDF8), 1, 2),
    timeline_track: blend(rgb(0x24273A), rgb(0xB7BDF8), 1, 4),
    timeline_marker: rgb(0xEED49F),
    scrub_popup_fg: rgb(0x24273A),
    scrub_popup_bg: rgb(0xEED49F),

    // Pop-ups (red for the breakpoint "stop", yellow for the editor,
    // sky for save; the preview is the value green)
    breakpoint_border: rgb(0xED8796),
    editor_border: rgb(0xEED49F),
    save_border: rgb(0x91D7E3),
    preview: rgb(0xA6DA95),
};
