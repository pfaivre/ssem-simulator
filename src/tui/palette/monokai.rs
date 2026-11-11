//! The Monokai theme: dark background with bright accents, after the
//! classic Monokai color scheme.
//!
//! The base colors are Monokai's background/foreground, the accents its
//! iconic pink/green/yellow/cyan/purple/orange, and dimmed text its comment
//! gray. The zebra stripe is a subtle tint just above the background (a bit
//! below Monokai's own line-highlight `#3E3D32`). Adjust them here to change
//! the theme's appearance.

use super::{Palette, blend, rgb};

pub const MONOKAI: Palette = Palette {
    // Base background, default text color, and chrome (a warm gray, so the
    // signature cyan stays free for memory accesses)
    fg: rgb(0xF8F8F2),
    bg: rgb(0x272822),
    chrome: rgb(0x9B978B),
    status_running: rgb(0xA6E22E),
    status_stopped: rgb(0xF8F8F2),
    status_rewind: rgb(0xE6DB74),
    status_fg: rgb(0xF8F8F2),
    hover: rgb(0x3E3D32),
    hover_fg: rgb(0xF8F8F2),
    menu_bg: rgb(0x272822),
    menu_block_bg: rgb(0x272822),
    status_bg: rgb(0x1D1E19),
    status_block_bg: rgb(0x1D1E19),
    key_hint: rgb(0x9B978B),
    selection: rgb(0x9B978B),

    // Main body
    value: rgb(0xA6E22E),
    highlight: rgb(0xE6DB74),
    next_marker: rgb(0x9B978B),
    dim: rgb(0x75715E),
    zebra_bg: rgb(0x2c2b26),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in purple, memory accesses in cyan, arithmetic in orange; the pink
    // red is reserved for STP and breakpoints
    op_jump: rgb(0xAE81FF),
    op_memory: rgb(0x66D9EF),
    op_arithmetic: rgb(0xFD971F),
    op_test: rgb(0xF8F8F2),
    op_halt: rgb(0xF92672),
    op_num: rgb(0x75715E),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump purple family, reads in the memory cyan
    // family, writes in the destination yellow family
    exec_ramp: [rgb(0x8F6FD6), rgb(0xC4A5F2), rgb(0xE8DCFF)],
    read_ramp: [rgb(0x4FA8BE), rgb(0x8FD8EC), rgb(0xC9F0F8)],
    write_ramp: [rgb(0xB3A94F), rgb(0xD6CC7E), rgb(0xF2EFC0)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x75715E),
    edge_jump: rgb(0xAE81FF),
    edge_skip: rgb(0xF8F8F2),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on yellow; the bar's fill/track are bg blended to chrome at the
    // fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0x272822), rgb(0x9B978B), 1, 2),
    timeline_track: blend(rgb(0x272822), rgb(0x9B978B), 1, 4),
    timeline_marker: rgb(0xE6DB74),
    scrub_popup_fg: rgb(0x272822),
    scrub_popup_bg: rgb(0xE6DB74),

    // Pop-ups (pink for the breakpoint "stop", yellow for the editor, a
    // light cyan tint for save; the preview is the value green)
    breakpoint_border: rgb(0xF92672),
    editor_border: rgb(0xE6DB74),
    save_border: rgb(0xA6E8F5),
    preview: rgb(0xA6E22E),
};
