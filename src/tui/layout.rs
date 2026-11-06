//! Layout: geometry constants for the bars and main body, the timeline bar's
//! geometry helpers, and the optional view modes (heatmap, flow) with the
//! width they add to the store view.

use ratatui::layout::Rect;
use rust_i18n::t;
use serde::{Deserialize, Serialize};

/// Minimum terminal width to display the store in full.
pub(super) const MIN_WIDTH: u16 = 59;

/// Minimum terminal height (menu bar + content + history bar + status bar).
pub(super) const MIN_HEIGHT: u16 = 13;

/// Total height of the fixed bars: menu bar + history scrub bar + status bar.
pub(super) const CHROME_HEIGHT: u16 = 3;

/// Maximum width of the timeline bar (in cells).
const MAX_TIMELINE_BAR_WIDTH: usize = 100;

/// Column and width of the timeline bar within the timeline area.
pub(super) fn timeline_bar_geometry(area: Rect, start: u64, head: u64) -> (u16, u16) {
    let prefix = t!("history.prefix");
    let suffix_len = format!("] {start}..{head}").len();
    let bar_width = (area.width as usize)
        .saturating_sub(prefix.len() + suffix_len)
        .min(MAX_TIMELINE_BAR_WIDTH);
    let total = prefix.len() + bar_width + suffix_len;
    // Mirror ratatui's Alignment::Center: (area / 2) - (line / 2)
    let offset = (area.width as usize / 2).saturating_sub(total / 2);
    (
        area.x + offset as u16 + prefix.len() as u16,
        bar_width as u16,
    )
}

/// Offset (in cells) of the current-position marker within the timeline bar:
/// the current cycle's position between the oldest retained cycle and the
/// tip. Returns None when there is no bar or no retained history.
pub(super) fn timeline_marker_offset(
    bar_width: u16,
    start: u64,
    head: u64,
    cycle_count: u64,
) -> Option<usize> {
    if bar_width == 0 || head < start {
        return None;
    }
    let pos = cycle_count.clamp(start, head);
    let span = head - start;
    let offset = ((pos - start) * (u64::from(bar_width) - 1))
        .checked_div(span)
        .unwrap_or(0) as usize;
    Some(offset.min(usize::from(bar_width) - 1))
}

/// Total number of lines in the main body content:
/// 1 blank + CI + A + 1 blank + 32 store words.
pub(super) const CONTENT_HEIGHT: usize = 36;

/// Content row of the CI register in the main body
/// (0 blank, 1 CI, 2 A, 3 blank, 4..=35 store words 0..=31).
pub(super) const CI_ROW: usize = 1;

/// Content row of the accumulator in the main body.
pub(super) const A_ROW: usize = 2;

/// Content row of store word 0; word `i` is at content row `STORE_ROW_OFFSET + i`.
pub(super) const STORE_ROW_OFFSET: usize = 4;

/// Width of a store line's text: pointer column (1) + bits (32) + suffix (18)
/// + mnemonic (4) + data (3), excluding the optional heatmap bar.
pub(super) const STORE_TEXT_WIDTH: usize = 58;

/// Width of the pointer column (`>`, `»`, `•` or blank) at the start of a
/// store line.
pub(super) const POINTER_WIDTH: usize = 1;

/// Width of the 32-bit word display.
pub(super) const BITS_WIDTH: usize = 32;

/// Width of the " NN: value  " suffix between the bits and the mnemonic.
pub(super) const SUFFIX_WIDTH: usize = 18;

/// Width of the mnemonic field (left-aligned, padded to 4).
pub(super) const MNEMONIC_WIDTH: usize = 4;

/// Width of the data field (" NN" when the opcode uses an address, else blank).
pub(super) const DATA_WIDTH: usize = 3;

const _: () = assert!(
    STORE_TEXT_WIDTH == POINTER_WIDTH + BITS_WIDTH + SUFFIX_WIDTH + MNEMONIC_WIDTH + DATA_WIDTH
);

/// Rounding integer division `(n + d/2) / d`, mirroring the scrollbar
/// widget's thumb math so the app's hit-testing matches what is drawn.
pub(super) const fn rounding_divide(numerator: usize, denominator: usize) -> usize {
    (numerator + denominator / 2) / denominator
}

/// The scrollbar metrics of the main body for a body `body_height` rows
/// tall: (track length, thumb length, max scroll). The scrollbar has no
/// begin/end arrows, so the track spans the whole body. The thumb length
/// is zero when the whole content fits (no scrollbar drawn).
pub(super) fn scrollbar_metrics(body_height: u16) -> (usize, usize, usize) {
    let body = body_height as usize;
    let max_scroll = CONTENT_HEIGHT.saturating_sub(body);
    let track = body;
    if max_scroll == 0 || track == 0 {
        return (track, 0, max_scroll);
    }
    // The scrollbar widget is given content_length = max_scroll + 1 and
    // viewport_content_length = body_height, so its max viewport position
    // is max_scroll + body_height (== CONTENT_HEIGHT here)
    let max_viewport = max_scroll + body;
    let thumb = rounding_divide(body * track, max_viewport).clamp(1, track);
    (track, thumb, max_scroll)
}

/// The thumb's start offset within the scrollbar track for the given
/// scroll, mirroring the widget's math (clamped so the thumb stays on the
/// track). The thumb's first screen row is then `1 + start`: the main body
/// starts at screen row 1 and the scrollbar has no begin arrow.
pub(super) fn scrollbar_thumb_start(body_height: u16, scroll: u16) -> usize {
    let (track, thumb, max_scroll) = scrollbar_metrics(body_height);
    if track == 0 {
        return 0;
    }
    let max_viewport = max_scroll + body_height as usize;
    rounding_divide(scroll as usize * track, max_viewport).clamp(0, track - thumb)
}

