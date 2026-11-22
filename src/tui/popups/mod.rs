//! Pop-up windows for the TUI: help, breakpoint editor, word editor, and save.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::ssem::Opcode;
use crate::ssem::Store;
use crate::ssem::simulator::{Breakpoint, BreakpointCondition, Simulator};

use super::app::{BIT_REPRESENTATIONS, KeyRepeatGuard, control_for_key, keymap};
use super::layout::{
    FlowMode, HeatmapMode, MAX_COMMENT_WIDTH, timeline_bar_geometry, timeline_filled_len,
};
use super::palette::{Palette, Theme};
use super::snapshot::Snapshot;

/// The folder a source file lives in: its parent, or `.` for a bare file
/// name (whose parent is the empty path).
pub(super) fn parent_dir_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Canonicalize a folder so it is shown and used as absolute, falling back
/// to the path as given when it cannot be resolved (e.g. it no longer
/// exists).
pub(super) fn canonical_dir(dir: PathBuf) -> PathBuf {
    std::fs::canonicalize(&dir).unwrap_or(dir)
}

/// Compute a centered, clamped pop-up rectangle within the given area.
pub(super) fn popup_rect(height: u16, width: u16, area: Rect) -> Rect {
    let height = height.min(area.height);
    let width = width.min(area.width);
    let y = area.y + area.height.saturating_sub(height) / 2;
    let x = area.x + area.width.saturating_sub(width) / 2;
    Rect::new(x, y, width, height)
}

/// The content area inside a bordered block, with a 1-column horizontal margin
/// on each side so text does not touch the border.
/// The character a key event represents, if any.
///
/// Besides plain ASCII digits, this maps the French AZERTY top row: on
/// terminals sending CSI-u key events, Shift+key arrives as the *unshifted*
/// character plus the SHIFT modifier (the shifted key is only reported with
/// the kitty "alternate keys" flag, which we do not enable), so the AZERTY
/// digit keys deliver `& é " ' ( - è _ ç à` + Shift, which we translate back
/// to `1 2 3 4 5 6 7 8 9 0`. With `path_chars`, `;` + Shift also maps to
/// `.` so a dot can be typed in the save pop-up's file name.
pub(super) fn shifted_char_of(key: &KeyEvent, path_chars: bool) -> Option<char> {
    let KeyCode::Char(c) = key.code else {
        return None;
    };
    if c.is_ascii_digit() {
        return Some(c);
    }
    if !key.modifiers.contains(KeyModifiers::SHIFT) {
        return None;
    }
    Some(match c {
        '&' => '1',
        '\u{e9}' => '2', // é
        '"' => '3',
        '\'' => '4',
        '(' => '5',
        '-' => '6',
        '\u{e8}' => '7', // è
        '_' => '8',
        '\u{e7}' => '9', // ç
        '\u{e0}' => '0', // à
        ';' if path_chars => '.',
        _ => return None,
    })
}

/// The character a key event represents when it is a digit, if any
/// (AZERTY top row included), plus `.` for the save pop-up's file name.
pub(super) fn digit_of(key: &KeyEvent) -> Option<char> {
    shifted_char_of(key, true)
}

/// The digit a key event represents, if any, for free-text fields: the
/// AZERTY top row is translated, but not the `;`/`:` path characters,
/// which keep their literal meaning in a comment.
pub(super) fn azerty_digit_of(key: &KeyEvent) -> Option<char> {
    shifted_char_of(key, false)
}

/// The content area inside a bordered block, with a 1-column horizontal margin
/// on each side so text does not touch the border.
fn content_area(block: &Block, rect: Rect) -> Rect {
    let inner = block.inner(rect);
    Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    }
}

/// The pop-up rect width for a content width: 2 border columns plus a
/// 1-column margin on each side, so text does not touch the border.
pub(super) fn popup_width(content_width: usize) -> u16 {
    (content_width + 4) as u16
}

/// The state and chrome shared by every pop-up: the open flag, the rect from
/// the last draw (for mouse hit-testing), and the frame drawing — Clear, the
/// theme's base colors, and the rounded bordered block. Pop-ups embed this as
/// `frame` and route their drawing through [`PopupFrame::begin_popup`].
pub(super) struct PopupFrame {
    pub open: bool,
    pub last_rect: Rect,
}

