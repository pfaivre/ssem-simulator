//! Layout: geometry constants for the bars and main body, and the timeline
//! bar's geometry helpers.

use ratatui::layout::Rect;

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
