//! The Rose Pine theme: dark with soft pastels, after the official
//! Rosé Pine (main) palette.
//!
//! The base colors are Rose Pine's base/text, the accents its iris/foam/
//! pine/gold/rose/love, and chrome its subtext gray (dimmed text its muted
//! gray). The zebra stripe is Rose Pine's surface, just above the base.
//! Adjust them here to change the theme's appearance.

use super::{Palette, blend, rgb};

pub const ROSE_PINE: Palette = Palette {
    // Base background, default text color, and chrome (the subtext gray,
    // so the foam stays free for memory accesses)
    fg: rgb(0xE0DEF4),
    bg: rgb(0x191724),
    chrome: rgb(0x908CAA),
    status_running: rgb(0x9CCFD8),
    status_stopped: rgb(0xE0DEF4),
    status_rewind: rgb(0xF6C177),
    status_fg: rgb(0xE0DEF4),
    hover: rgb(0x26233A),
    hover_fg: rgb(0xE0DEF4),
    menu_bg: rgb(0x191724),
    menu_block_bg: rgb(0x191724),
    status_bg: rgb(0x21202E),
    status_block_bg: rgb(0x21202E),
    key_hint: rgb(0x908CAA),
    selection: rgb(0x908CAA),

    // Main body
    value: rgb(0xEBBCBA),
    highlight: rgb(0xF6C177),
    next_marker: rgb(0x908CAA),
    dim: rgb(0x6E6A86),
    zebra_bg: rgb(0x1F1D2E),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in iris, memory accesses in foam, arithmetic in pine; love red is
    // reserved for STP and breakpoints
    op_jump: rgb(0xC4A7E7),
    op_memory: rgb(0x9CCFD8),
    op_arithmetic: rgb(0x31748F),
    op_test: rgb(0xE0DEF4),
    op_halt: rgb(0xEB6F92),
    op_num: rgb(0x6E6A86),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump iris family, reads in the memory foam
    // family, writes in the destination gold family
    exec_ramp: [rgb(0x8A76B8), rgb(0xAE93D9), rgb(0xD3C2F2)],
    read_ramp: [rgb(0x6FA8B5), rgb(0x9CCFD8), rgb(0xD2F0F5)],
    write_ramp: [rgb(0xC4A05F), rgb(0xE0BC85), rgb(0xF8DCAE)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x6E6A86),
    edge_jump: rgb(0xC4A7E7),
    edge_skip: rgb(0xE0DEF4),

    // Timeline and scrub pop-up (the pop-up text is the background color,
    // dark on gold; the bar's fill/track are bg blended to chrome at the
    // fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0x191724), rgb(0x908CAA), 1, 2),
    timeline_track: blend(rgb(0x191724), rgb(0x908CAA), 1, 4),
    timeline_marker: rgb(0xF6C177),
    scrub_popup_fg: rgb(0x191724),
    scrub_popup_bg: rgb(0xF6C177),

    // Pop-ups (love for the breakpoint "stop", gold for the editor, pine
    // for save; the preview is the value rose)
    breakpoint_border: rgb(0xEB6F92),
    editor_border: rgb(0xF6C177),
    save_border: rgb(0x31748F),
    preview: rgb(0xEBBCBA),
};
