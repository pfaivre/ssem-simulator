//! Cold light theme

use super::{Palette, rgb};

pub const LIGHT: Palette = Palette {
    // Base background, default text color, and chrome
    fg: rgb(0x1F2328),
    bg: rgb(0xF4F4F4),
    chrome: rgb(0x3B6FD4),
    status_running: rgb(0x8edc8a),
    status_stopped: rgb(0xF4F4F4),
    status_rewind: rgb(0xB45309),
    status_fg: rgb(0xF4F4F4),
    hover: rgb(0xA9C2F2),
    hover_fg: rgb(0x1F3A6E),
    menu_bg: rgb(0xF4F4F4),
    menu_block_bg: rgb(0xF4F4F4),
    status_bg: rgb(0x3B6FD4),
    status_block_bg: rgb(0x3B6FD4),
    key_hint: rgb(0x3B6FD4),
    selection: rgb(0x3B6FD4),

    // Main body
    value: rgb(0x2E7D32),
    highlight: rgb(0xB45309),
    next_marker: rgb(0x3B6FD4),
    dim: rgb(0x8A8F98),
    zebra_bg: rgb(0xECECEC),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in purple, memory accesses in teal, arithmetic in blue; red is
    // reserved for STP and breakpoints
    op_jump: rgb(0xA626A4),
    op_memory: rgb(0x0E7C86),
    op_arithmetic: rgb(0x0550AE),
    op_test: rgb(0x1F2328),
    op_halt: rgb(0xD1242F),
    op_num: rgb(0x8A8F98),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump purple family, reads in the memory teal
    // family, writes in the destination amber family. On a light background
    // the ramps run the other way round from the dark themes: the darkest
    // shade marks the highest count
    exec_ramp: [rgb(0xDDB6E6), rgb(0xB07FC0), rgb(0x8F5C96)],
    read_ramp: [rgb(0x8FCCC6), rgb(0x2E8B85), rgb(0x0A565D)],
    write_ramp: [rgb(0xF2C063), rgb(0xC87F1F), rgb(0x93450A)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x8A8F98),
    edge_jump: rgb(0xA626A4),
    edge_skip: rgb(0x1F2328),

    // Timeline and scrub pop-up
    timeline_marker: rgb(0xF2C94C),
    scrub_popup_fg: rgb(0x1F2328),
    scrub_popup_bg: rgb(0xF2C94C),

    // Pop-ups
    breakpoint_border: rgb(0xD1242F),
    editor_border: rgb(0xC77400),
    save_border: rgb(0x0969DA),
    preview: rgb(0x2E7D32),
};
