//! Cold light theme

use super::{Palette, rgb};

pub const LIGHT: Palette = Palette {
    // Base background, default text color, and chrome
    fg: rgb(0x1F2328),
    bg: rgb(0xF4F4F4),
    chrome: rgb(0x3B6FD4),
    status_running: rgb(0x2E7D32),
    status_stopped: rgb(0x3B6FD4),
    status_rewind: rgb(0xB45309),
    key_hint: rgb(0x3B6FD4),
    selection: rgb(0x3B6FD4),

    // Main body
    value: rgb(0x2E7D32),
    highlight: rgb(0xB45309),
    next_marker: rgb(0x3B6FD4),
    dim: rgb(0x8A8F98),
    zebra_bg: rgb(0xECECEC),

    // Opcodes (shared with the pointer arrows)
    op_movement: rgb(0xA626A4),
    op_arithmetic: rgb(0x0E7C86),
    op_test: rgb(0x1F2328),
    op_halt: rgb(0xD1242F),
    op_num: rgb(0x8A8F98),

    // Heatmap ramps (low, mid, high)
    exec_ramp: [rgb(0x2E7D32), rgb(0xC77400), rgb(0xD1242F)],
    read_ramp: [rgb(0x3B6FD4), rgb(0x0E7C86), rgb(0x0891B2)],
    write_ramp: [rgb(0xC77400), rgb(0xD1242F), rgb(0xB91C1C)],

    // Control-flow graph edges
    edge_loop: rgb(0xD1242F),
    edge_fallthrough: rgb(0x8A8F98),
    edge_jump: rgb(0xB45309),
    edge_skip: rgb(0x1F2328),

    // Timeline and scrub pop-up
    timeline_marker: rgb(0xB45309),
    scrub_popup_fg: rgb(0x1F2328),
    scrub_popup_bg: rgb(0xF2C94C),

    // Pop-ups
    breakpoint_border: rgb(0xD1242F),
    editor_border: rgb(0xC77400),
    save_border: rgb(0x0969DA),
    preview: rgb(0x2E7D32),
};
