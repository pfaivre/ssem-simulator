//! The Monokai theme: dark background with bright accents, after the
//! classic Monokai color scheme.
//!
//! The base colors are Monokai's background/foreground, the accents its
//! iconic pink/green/yellow/cyan/purple/orange, and dimmed text its comment
//! gray. The zebra stripe is a subtle tint just above the background (a bit
//! below Monokai's own line-highlight `#3E3D32`). Adjust them here to change
//! the theme's appearance.

use super::{Palette, rgb};

pub const MONOKAI: Palette = Palette {
    // Base background, default text color, and chrome
    fg: rgb(0xF8F8F2),
    bg: rgb(0x272822),
    chrome: rgb(0x66D9EF),
    status_running: rgb(0xA6E22E),
    status_stopped: rgb(0x66D9EF),
    status_rewind: rgb(0xE6DB74),
    key_hint: rgb(0x66D9EF),
    selection: rgb(0x66D9EF),

    // Main body
    value: rgb(0xA6E22E),
    highlight: rgb(0xE6DB74),
    next_marker: rgb(0x66D9EF),
    dim: rgb(0x75715E),
    zebra_bg: rgb(0x2c2b26),

    // Opcodes (shared with the pointer arrows)
    op_movement: rgb(0xAE81FF),
    op_arithmetic: rgb(0x66D9EF),
    op_test: rgb(0xF8F8F2),
    op_halt: rgb(0xF92672),
    op_num: rgb(0x75715E),

    // Heatmap ramps (low, mid, high)
    exec_ramp: [rgb(0xA6E22E), rgb(0xE6DB74), rgb(0xF92672)],
    read_ramp: [rgb(0x66D9EF), rgb(0xA6E22E), rgb(0xE6DB74)],
    write_ramp: [rgb(0xE6DB74), rgb(0xFD971F), rgb(0xF92672)],

    // Control-flow graph edges
    edge_loop: rgb(0xF92672),
    edge_fallthrough: rgb(0x75715E),
    edge_jump: rgb(0xE6DB74),
    edge_skip: rgb(0xF8F8F2),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on yellow)
    timeline_marker: rgb(0xE6DB74),
    scrub_popup_fg: rgb(0x272822),
    scrub_popup_bg: rgb(0xE6DB74),

    // Pop-ups
    breakpoint_border: rgb(0xF92672),
    editor_border: rgb(0xE6DB74),
    preview: rgb(0xA6E22E),
};