/// The scroll that puts the thumb's start at the given track offset (the
/// inverse of `scrollbar_thumb_start`), clamped to the scroll range.
pub(super) fn scrollbar_scroll_for_thumb_start(body_height: u16, thumb_start: usize) -> u16 {
    let (track, _, max_scroll) = scrollbar_metrics(body_height);
    if track == 0 {
        return 0;
    }
    let max_viewport = max_scroll + body_height as usize;
    rounding_divide(thumb_start * max_viewport, track).min(max_scroll) as u16
}

/// Screen column where the mnemonic field of a store row starts: the pointer,
/// then the heatmap bars (spliced in right after the pointer), then the bits
/// and the suffix. Mirrors the span order in `draw_main_body`.
pub(super) fn mnemonic_column(heatmap: HeatmapMode) -> usize {
    POINTER_WIDTH + heatmap.extra_width() as usize + BITS_WIDTH + SUFFIX_WIDTH
}

/// Screen column where the data field of a store row starts (right after
/// the mnemonic).
pub(super) fn data_column(heatmap: HeatmapMode) -> usize {
    mnemonic_column(heatmap) + MNEMONIC_WIDTH
}

/// Width of the CI and accumulator lines' text: bits with leading space (33)
/// + label (16), excluding the optional heatmap spacer.
pub(super) const REGISTER_TEXT_WIDTH: usize = 49;

/// Maximum width (in cells) of the source-comment column: longer comments
/// are truncated to fit.
pub(super) const MAX_COMMENT_WIDTH: usize = 80;

/// Minimum width (in cells) of the source-comment column when comments are
/// shown: even when no word has a comment yet, a minimal column (the " ;"
/// prefix plus this many cells) is kept, so a comment can be added by
/// double-clicking a store row.
pub(super) const MIN_COMMENT_WIDTH: usize = 1;

/// Width of the source-comment column for the given per-word comments:
/// the widest comment, capped at `MAX_COMMENT_WIDTH`. Zero when no word
/// has a comment (the drawn column then falls back to `MIN_COMMENT_WIDTH`,
/// see `drawn_comment_width`).
pub(super) fn comment_column_width(comments: &[Option<String>; 32]) -> usize {
    comments
        .iter()
        .filter_map(|c| c.as_deref())
        .map(|c| c.chars().count())
        .max()
        .unwrap_or(0)
        .min(MAX_COMMENT_WIDTH)
}

/// Width of the execution heatmap bar (in cells) shown next to each store word.
pub(super) const HEATMAP_BAR_WIDTH: usize = 6;

/// Width of each bar (reads, writes) in the read/write heatmap.
pub(super) const RW_BAR_WIDTH: usize = 3;

/// Extra width required by the execution heatmap (bar + surrounding spaces).
const EXEC_EXTRA_WIDTH: u16 = (HEATMAP_BAR_WIDTH + 2) as u16;

/// Extra width required by the read/write heatmap:
/// leading space + "r" + bar + space + "w" + bar + trailing space.
const RW_EXTRA_WIDTH: u16 = (2 + RW_BAR_WIDTH + 2 + RW_BAR_WIDTH + 1) as u16;

/// Which heatmap (if any) is shown next to each store word in the store view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum HeatmapMode {
    /// No heatmap.
    Off,
    /// Execution frequency of each store word, as one bar per word.
    Exec,
    /// Per-word store reads vs writes, as two labeled bars per word.
    Rw,
}

impl HeatmapMode {
    /// The next mode when the heatmap key is pressed: off → exec → r/w → off.
    pub(super) fn cycle(self) -> Self {
        match self {
            HeatmapMode::Off => HeatmapMode::Exec,
            HeatmapMode::Exec => HeatmapMode::Rw,
            HeatmapMode::Rw => HeatmapMode::Off,
        }
    }

    /// Extra screen width (in cells) the mode adds next to each store word.
    pub(super) fn extra_width(self) -> u16 {
        match self {
            HeatmapMode::Off => 0,
            HeatmapMode::Exec => EXEC_EXTRA_WIDTH,
            HeatmapMode::Rw => RW_EXTRA_WIDTH,
        }
    }

    /// The label shown in the top bar, or None when the heatmap is off.
    pub(super) fn label(self) -> Option<String> {
        match self {
            HeatmapMode::Off => None,
            HeatmapMode::Exec => Some(ts!("mode.heatmap_exec")),
            HeatmapMode::Rw => Some(ts!("mode.heatmap_rw")),
        }
    }
}

/// Which flow visualization (if any) is shown next to the store view,
/// cycled with the F key like the heatmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum FlowMode {
    /// The control flow between the store lines, drawn as a static graph.
    Control,
    /// The data flow of the last executed instruction, drawn as a pointer arrow.
    Data,
    /// No flow visualization.
    Off,
}

impl FlowMode {
    /// The next mode when the key is pressed: control → data → off → control.
    pub(super) fn cycle(self) -> Self {
        match self {
            FlowMode::Control => FlowMode::Data,
            FlowMode::Data => FlowMode::Off,
            FlowMode::Off => FlowMode::Control,
        }
    }

    /// The label shown in the top bar, or None when the visualization is off.
    pub(super) fn label(self) -> Option<String> {
        match self {
            FlowMode::Control => Some(ts!("mode.flow_control")),
            FlowMode::Data => Some(ts!("mode.flow_data")),
            FlowMode::Off => None,
        }
    }
}
