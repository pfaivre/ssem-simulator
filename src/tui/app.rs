//! The application state (UI thread only) and its input handling:
//! keyboard, mouse, history scrubbing, speed display.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;

use super::config::Config;
use super::draw::{comment_column_start, drawn_comment_width, effective_heatmap};
use super::layout::{
    CHROME_HEIGHT, CONTENT_HEIGHT, DATA_WIDTH, FlowMode, HeatmapMode, MIN_HEIGHT, MIN_WIDTH,
    MNEMONIC_WIDTH, STORE_ROW_OFFSET, data_column, mnemonic_column, scrollbar_metrics,
    scrollbar_scroll_for_thumb_start, scrollbar_thumb_start, timeline_bar_geometry,
};
use super::palette::{Palette, Theme};
use super::popups::{
    BreakpointPopup, CommentEditor, EditorPopup, HelpPopup, PickerPopup, SavePopup,
};
use crate::ssem::history::History;
use crate::ssem::simulator::Simulator;

/// Bit representations cycled through with the D key: (on_char, off_char).
pub(super) const BIT_REPRESENTATIONS: [(char, char); 5] = [
    ('#', '.'),             // High contrast ASCII
    ('\u{25CF}', '\u{b7}'), // Fancy characters (● and ·)
    ('\u{2212}', '\u{b7}'), // Fancy characters (− and ·)
    ('_', '.'),             // ASCII compatible
    ('1', '0'),             // Binary digits
];

/// The keymap: (key, legend, help description).
/// An empty legend means the key does not appear in the bottom bar (e.g. arrow keys).
pub(super) const KEYMAP: &[(&str, &str, &str)] = &[
    ("Q", "QUIT", "Quit the simulator"),
    ("H", "HELP", "Toggle this help pop-up"),
    ("P", "RUN/STOP", "Run the program or pause it"),
    ("F10", "STEP", "Execute one instruction"),
    (
        "\u{2190}/\u{2192}",
        "BACK/FWD",
        "Step back/forward one cycle in the execution history",
    ),
    ("\u{2191}/\u{2193}", "", "Scroll the store view"),
    ("I", "SPEED UP", "Increase the execution speed"),
    ("K", "SPEED DOWN", "Decrease the execution speed"),
    ("D", "DISPLAY", "Cycle the bit representations"),
    (
        "E",
        "EDIT",
        "Edit a register or store word (double-click its line)",
    ),
    (
        "M",
        "HEATMAP",
        "Cycle the heatmap: off, execution frequency, reads/writes",
    ),
    (
        "F",
        "FLOW",
        "Cycle the flow view: data flow, control flow, off",
    ),
    (
        "C",
        "COMMENTS",
        "Toggle the source-comment column next to each store word",
    ),
    ("B", "BREAKPT", "Open the breakpoint pop-up"),
    (
        "S",
        "SAVE",
        "Save the machine state to a .asm/.snp file in the input file's folder",
    ),
    ("T", "THEME", "Cycle through the color themes"),
    (
        "MOUSE",
        "",
        "Drag the timeline bar to scrub the history; double-click a comment cell to edit it.",
    ),
];

/// How often the displayed speed is refreshed.
const SPEED_DISPLAY_PERIOD: Duration = Duration::from_millis(500);

/// Smoothing factor (in seconds) for the speed display.
const SPEED_SMOOTHING: f64 = 0.5;

/// Maximum time between two left-clicks for them to count as a double-click.
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(500);

/// Minimum interval between two presses of the same view-mode key (T/F/M/C/D)
/// for the second press to count. OS key-repeat fires every ~30 ms, so a held
/// key is swallowed; a human double-press (~100 ms apart) always counts.
const MODE_KEY_REPEAT_GUARD: Duration = Duration::from_millis(50);

/// Whether the key is a view-mode key that must not auto-repeat when held.
fn is_mode_key(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Char('m' | 'M' | 'f' | 'F' | 'c' | 'C' | 't' | 'T' | 'd' | 'D')
    )
}

/// The application state for the TUI (UI thread only).
pub(super) struct App {
    pub(super) bit_representation: usize,
    pub(super) store_scroll: u16,
    pub(super) last_area: Rect,

