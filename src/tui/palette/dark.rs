//! Base 16 Ocean inspired theme

use super::{Palette, rgb};

pub const DARK: Palette = Palette {
    // Base background, default text color, and chrome
    // (colors.primary foreground/background, colors.normal blue)
    fg: rgb(0xEFF1F5),
    bg: rgb(0x2B303B),
    chrome: rgb(0x8CA1B4),
    status_running: rgb(0x98C38C),
    status_stopped: rgb(0x8CA1B4),
    status_rewind: rgb(0xEFCD86),
    key_hint: rgb(0x8CA1B4),
    selection: rgb(0x8CA1B4),

    // Main body (colors.normal green/yellow/blue; zebra is a subtle tint
    // just above the background)
    value: rgb(0x98C38C),
    highlight: rgb(0xEFCD86),
    next_marker: rgb(0x8CA1B4),
    dim: rgb(0x6C707D),
    zebra_bg: rgb(0x2f343f),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in magenta, memory accesses in cyan, arithmetic in blue; red is
    // reserved for STP and breakpoints
    op_jump: rgb(0xBD88AC),
    op_memory: rgb(0x8DB7B5),
    op_arithmetic: rgb(0x6C9EF8),
    op_test: rgb(0xEFF1F5),
    op_halt: rgb(0xCE5564),
    op_num: rgb(0x6C707D),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump magenta family, reads in the memory cyan
    // family, writes in the destination yellow family (all distinct shades
    // of the corresponding flat op/highlight colors)
    exec_ramp: [rgb(0x8A6284), rgb(0xB07BA0), rgb(0xDDB8DC)],
    read_ramp: [rgb(0x628E8C), rgb(0x7FA8A6), rgb(0xC2E2E0)],
    write_ramp: [rgb(0xB39A5F), rgb(0xD4B96F), rgb(0xF0E2B4)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x6C707D),
    edge_jump: rgb(0xBD88AC),
    edge_skip: rgb(0xEFF1F5),

    // Timeline and scrub pop-up (normal yellow; the pop-up text is the
    // background color, dark on yellow)
    timeline_marker: rgb(0xEFCD86),
    scrub_popup_fg: rgb(0x2B303B),
    scrub_popup_bg: rgb(0xEFCD86),

    // Pop-ups (red for the breakpoint "stop", yellow for the editor, a
    // cool tint for save; the preview is the value green)
    breakpoint_border: rgb(0xCE5564),
    editor_border: rgb(0xEFCD86),
    save_border: rgb(0xA0BBBA),
    preview: rgb(0x98C38C),
};
