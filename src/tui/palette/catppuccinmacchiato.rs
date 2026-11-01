//! The Catppuccin Macchiato theme: a dark pastel theme, after the official
//! Catppuccin Macchiato palette.
//!
//! The base colors are Macchiato's base/text, the accents its blue, green,
//! yellow, mauve, teal, red, and peach, and dimmed text its overlay gray.
//! The zebra stripe is a subtle tint just above the base (below Macchiato's
//! own surface highlight `#363A4F`). Adjust them here to change the theme's
//! appearance.

use super::{Palette, rgb};

pub const CATPPUCCIN_MACCHIATO: Palette = Palette {
    // Base background, default text color, and chrome
    fg: rgb(0xCAD3F5),
    bg: rgb(0x24273A),
    chrome: rgb(0x8AADF4),
    status_running: rgb(0xA6DA95),
    status_stopped: rgb(0x8AADF4),
    status_rewind: rgb(0xEED49F),
    key_hint: rgb(0x8AADF4),
    selection: rgb(0x8AADF4),

    // Main body
    value: rgb(0xA6DA95),
    highlight: rgb(0xEED49F),
    next_marker: rgb(0x8AADF4),
    dim: rgb(0x6E738D),
    zebra_bg: rgb(0x282b3e),

    // Opcodes (shared with the pointer arrows)
    op_movement: rgb(0xC6A0F6),
    op_arithmetic: rgb(0x8BD5CA),
    op_test: rgb(0xCAD3F5),
    op_halt: rgb(0xED8796),
    op_num: rgb(0x6E738D),

    // Heatmap ramps (low, mid, high)
    exec_ramp: [rgb(0xA6DA95), rgb(0xEED49F), rgb(0xED8796)],
    read_ramp: [rgb(0x8AADF4), rgb(0x8BD5CA), rgb(0x91D7E3)],
    write_ramp: [rgb(0xEED49F), rgb(0xF5A97F), rgb(0xED8796)],

    // Control-flow graph edges
    edge_loop: rgb(0xED8796),
    edge_fallthrough: rgb(0x6E738D),
    edge_jump: rgb(0xEED49F),
    edge_skip: rgb(0xCAD3F5),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on yellow)
    timeline_marker: rgb(0xEED49F),
    scrub_popup_fg: rgb(0x24273A),
    scrub_popup_bg: rgb(0xEED49F),

    // Pop-ups
    breakpoint_border: rgb(0xED8796),
    editor_border: rgb(0xEED49F),
    save_border: rgb(0xC6A0F6),
    preview: rgb(0xA6DA95),
};