    // Speed display: exponentially smoothed
    pub(super) current_speed: f64,
    pub(super) displayed_speed: u32,
    pub(super) last_speed_refresh: Instant,
    pub(super) last_refresh_time: Instant,
    pub(super) last_cycles: u64,

    pub(super) help_popup: HelpPopup,
    pub(super) breakpoint_popup: BreakpointPopup,
    pub(super) editor_popup: EditorPopup,
    pub(super) save_popup: SavePopup,
    /// The inline comment editor, opened by double-clicking the comment
    /// column of a store row.
    pub(super) comment_editor: CommentEditor,
    /// The mnemonic/data picker, opened by double-clicking the mnemonic or
    /// data field of a store row.
    pub(super) picker: PickerPopup,

    /// The last left-click, for double-click detection: (time, column, row).
    pub(super) last_click: Option<(Instant, u16, u16)>,

    /// Which heatmap is shown in the store view (cycled with the M key).
    pub(super) heatmap: HeatmapMode,

    /// Which flow visualization (if any) is shown next to the store view
    /// (cycled with the F key).
    pub(super) flow: FlowMode,

    /// Whether the source-comment column is shown next to each store word
    /// (toggled with the C key). A minimal column is kept even when no word
    /// has a comment yet, so one can be added by double-clicking a row.
    pub(super) show_comments: bool,

    /// The color theme used to draw the interface (cycled with the T key,
    /// or chosen with the `--theme` command-line option).
    pub(super) theme: Theme,

    /// Where to save the view modes when the application closes
    /// (None in tests: persistence disabled).
    pub(super) config_path: Option<PathBuf>,

    /// The last config-save failure, reported when the TUI exits.
    pub(super) config_error: Option<String>,

    /// The last view-mode key press (key, time), to swallow OS key-repeat.
    pub(super) last_mode_key: Option<(KeyCode, Instant)>,

    /// Minimum interval between two presses of the same view-mode key for the
    /// second one to count (tests set it to zero).
    pub(super) mode_key_guard: Duration,

    /// Whether the user is currently dragging the timeline scrubber.
    pub(super) scrubbing: bool,

    /// While dragging the main body's scrollbar, the offset between the
    /// cursor's row and the thumb's top row at grab time, so the thumb
    /// follows the cursor 1:1. None when not dragging.
    pub(super) scrollbar_grab: Option<i32>,

    /// Screen area of the timeline bar, for mouse hit-testing.
    pub(super) timeline_area: Rect,

    pub(super) should_quit: bool,
}

impl App {
    /// An app with the default view modes and no config file (tests).
    #[cfg(test)]
    pub(super) fn new() -> Self {
        let mut app = Self::from_parts(Config::default(), None, None);
        // Tests press the same key in quick succession on purpose
        app.mode_key_guard = Duration::ZERO;
        app
    }

    /// An app starting from the given persisted view modes, saving changes
    /// back to `config_path`. The `theme_override` (from `--theme`) wins
    /// over the persisted theme for this session.
    pub(super) fn from_parts(
        config: Config,
        theme_override: Option<Theme>,
        config_path: Option<PathBuf>,
    ) -> Self {
        let now = Instant::now();
        App {
            // Clamp so a hand-edited out-of-range index cannot panic format_bits
            bit_representation: config.bit_representation.min(BIT_REPRESENTATIONS.len() - 1),
            store_scroll: 0,
            last_area: Rect::default(),
            current_speed: 0.0,
            displayed_speed: 0,
            last_speed_refresh: now,
            last_refresh_time: now,
            last_cycles: 0,
            help_popup: HelpPopup::new(),
            breakpoint_popup: BreakpointPopup::new(),
            editor_popup: EditorPopup::new(),
            save_popup: SavePopup::new(),
            comment_editor: CommentEditor::new(),
            picker: PickerPopup::new(),
            last_click: None,
            heatmap: config.heatmap,
            flow: config.flow,
            show_comments: config.comments,
            theme: theme_override.unwrap_or(config.theme),
            config_path,
            config_error: None,
            last_mode_key: None,
            mode_key_guard: MODE_KEY_REPEAT_GUARD,
            scrubbing: false,
            scrollbar_grab: None,
            timeline_area: Rect::default(),
            should_quit: false,
        }
    }

