//! The Rose Pine Dawn theme: light with soft pastels, after the official
//! Rosé Pine Dawn palette (the light sibling of Rosé Pine).
//!
//! The base colors are Dawn's base/text, the accents its iris/foam/pine/
//! gold/rose/love, and chrome its subtext gray (dimmed text its muted
//! gray). The zebra stripe is Dawn's highlight-low, just below the base.
//! Adjust them here to change the theme's appearance.

use super::{Palette, blend, rgb};

pub const ROSE_PINE_DAWN: Palette = Palette {
    // Base background, default text color, and chrome
    fg: rgb(0x575279),
    bg: rgb(0xFAF4ED),
    chrome: rgb(0x797593),
    status_running: rgb(0x286983),
    status_stopped: rgb(0x575279),
    status_rewind: rgb(0xEA9D34),
    status_fg: rgb(0x575279),
    hover: rgb(0xF2E9E1),
    hover_fg: rgb(0x575279),
    menu_bg: rgb(0xFAF4ED),
    menu_block_bg: rgb(0xFAF4ED),
    status_bg: rgb(0xF2E9E1),
    status_block_bg: rgb(0xF2E9E1),
    key_hint: rgb(0x797593),
    selection: rgb(0x797593),

    // Main body (on a light background the zebra is a subtle tint just
    // below the background)
    value: rgb(0xD7827E),
    highlight: rgb(0xEA9D34),
    next_marker: rgb(0x797593),
    dim: rgb(0x9893A5),
    zebra_bg: rgb(0xF4EDE8),

    // Opcodes (shared with the pointer arrows and the graph edges): jumps
    // in iris, memory accesses in foam, arithmetic in pine; love red is
    // reserved for STP and breakpoints
    op_jump: rgb(0x907AA9),
    op_memory: rgb(0x56949F),
    op_arithmetic: rgb(0x286983),
    op_test: rgb(0x575279),
    op_halt: rgb(0xB4637A),
    op_num: rgb(0x9893A5),

    // Heatmap ramps (low, mid, high), each in the hue family of what it
    // measures: exec in the jump iris family, reads in the memory foam
    // family, writes in the destination gold family. On a light background
    // the ramps run the other way round from the dark themes: the darkest
    // shade marks the highest count
    exec_ramp: [rgb(0xE3DCEB), rgb(0xB9A8D1), rgb(0x907AA9)],
    read_ramp: [rgb(0xC5DEE2), rgb(0x7FB3BC), rgb(0x56949F)],
    write_ramp: [rgb(0xF6DFC0), rgb(0xEAB96A), rgb(0xEA9D34)],

    // Control-flow graph edges: jump edges take the jump color (backward
    // edges are bold), CMP skip edges the test color, fall-through is dim
    edge_fallthrough: rgb(0x9893A5),
    edge_jump: rgb(0x907AA9),
    edge_skip: rgb(0x575279),

    // Timeline and scrub pop-up (the bar's fill/track are bg blended to
    // chrome at the fill fraction of the shade glyphs it used to draw)
    timeline_fill: blend(rgb(0xFAF4ED), rgb(0x797593), 1, 2),
    timeline_track: blend(rgb(0xFAF4ED), rgb(0x797593), 1, 4),
    timeline_marker: rgb(0xEA9D34),
    scrub_popup_fg: rgb(0x575279),
    scrub_popup_bg: rgb(0xEA9D34),

    // Pop-ups (love for the breakpoint "stop", gold for the editor, pine
    // for save; the preview is the value rose)
    breakpoint_border: rgb(0xB4637A),
    editor_border: rgb(0xEA9D34),
    save_border: rgb(0x286983),
    preview: rgb(0xD7827E),
};
