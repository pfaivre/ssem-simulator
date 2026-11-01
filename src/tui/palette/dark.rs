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

    // Opcodes (shared with the pointer arrows):
    // colors.normal magenta/cyan/white/red, colors.bright black
    op_movement: rgb(0xBD88AC),
    op_arithmetic: rgb(0x8DB7B5),
    op_test: rgb(0xEFF1F5),
    op_halt: rgb(0xCE5564),
    op_num: rgb(0x6C707D),

    // Heatmap ramps (low, mid, high): normal green/yellow/red,
    // normal blue/cyan, normal yellow/red; highs are the bright variants
    exec_ramp: [rgb(0x98C38C), rgb(0xEFCD86), rgb(0xCE5564)],
    read_ramp: [rgb(0x8CA1B4), rgb(0x8DB7B5), rgb(0xA0BBBA)],
    write_ramp: [rgb(0xEFCD86), rgb(0xCE5564), rgb(0xCF6773)],

    // Control-flow graph edges (colors.normal red/yellow/white, bright black)
    edge_loop: rgb(0xCE5564),
    edge_fallthrough: rgb(0x6C707D),
    edge_jump: rgb(0xEFCD86),
    edge_skip: rgb(0xEFF1F5),

    // Timeline and scrub pop-up (normal yellow; the pop-up text is the
    // background color, dark on yellow)
    timeline_marker: rgb(0xEFCD86),
    scrub_popup_fg: rgb(0x2B303B),
    scrub_popup_bg: rgb(0xEFCD86),

    // Pop-ups (colors.normal red/yellow/green)
    breakpoint_border: rgb(0xCE5564),
    editor_border: rgb(0xEFCD86),
    save_border: rgb(0x6C9EF8),
    preview: rgb(0x98C38C),
};
