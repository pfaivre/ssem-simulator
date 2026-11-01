//! Layout: geometry constants for the bars and main body, the timeline bar's
//! geometry helpers, and the optional view modes (heatmap, flow) with the
//! width they add to the store view.

use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

/// Minimum terminal width to display the store in full.
pub(super) const MIN_WIDTH: u16 = 59;

/// Minimum terminal height (top bar + content + timeline + bottom bar).
pub(super) const MIN_HEIGHT: u16 = 13;

/// Total height of the fixed bars: top bar + timeline + bottom bar.
pub(super) const CHROME_HEIGHT: u16 = 3;

/// Maximum width of the timeline bar (in cells).
const MAX_TIMELINE_BAR_WIDTH: usize = 100;

/// Column and width of the timeline bar within the timeline area.
pub(super) fn timeline_bar_geometry(area: Rect, start: u64, head: u64) -> (u16, u16) {
    const PREFIX: &str = " HISTORY [";
    let suffix_len = format!("] {start}..{head}").len();
    let width = (area.width as usize)
        .saturating_sub(PREFIX.len() + suffix_len)
        .min(MAX_TIMELINE_BAR_WIDTH);
    (area.x + PREFIX.len() as u16, width as u16)
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

/// Width of a store line's text: pointer column (1) + bits (32) + suffix (25),
/// excluding the optional heatmap bar.
pub(super) const STORE_TEXT_WIDTH: usize = 58;

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
    pub(super) fn label(self) -> Option<&'static str> {
        match self {
            HeatmapMode::Off => None,
            HeatmapMode::Exec => Some("EXEC"),
            HeatmapMode::Rw => Some("R/W"),
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
    pub(super) fn label(self) -> Option<&'static str> {
        match self {
            FlowMode::Control => Some("CONTROL"),
            FlowMode::Data => Some("DATA"),
            FlowMode::Off => None,
        }
    }
}