impl PopupFrame {
    pub fn new() -> Self {
        Self {
            open: false,
            last_rect: Rect::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the given screen cell lies inside the pop-up's last rect.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// Draw the pop-up frame at `rect` and return the content area inside the
    /// border. Records `rect` as the last-drawn rect, clears it, restores the
    /// theme's base colors under it (Clear resets cells to the terminal
    /// default), and renders the rounded bordered block with `title`.
    /// `border_color` colors the border; `None` leaves it in the base fg.
    pub fn begin_popup(
        &mut self,
        f: &mut Frame,
        rect: Rect,
        title: impl Into<Line<'static>>,
        border_color: Option<Color>,
        palette: &Palette,
    ) -> Rect {
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let mut block = Block::bordered()
            .title(title)
            .border_type(BorderType::Rounded);
        if let Some(color) = border_color {
            block = block.border_style(Style::new().fg(color));
        }
        let content = content_area(&block, rect);
        f.render_widget(block, rect);
        content
    }
}

/// Display width of a string in terminal columns.
///
/// Uses character count, which is correct for the characters used in the UI
/// (no double-width CJK or emoji). `.len()` would return byte count and
/// overestimate strings containing multi-byte characters like `·` or `↑`.
pub(super) fn display_width(s: impl AsRef<str>) -> usize {
    s.as_ref().chars().count()
}

/// The screen anchors the welcome tour points at: the main UI elements,
/// recorded by the last draw. Built once per frame in `draw.rs`, after the
/// menu bar is drawn, so `run_button` is always present.
pub(super) struct TourAnchors {
    /// The main body (the store view).
    pub body: Rect,
    /// The history timeline bar.
    pub timeline: Rect,
    /// The Run/Stop menu-bar button.
    pub run_button: Rect,
}

/// Everything a pop-up's `draw` needs besides the frame, bundled so all
/// pop-ups can be drawn through one uniform `Popup::draw(f, ctx)` call.
/// Built once per frame in `draw.rs`.
pub(super) struct PopupDrawCtx<'a> {
    pub area: Rect,
    pub palette: &'static Palette,
    pub snap: &'a Snapshot,
    /// Pre-built settings rows view (speed, modes, theme).
    pub settings_view: SettingsView,
    /// Screen column just right of the store text, where the picker anchors.
    pub picker_anchor_col: u16,
    /// The anchors the welcome tour points at.
    pub tour: TourAnchors,
}

/// What a pop-up did in response to an input event, for `App` to apply.
///
/// Pop-ups mutate the simulator directly where they operate on it (save,
/// load, edit); the effects on `App`-owned state — view-mode settings, the
/// quit flag — come back as events, so `App` stays the only place that
/// touches its own fields in response to pop-up input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PopupEvent {
    /// Nothing happened.
    None,
    /// A settings control was activated (settings pop-up).
    Setting(SettingControl),
    /// The user confirmed the pop-up's action (confirm pop-up); `App`
    /// applies it to the machine (Clear) or to itself (Quit).
    Confirmed(ConfirmAction),
}

/// The operations every pop-up shares, so `App` can hold all of them in one
/// ordered registry (`App::popups_mut`) instead of re-listing the pop-ups at
/// every key, mouse, close and draw site. Adding a pop-up = one field + one
/// entry in that array + an impl of this trait. The chrome methods default
/// to the pop-up's embedded [`PopupFrame`], so an impl only writes the two
/// frame accessors plus the handlers it needs: `handle_key`/`draw`, and
/// `handle_click`/`handle_move` (with `is_modal`/`guards_mode_keys`) for
/// pop-ups with clickable controls or key guards.
pub(super) trait Popup {
    fn frame(&self) -> &PopupFrame;
    fn frame_mut(&mut self) -> &mut PopupFrame;

    fn is_open(&self) -> bool {
        self.frame().is_open()
    }
    fn close(&mut self) {
        self.frame_mut().close()
    }
    /// Whether the screen cell lies inside this pop-up's last drawn rect.
    fn contains(&self, column: u16, row: u16) -> bool {
        self.frame().contains(column, row)
    }
    /// Pop-ups with their own clickable controls (settings, picker, confirm,
    /// open browser) handle mouse clicks themselves through `handle_click`;
    /// for the others a click inside is consumed and a click outside closes
    /// them. The welcome tour also handles its own clicks: a click outside
    /// advances to the next step instead of closing it.
    fn handles_own_clicks(&self) -> bool {
        false
    }
    /// Whether the pop-up is modal: while it is open, the menu bar stays
    /// inert and every non-click mouse event is swallowed (the picker, the
    /// confirm pop-up, the open browser, and the welcome tour).
    fn is_modal(&self) -> bool {
        false
    }
    /// Whether view-mode keys (T/F/M/C/D) must not auto-repeat while this
    /// pop-up is open — a held key would otherwise cycle the modes. Only
    /// the settings pop-up needs it (its keys are the view-mode keys).
    fn guards_mode_keys(&self) -> bool {
        false
    }
    /// Route a key event to the pop-up while it is open, including its close
    /// key, and return what it did. Esc is closed centrally by
    /// `App::route_popup_key`.
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) -> PopupEvent {
        let _ = (key, sim);
        PopupEvent::None
    }
    /// Handle a left click while the pop-up is open. Only called for pop-ups
    /// with `handles_own_clicks`.
    fn handle_click(&mut self, column: u16, row: u16, sim: &mut Simulator) -> PopupEvent {
        let _ = (column, row, sim);
        PopupEvent::None
    }
    /// Update the hover highlight on a mouse move while the pop-up is open.
    fn handle_move(&mut self, column: u16, row: u16) {
        let _ = (column, row);
    }
    /// Handle a mouse-wheel scroll while the pop-up is open (`true` for
    /// ScrollUp). Only the open browser scrolls its list; the default
    /// ignores the wheel. Dispatched to every open pop-up before the modal
    /// swallow in `App::handle_mouse`.
    fn handle_scroll(&mut self, up: bool) {
        let _ = up;
    }
    /// Draw the pop-up as an overlay. The inline comment editor is drawn by
    /// the main body, not as an overlay, so its impl is a no-op.
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let _ = (f, ctx);
    }
}

mod breakpoint;
mod browser;
mod comment;
mod confirm;
mod editor;
mod field;
mod help;
mod picker;
mod settings;
mod welcome;

pub use breakpoint::BreakpointPopup;
pub use browser::OpenPopup;
pub use comment::CommentEditor;
pub use confirm::{ConfirmAction, ConfirmPopup};
pub use editor::EditorPopup;
pub use field::FieldPopup;
pub use help::HelpPopup;
pub use picker::PickerPopup;
pub use settings::{SettingControl, SettingsPopup, SettingsView};
pub use welcome::WelcomePopup;

// Reached into directly by the shared test module.
#[cfg(test)]
use breakpoint::InputField;
#[cfg(test)]
pub use confirm::ConfirmButton;
#[cfg(test)]
use editor::{EditField, EditTarget};
#[cfg(test)]
use field::default_save_name;
#[cfg(test)]
pub use picker::PickerKind;

#[cfg(test)]
mod tests;