    /// Persist the current view modes to the config file, called once when
    /// the application closes (best effort: a failure is remembered in
    /// `config_error` and reported after the TUI exits).
    pub(super) fn save_config(&mut self) {
        let Some(path) = &self.config_path else {
            return;
        };
        let config = Config {
            theme: self.theme,
            flow: self.flow,
            heatmap: self.heatmap,
            comments: self.show_comments,
            bit_representation: self.bit_representation,
        };
        if let Err(e) = config.save(path) {
            self.config_error = Some(format!("could not save {}: {e}", path.display()));
        }
    }

    /// The palette of the current color theme.
    pub(super) fn palette(&self) -> &'static Palette {
        self.theme.palette()
    }

    /// Update the smoothed speed display value from the latest cycle count.
    pub(super) fn update_speed_display(&mut self, is_running: bool, cycle_count: u64) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refresh_time);
        self.last_refresh_time = now;

        if is_running {
            // saturating_sub: rewinding the history makes cycle_count jump backward
            let cycles_delta = cycle_count.saturating_sub(self.last_cycles);
            let measured = cycles_delta as f64 / elapsed.as_secs_f64().max(1e-9);
            let alpha = 1.0 - (-elapsed.as_secs_f64() / SPEED_SMOOTHING).exp();
            self.current_speed += alpha * (measured - self.current_speed);

            if now.duration_since(self.last_speed_refresh) >= SPEED_DISPLAY_PERIOD {
                self.displayed_speed = self.current_speed as u32;
                self.last_speed_refresh = now;
            }
        } else {
            self.current_speed = 0.0;
            self.displayed_speed = 0;
        }

        self.last_cycles = cycle_count;
    }

    /// Handle a keyboard event, mutating the simulator as needed.
    pub(super) fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Holding a key may report Repeat events. Only the scrub keys act on
        // repeats (for continuous time-travel); the view-mode keys (T/F/M/C/D)
        // must not repeat, so they are ignored here.
        // Pop-ups keep priority over the scrub keys.
        if key.kind == KeyEventKind::Repeat {
            if self.help_popup.is_open() {
                self.help_popup.handle_key(key);
            } else if self.save_popup.is_open() {
                self.save_popup.handle_key(key, sim);
            } else if self.comment_editor.is_open() {
                self.comment_editor.handle_key(key, sim);
            } else if self.picker.is_open() {
                self.picker_key(key, sim);
            } else if !self.breakpoint_popup.is_open() && !self.editor_popup.is_open() {
                self.handle_scrub_key(key.code, sim);
            }
            return;
        }

        // Only handle key press events (ignore release on platforms that report them)
        if key.kind != KeyEventKind::Press {
            return;
        }

        // While the picker is open, keys go to it: arrows move the
        // selection, Enter applies it (inside handle_key), Esc cancels
        if self.picker.is_open() {
            self.picker_key(key, sim);
            return;
        }

        // While the breakpoint pop-up is open, keys go to it
        if self.breakpoint_popup.is_open() {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('b')
                || key.code == KeyCode::Char('B')
            {
                self.breakpoint_popup.close();
            } else {
                self.breakpoint_popup.handle_key(key, &mut sim.breakpoints);
            }
            return;
        }

        // While the editor pop-up is open, keys go to it
        if self.editor_popup.is_open() {
            if key.code == KeyCode::Esc
                || key.code == KeyCode::Char('e')
                || key.code == KeyCode::Char('E')
            {
                self.editor_popup.close();
            } else {
                self.editor_popup.handle_key(key, sim);
            }
            return;
        }

        // While the inline comment editor is open, keys go to it: Enter
        // validates (inside handle_key), Esc cancels, every other key types
        // into the comment
        if self.comment_editor.is_open() {
            if key.code == KeyCode::Esc {
                self.comment_editor.close();
            } else {
                self.comment_editor.handle_key(key, sim);
            }
            return;
        }

        // While the save pop-up is open, keys go to it; only Esc closes it,
        // because every letter can appear in a file name
        if self.save_popup.is_open() {
            if key.code == KeyCode::Esc {
                self.save_popup.close();
            } else {
                self.save_popup.handle_key(key, sim);
            }
            return;
        }

        // While the help pop-up is open, H or Esc closes it; other keys scroll it
        if self.help_popup.is_open() {
            if key.code == KeyCode::Char('h')
                || key.code == KeyCode::Char('H')
                || key.code == KeyCode::Esc
            {
                self.help_popup.close();
            } else {
                self.help_popup.handle_key(key);
            }
            return;
        }

        // Swallow OS key-repeat for the view-mode keys (T/F/M/C/D): a held
        // key fires a press every ~30 ms, which must not cycle the modes.
        // This is what makes the no-repeat rule hold on every terminal,
        // without relying on the kitty keyboard protocol.
        if is_mode_key(key.code) && !self.mode_key_accepted(key.code) {
            return;
        }

        match key.code {
            KeyCode::Char(c) => match c.to_ascii_lowercase() {
                'q' => self.should_quit = true,
                'p' => sim.stop_flag = !sim.stop_flag,
                'i' => Self::speed_up(sim),
                'k' => Self::speed_down(sim),
                'd' => {
                    self.bit_representation =
                        (self.bit_representation + 1) % BIT_REPRESENTATIONS.len();
                }
                'b' => self.breakpoint_popup.open(),
                'e' => {
                    self.comment_editor.close();
                    self.editor_popup.open();
                }
                's' => self.save_popup.open(sim.source_path.as_deref()),
                'm' => self.heatmap = self.heatmap.cycle(),
                'f' => self.flow = self.flow.cycle(),
                'c' => self.show_comments = !self.show_comments,
                't' => self.theme = self.theme.cycle(),
                'h' => self.help_popup.open(),
                _ => {}
            },
            KeyCode::F(10) => {
                // Step: stop the machine and execute one instruction
                sim.stop_flag = true;
                sim.instruction_cycle();
                sim.check_breakpoints();
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Home | KeyCode::End => {
                self.handle_scrub_key(key.code, sim)
            }
            KeyCode::Up => {
                self.store_scroll = self.store_scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                let visible_height = self.last_area.height.saturating_sub(CHROME_HEIGHT) as usize;
                let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
                self.store_scroll = (self.store_scroll as usize + 1).min(max_scroll) as u16;
            }
            KeyCode::Esc => self.should_quit = true,
            _ => {}
        }
    }

    /// Whether a view-mode key press should be acted on: false when it is an
    /// OS key-repeat arriving less than `mode_key_guard` after the previous
    /// press of the same key. The last-press time is updated on every event,
    /// so a whole burst of repeats is swallowed.
    fn mode_key_accepted(&mut self, code: KeyCode) -> bool {
        let now = Instant::now();
        let accepted = self
            .last_mode_key
            .is_none_or(|(last, at)| last != code || now.duration_since(at) >= self.mode_key_guard);
        self.last_mode_key = Some((code, now));
        accepted
    }

    /// Route a key to the picker while it is open (arrows select, Enter
    /// applies, Esc cancels). When the key closes the picker, the last
    /// click is forgotten so the next click cannot chain into a
    /// double-click with the one that opened the picker.
    fn picker_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.code == KeyCode::Esc {
            self.picker.close();
        } else {
            self.picker.handle_key(key, sim);
        }
        if !self.picker.is_open() {
            self.last_click = None;
        }
    }

    /// Handle a history-scrubbing key: Left Right Home End.
    pub(super) fn handle_scrub_key(&mut self, code: KeyCode, sim: &mut Simulator) {
        match code {
            KeyCode::Left => self.scrub_relative(-1, sim),
            KeyCode::Right => self.scrub_relative(1, sim),
            KeyCode::Home => {
                if let Some(target) = sim.history.as_ref().map(History::start)
                    && target != sim.cycle_count
                {
                    sim.rewind_to(target);
                }
            }
            KeyCode::End => {
                if let Some(target) = sim.history.as_ref().map(History::head)
                    && target != sim.cycle_count
                {
                    sim.rewind_to(target);
                }
            }
            _ => {}
        }
    }

    /// Scrub the history by `delta` cycles (negative goes back in time).
    pub(super) fn scrub_relative(&mut self, delta: i64, sim: &mut Simulator) {
        let Some(history) = &sim.history else {
            return;
        };
        if history.is_empty() {
            return;
        }
        let target = (sim.cycle_count as i64 + delta)
            .clamp(history.start() as i64, history.head() as i64) as u64;
        if target != sim.cycle_count {
            sim.rewind_to(target);
        }
    }

    /// Rewind to the cycle under the given screen column on the timeline bar.
    pub(super) fn scrub_to_column(&mut self, column: u16, sim: &mut Simulator) {
        let Some(history) = &sim.history else {
            return;
        };
        if history.is_empty() {
            return;
        }
        let (start, head) = (history.start(), history.head());
        let (bar_x, bar_width) = timeline_bar_geometry(self.timeline_area, start, head);
        let cycle = if bar_width <= 1 {
            start
        } else {
            let offset = (column as i32 - bar_x as i32).clamp(0, bar_width as i32 - 1) as u64;
            start + offset * (head - start) / (bar_width as u64 - 1)
        };
        if cycle != sim.cycle_count {
            sim.rewind_to(cycle);
        }
    }

    /// Handle a mouse event: scrub the timeline, double-click to edit,
    /// scroll wheel to scroll.
    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent, sim: &mut Simulator) {
        // Releasing the left button ends a scrub or a scrollbar drag,
        // wherever the cursor is
        if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
            self.scrubbing = false;
            self.scrollbar_grab = None;
            return;
        }

        // While scrubbing, drags anywhere on screen move the cursor
        // (scrub_to_column clamps the column to the bar)
        if self.scrubbing && matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left)) {
            self.scrub_to_column(mouse.column + 1, sim);
            return;
        }

        // While dragging the scrollbar, drags anywhere on screen move the
        // thumb, even off the scrollbar column (scroll_thumb_to_row clamps
        // the row to the track)
        if let Some(offset) = self.scrollbar_grab
            && matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left))
        {
            let target = (mouse.row as i32 - offset).max(0) as u16;
            self.scroll_thumb_to_row(target);
            return;
        }

        // While the picker is open, it is modal: a click on an entry
        // applies the choice, any other click closes it without changes
        if self.picker.is_open() {
            if matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
                match self.picker.entry_at(mouse.column, mouse.row) {
                    Some(entry) => self.picker.select_and_apply(entry, sim),
                    None => self.picker.close(),
                }
                // The picker is closed now: forget the last click so the
                // next click cannot chain into a double-click with the one
                // that opened the picker
                self.last_click = None;
            }
            return;
        }

        // The wheel over the help pop-up scrolls its text
        if self.help_popup.is_open()
            && matches!(
                mouse.kind,
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
            )
            && self.help_popup.contains(mouse.column, mouse.row)
        {
            let delta = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                -1
            } else {
                1
            };
            self.help_popup.scroll_by(delta);
            return;
        }

        // The timeline row: click to scrub, scroll wheel to step
        if self.timeline_area.height > 0 && mouse.row == self.timeline_area.y {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Scrubbing requires a non-empty history
                    if sim.history.as_ref().is_some_and(|h| !h.is_empty()) {
                        self.scrubbing = true;
                        self.scrub_to_column(mouse.column + 1, sim);
                    }
                }
                MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                    let direction = if matches!(mouse.kind, MouseEventKind::ScrollUp) {
                        -1
                    } else {
                        1
                    };
                    let step = sim
                        .history
                        .as_ref()
                        .map(|h| (h.len() / 100).max(1))
                        .unwrap_or(0);
                    if step > 0 {
                        self.scrub_relative(direction * step as i64, sim);
                    }
                }
                _ => {}
            }
            return;
        }

        // The scrollbar on the right edge of the main body (shown when the
        // window is too short for the whole body): press to jump the thumb
        // there and grab it. The drag itself is handled above, so it keeps
        // working when the cursor leaves the column; the wheel keeps working
        // on that column too (handled by the generic scroll below).
        if self.scrollbar_hit(mouse.column, mouse.row)
            && matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left))
        {
            self.scroll_thumb_to_row(mouse.row);
            let body_height = self.last_area.height.saturating_sub(CHROME_HEIGHT);
            let thumb_top = 1 + scrollbar_thumb_start(body_height, self.store_scroll) as i32;
            self.scrollbar_grab = Some(mouse.row as i32 - thumb_top);
            return;
        }

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let now = Instant::now();
                let is_double_click = self
                    .last_click
                    .map(|(time, col, row)| {
                        now.duration_since(time) < DOUBLE_CLICK_THRESHOLD
                            && col == mouse.column
                            && row == mouse.row
                    })
                    .unwrap_or(false);

                self.last_click = Some((now, mouse.column, mouse.row));

                if is_double_click {
                    // Double-click on the comment column edits that word's
                    // comment inline, on the mnemonic or data field opens
                    // the picker list for that field; anywhere else opens
                    // the word editor
                    if let Some(index) =
                        self.comment_cell_at(mouse.column, mouse.row, &sim.store.comments)
                    {
                        self.editor_popup.close();
                        self.comment_editor
                            .open_for(index, sim.store.comments[index].as_deref());
                    } else if let Some(index) = self.mnemonic_cell_at(mouse.column, mouse.row) {
                        self.editor_popup.close();
                        self.comment_editor.close();
                        self.picker
                            .open_for_mnemonic(index, sim.store.words[index], mouse.row);
                    } else if let Some(index) = self.data_cell_at(mouse.column, mouse.row) {
                        self.editor_popup.close();
                        self.comment_editor.close();
                        self.picker
                            .open_for_data(index, sim.store.words[index], mouse.row);
                    } else {
                        self.comment_editor.close();
                        self.try_open_editor_at(mouse.row);
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.store_scroll = self.store_scroll.saturating_sub(1);
            }
            MouseEventKind::ScrollDown => {
                let visible_height = self.last_area.height.saturating_sub(CHROME_HEIGHT) as usize;
                let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
                self.store_scroll = (self.store_scroll as usize + 1).min(max_scroll) as u16;
            }
            _ => {}
        }
    }

    /// If the screen position (column, row) lies in the comment column of a
    /// store-word row, return that word's index. The geometry mirrors
    /// `draw`: the column starts after the store text, the (effective)
    /// heatmap, and the flow gutter, and exists whenever comments are shown
    /// — a minimal column is kept even when no word has a comment yet, so
    /// one can be added (or an edit is in progress).
    fn comment_cell_at(
        &self,
        column: u16,
        row: u16,
        comments: &[Option<String>; 32],
    ) -> Option<usize> {
        let area = self.last_area;
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return None;
        }
        let heatmap = effective_heatmap(area.width, self.heatmap);
        let comment_width = drawn_comment_width(
            area.width,
            heatmap,
            self.flow,
            comments,
            self.comment_editor.edit_width(),
            self.show_comments || self.comment_editor.is_open(),
        );
        if comment_width == 0 {
            return None;
        }
        if (column as usize) < comment_column_start(heatmap, self.flow) {
            return None;
        }

        self.store_index_at_row(row)
    }

    /// If the screen position (column, row) lies in the mnemonic field of a
    /// store-word row, return that word's index. The column mirrors `draw`:
    /// pointer, heatmap bars, bits, suffix, then the 4-cell mnemonic.
    fn mnemonic_cell_at(&self, column: u16, row: u16) -> Option<usize> {
        let index = self.store_index_at_row(row)?;
        let heatmap = effective_heatmap(self.last_area.width, self.heatmap);
        let start = mnemonic_column(heatmap);
        (start..start + MNEMONIC_WIDTH)
            .contains(&(column as usize))
            .then_some(index)
    }

    /// If the screen position (column, row) lies in the data field of a
    /// store-word row, return that word's index (the 3 cells right after
    /// the mnemonic).
    fn data_cell_at(&self, column: u16, row: u16) -> Option<usize> {
        let index = self.store_index_at_row(row)?;
        let heatmap = effective_heatmap(self.last_area.width, self.heatmap);
        let start = data_column(heatmap);
        (start..start + DATA_WIDTH)
            .contains(&(column as usize))
            .then_some(index)
    }

    /// The store word drawn on the given screen row, if any. Same row
    /// mapping as `try_open_editor_at`: the main body starts at row 1
    /// (below the top bar) and is scrolled by `store_scroll` lines.
    fn store_index_at_row(&self, row: u16) -> Option<usize> {
        let area = self.last_area;
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return None;
        }
        let body_top = 1u16;
        let body_height = area.height.saturating_sub(CHROME_HEIGHT);
        if row < body_top || row >= body_top + body_height {
            return None;
        }
        let content_row =
            (row as usize).saturating_sub(body_top as usize) + self.store_scroll as usize;
        match content_row {
            r if (STORE_ROW_OFFSET..CONTENT_HEIGHT).contains(&r) => Some(r - STORE_ROW_OFFSET),
            _ => None,
        }
    }

    /// Whether the given screen position is on the scrollbar drawn on the
    /// right edge of the main body. The scrollbar exists only when the
    /// window is too short for the whole body (all CONTENT_HEIGHT rows);
    /// it spans the body rows (between the top bar and the timeline).
    fn scrollbar_hit(&self, column: u16, row: u16) -> bool {
        let area = self.last_area;
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            return false;
        }
        let body_height = area.height.saturating_sub(CHROME_HEIGHT);
        if (body_height as usize) >= CONTENT_HEIGHT {
            return false;
        }
        column == area.right() - 1 && (1..area.height - 2).contains(&row)
    }

    /// Set the scroll so the scrollbar thumb's top lands on the given
    /// screen row, clamped to the track: the top arrow row scrolls to the
    /// top, the bottom one to the end.
    fn scroll_thumb_to_row(&mut self, row: u16) {
        let body_height = self.last_area.height.saturating_sub(CHROME_HEIGHT);
        let (track, thumb, max_scroll) = scrollbar_metrics(body_height);
        if track == 0 || max_scroll == 0 {
            return;
        }
        // The thumb's first screen row is 1 + start (the body starts at
        // row 1 and the scrollbar has no begin arrow), so the track offset
        // is row - 1
        let start = (row as i32 - 1).clamp(0, (track - thumb) as i32) as usize;
        self.store_scroll = scrollbar_scroll_for_thumb_start(body_height, start);
    }

    /// Open the editor pop-up if the given screen row corresponds to CI, A, or a store word.
    ///
    /// The main body starts at row 1 (below the top bar). Content rows:
    /// 0 = blank, 1 = CI, 2 = A, 3 = blank, 4..=35 = store words 0..=31.
    /// The store view is scrolled by `store_scroll` lines.
    fn try_open_editor_at(&mut self, screen_row: u16) {
        // The word editor and the inline comment editor are mutually exclusive
        self.comment_editor.close();
        // Ignore clicks outside the main body or when the terminal is too small
        if self.last_area.width < MIN_WIDTH || self.last_area.height < MIN_HEIGHT {
            return;
        }
        let body_top = 1u16;
        let body_height = self.last_area.height.saturating_sub(CHROME_HEIGHT);
        if screen_row < body_top || screen_row >= body_top + body_height {
            return;
        }

        let content_row =
            (screen_row as usize).saturating_sub(body_top as usize) + self.store_scroll as usize;

        match content_row {
            1 => self.editor_popup.open_for_ci(),
            2 => self.editor_popup.open_for_a(),
            row if (4..=35).contains(&row) => {
                self.editor_popup.open_for_word(row - 4);
            }
            _ => {}
        }
    }

    /// Increase the execution speed.
    fn speed_up(sim: &mut Simulator) {
        let speed = sim.speed;
        sim.speed = if speed < 10 {
            speed + 1
        } else if speed < 100 {
            speed + 10
        } else {
            (speed + 100).min(10_000_000)
        };
    }

    /// Decrease the execution speed.
    fn speed_down(sim: &mut Simulator) {
        let speed = sim.speed;
        sim.speed = if speed <= 10 {
            speed.saturating_sub(1).max(1)
        } else if speed <= 100 {
            speed - 10
        } else {
            speed - 100
        };
    }

    /// Format a 32-bit word as bits in SSEM order (least significant bit first).
    pub(super) fn format_bits(&self, word: i32) -> String {
        let (on, off) = BIT_REPRESENTATIONS[self.bit_representation];
        let mut s = String::with_capacity(32);
        for i in 0..32 {
            s.push(if (word >> i) & 1 == 1 { on } else { off });
        }
        s
    }
}
