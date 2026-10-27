//! Interactive terminal interface for the SSEM simulator, built with ratatui.
//!
//! # Architecture: two threads
//!
//! The simulation and the interface run on separate threads, so that a high
//! target speed cannot starve the UI of CPU time:
//!
//! - **Machine thread**: runs the simulation at the target speed. It locks the
//!   simulator, executes a time-limited batch of cycles, releases the lock, and
//!   sleeps until the batch deadline. The short batch duration (~1 ms) keeps the
//!   lock available for the UI thread.
//! - **UI thread** (main): polls for keyboard input at ~60 fps, takes a state
//!   snapshot under a brief lock, and draws without holding the lock (so terminal
//!   I/O never blocks the machine thread).
//!
//! Shared state lives in an `Arc<Mutex<Simulator>>`; a separate
//! `Arc<AtomicBool>` signals the machine thread to exit.
//!
//! The interface is composed of four areas:
//! - A top bar showing status indicators (status, cycles, last instruction, speed, breakpoints)
//! - A main body showing the machine state (CI, accumulator, and store words)
//! - A timeline bar showing the retained execution history
//! - A bottom bar showing the keyboard shortcuts
//!
//! Two pop-ups are available: a help pop-up (H key) and a breakpoint editor (B key).
//! A third pop-up, the memory/register editor (E key), lets you modify CI, A, or store words.
//!
//! Mouse: double-click a line (CI, A, or a store word) to open the editor for it;
//! scroll wheel scrolls the store view.
//!
//! Time-travel: the timeline bar shows the retained execution history. Drag it
//! with the mouse (or use , . Home End, or the scroll wheel over it) to move
//! through the recorded cycles; the machine pauses and its state is restored to
//! the scrubbed cycle, and a small pop-up above the bar's position marker shows
//! the current cycle. Press P to resume from there — re-executing overwrites
//! the previously recorded future. Browsing the history resets the heatmap and
//! the last-instruction display.
//!
//! Press M to toggle the execution heatmap: a colored bar next to each store
//! word shows how often it has been executed (green = cold, red = hot).

mod popups;

use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};
use ratatui::{Frame, Terminal};

use crate::ssem::Store;
use crate::ssem::history::History;
use crate::ssem::simulator::{Breakpoint, LastInstruction, Simulator};
use popups::{BreakpointPopup, EditorPopup, HelpPopup};

/// Bit representations cycled through with the D key: (on_char, off_char).
const BIT_REPRESENTATIONS: [(char, char); 4] = [
    ('#', '.'),             // High contrast ASCII
    ('\u{2212}', '\u{b7}'), // Fancy characters (− and ·)
    ('_', '.'),             // ASCII compatible
    ('1', '0'),             // Binary digits
];

/// The keymap: (key, legend, help description).
/// An empty legend means the key does not appear in the bottom bar (e.g. arrow keys).
const KEYMAP: &[(&str, &str, &str)] = &[
    ("Q", "QUIT", "Quit the simulator"),
    ("P", "RUN/STOP", "Run the program or pause it"),
    ("F10", "STEP", "Execute one instruction"),
    (",", "BACK", "Step back one cycle in the execution history"),
    (
        ".",
        "FWD",
        "Step forward one cycle in the execution history",
    ),
    ("HOME/END", "", "Jump to the oldest / newest recorded cycle"),
    ("\u{2191}/\u{2193}", "", "Scroll the store view"),
    ("I", "SPEED UP", "Increase the execution speed"),
    ("K", "SPEED DOWN", "Decrease the execution speed"),
    ("D", "DISPLAY", "Cycle the bit representations"),
    (
        "E",
        "EDIT",
        "Edit a register or store word (or double-click a line)",
    ),
    ("M", "HEATMAP", "Toggle the execution heatmap"),
    ("B", "BREAKPT", "Open the breakpoint pop-up"),
    ("H", "HELP", "Toggle this help pop-up"),
    (
        "MOUSE",
        "",
        "Drag the timeline bar to scrub through the history",
    ),
];

/// Minimum terminal width to display the store in full.
const MIN_WIDTH: u16 = 59;

/// Minimum terminal height (top bar + content + timeline + bottom bar).
const MIN_HEIGHT: u16 = 13;

/// Total height of the fixed bars: top bar + timeline + bottom bar.
const CHROME_HEIGHT: u16 = 3;

/// Maximum width of the timeline bar (in cells).
const MAX_TIMELINE_BAR_WIDTH: usize = 100;

/// Column and width of the timeline bar within the timeline area.
fn timeline_bar_geometry(area: Rect, start: u64, head: u64) -> (u16, u16) {
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
fn timeline_marker_offset(
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
const CONTENT_HEIGHT: usize = 36;

/// How often the displayed speed is refreshed.
const SPEED_DISPLAY_PERIOD: Duration = Duration::from_millis(500);

/// Smoothing factor (in seconds) for the speed display.
const SPEED_SMOOTHING: f64 = 0.5;

/// Frame rate target for the UI thread.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Target wall-clock duration for each batch of cycles in the machine thread.
/// Short enough (~1 ms) that the UI thread can acquire the lock between batches.
const BATCH_DURATION: f64 = 0.001;

/// Maximum number of cycles per batch, capping how long the machine thread
/// can hold the lock even at very high target speeds.
const MAX_BATCH_SIZE: u64 = 10_000;

/// How long the machine thread sleeps when the simulation is stopped,
/// to avoid busy-waiting.
const IDLE_SLEEP: Duration = Duration::from_millis(10);

/// Maximum time between two left-clicks for them to count as a double-click.
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(500);

/// Width of the heatmap bar (in cells) shown next to each store word.
const HEATMAP_BAR_WIDTH: usize = 6;

/// Extra width required when the heatmap is shown (bar + leading space).
const HEATMAP_EXTRA_WIDTH: u16 = (HEATMAP_BAR_WIDTH + 2) as u16;

/// A snapshot of the simulator state, taken by the UI thread under a brief lock.
///
/// Drawing uses this snapshot instead of the live simulator so that the
/// (potentially slow) terminal I/O in `terminal.draw()` never holds the lock
/// and never blocks the machine thread.
struct Snapshot {
    ci: i32,
    a: i32,
    store: [i32; 32],
    cycle_count: u64,
    speed: u32,
    stop_flag: bool,
    last_instruction: Option<LastInstruction>,
    breakpoints: Vec<Breakpoint>,
    exec_counts: [u64; 32],
    history_capacity: u64,
    history_start: u64,
    history_head: u64,
    history_len: u64,
}

impl Snapshot {
    fn from_sim(sim: &Simulator) -> Self {
        let (history_capacity, history_start, history_head, history_len) = match &sim.history {
            Some(h) => (h.capacity(), h.start(), h.head(), h.len()),
            None => (0, 0, 0, 0),
        };
        Snapshot {
            ci: sim.ci,
            a: sim.a,
            store: sim.store.words,
            cycle_count: sim.cycle_count,
            speed: sim.speed,
            stop_flag: sim.stop_flag,
            last_instruction: sim.last_instruction,
            breakpoints: sim.breakpoints.clone(),
            exec_counts: sim.exec_counts,
            history_capacity,
            history_start,
            history_head,
            history_len,
        }
    }
}

/// The application state for the TUI (UI thread only).
struct App {
    bit_representation: usize,
    store_scroll: u16,
    last_area: Rect,

    // Speed display: exponentially smoothed
    current_speed: f64,
    displayed_speed: u32,
    last_speed_refresh: Instant,
    last_refresh_time: Instant,
    last_cycles: u64,

    help_popup: HelpPopup,
    breakpoint_popup: BreakpointPopup,
    editor_popup: EditorPopup,

    /// The last left-click, for double-click detection: (time, column, row).
    last_click: Option<(Instant, u16, u16)>,

    /// Whether the execution heatmap is shown in the store view.
    show_heatmap: bool,

    /// Whether the user is currently dragging the timeline scrubber.
    scrubbing: bool,

    /// Screen area of the timeline bar, for mouse hit-testing.
    timeline_area: Rect,

    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let now = Instant::now();
        App {
            bit_representation: 0,
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
            last_click: None,
            show_heatmap: false,
            scrubbing: false,
            timeline_area: Rect::default(),
            should_quit: false,
        }
    }

    /// Update the smoothed speed display value from the latest cycle count.
    fn update_speed_display(&mut self, is_running: bool, cycle_count: u64) {
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
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Holding a scrub key repeats it, for continuous time-travel.
        // Pop-ups keep priority over the scrub keys.
        if key.kind == KeyEventKind::Repeat {
            if !self.breakpoint_popup.is_open()
                && !self.editor_popup.is_open()
                && !self.help_popup.is_open()
            {
                self.handle_scrub_key(key.code, sim);
            }
            return;
        }

        // Only handle key press events (ignore release on platforms that report them)
        if key.kind != KeyEventKind::Press {
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

        // While the help pop-up is open, only H or Esc closes it
        if self.help_popup.is_open() {
            if key.code == KeyCode::Char('h')
                || key.code == KeyCode::Char('H')
                || key.code == KeyCode::Esc
            {
                self.help_popup.close();
            }
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
                'e' => self.editor_popup.open(),
                'm' => self.show_heatmap = !self.show_heatmap,
                'h' => self.help_popup.open(),
                ',' | '.' => self.handle_scrub_key(key.code, sim),
                _ => {}
            },
            KeyCode::F(10) => {
                // Step: stop the machine and execute one instruction
                sim.stop_flag = true;
                sim.instruction_cycle();
                sim.check_breakpoints();
            }
            KeyCode::Home | KeyCode::End => self.handle_scrub_key(key.code, sim),
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

    /// Handle a history-scrubbing key: `,` `.` Home End.
    fn handle_scrub_key(&mut self, code: KeyCode, sim: &mut Simulator) {
        match code {
            KeyCode::Char(',') => self.scrub_relative(-1, sim),
            KeyCode::Char('.') => self.scrub_relative(1, sim),
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
    fn scrub_relative(&mut self, delta: i64, sim: &mut Simulator) {
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
    fn scrub_to_column(&mut self, column: u16, sim: &mut Simulator) {
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
    fn handle_mouse(&mut self, mouse: MouseEvent, sim: &mut Simulator) {
        // Releasing the left button ends a scrub, wherever the cursor is
        if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
            self.scrubbing = false;
            return;
        }

        // While scrubbing, drags anywhere on screen move the cursor
        // (scrub_to_column clamps the column to the bar)
        if self.scrubbing && matches!(mouse.kind, MouseEventKind::Drag(MouseButton::Left)) {
            self.scrub_to_column(mouse.column, sim);
            return;
        }

        // The timeline row: click to scrub, scroll wheel to step
        if self.timeline_area.height > 0 && mouse.row == self.timeline_area.y {
            match mouse.kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // Scrubbing requires a non-empty history
                    if sim.history.as_ref().is_some_and(|h| !h.is_empty()) {
                        self.scrubbing = true;
                        self.scrub_to_column(mouse.column, sim);
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
                    self.try_open_editor_at(mouse.row);
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

    /// Open the editor pop-up if the given screen row corresponds to CI, A, or a store word.
    ///
    /// The main body starts at row 1 (below the top bar). Content rows:
    /// 0 = blank, 1 = CI, 2 = A, 3 = blank, 4..=35 = store words 0..=31.
    /// The store view is scrolled by `store_scroll` lines.
    fn try_open_editor_at(&mut self, screen_row: u16) {
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
    fn format_bits(&self, word: i32) -> String {
        let (on, off) = BIT_REPRESENTATIONS[self.bit_representation];
        let mut s = String::with_capacity(32);
        for i in 0..32 {
            s.push(if (word >> i) & 1 == 1 { on } else { off });
        }
        s
    }

    /// Compute the heatmap bar and its color for the store word at `index`.
    ///
    /// Returns `(bar_string, color)`. The bar is `HEATMAP_BAR_WIDTH` cells wide,
    /// filled proportionally to the execution count relative to the hottest line.
    /// Cold lines (zero executions) are dim; hot lines are red.
    fn heatmap_bar(&self, index: usize, exec_counts: &[u64; 32]) -> (String, Color) {
        let count = exec_counts[index];
        if count == 0 {
            return ("\u{b7}".repeat(HEATMAP_BAR_WIDTH), Color::DarkGray);
        }

        let max = *exec_counts.iter().max().unwrap_or(&1);
        let ratio = count as f64 / max as f64;
        let filled =
            ((ratio * HEATMAP_BAR_WIDTH as f64).ceil() as usize).clamp(1, HEATMAP_BAR_WIDTH);

        let bar = format!(
            "{}{}",
            "\u{2588}".repeat(filled),
            "\u{b7}".repeat(HEATMAP_BAR_WIDTH - filled),
        );

        let color = if ratio > 0.75 {
            Color::Red
        } else if ratio > 0.25 {
            Color::Yellow
        } else {
            Color::Green
        };

        (bar, color)
    }

    /// Draw the entire interface from a state snapshot.
    fn draw(&mut self, f: &mut Frame, snap: &Snapshot) {
        let area = f.area();
        self.last_area = area;

        // Clamp scroll to keep the content within the visible area
        let visible_height = area.height.saturating_sub(CHROME_HEIGHT) as usize;
        let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
        self.store_scroll = (self.store_scroll as usize).min(max_scroll) as u16;

        // Check minimum terminal size (wider when the heatmap is shown)
        let min_width = if self.show_heatmap {
            MIN_WIDTH + HEATMAP_EXTRA_WIDTH
        } else {
            MIN_WIDTH
        };
        if area.width < min_width || area.height < MIN_HEIGHT {
            let msg = format!(
                "Terminal too small: {}x{}, needs at least {}x{}",
                area.width, area.height, min_width, MIN_HEIGHT
            );
            let paragraph = Paragraph::new(msg);
            f.render_widget(paragraph, area);
            self.timeline_area = Rect::default();
            return;
        }

        // Layout: top bar (1 line), main body (flexible), timeline (1 line), bottom bar (1 line)
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        self.timeline_area = chunks[2];
        self.draw_top_bar(f, chunks[0], snap);
        self.draw_main_body(f, chunks[1], snap);
        self.draw_timeline(f, chunks[2], snap);
        self.draw_bottom_bar(f, chunks[3]);

        // Draw pop-ups on top
        if self.help_popup.is_open() {
            self.help_popup.draw(f, area);
        }
        if self.breakpoint_popup.is_open() {
            self.breakpoint_popup.draw(f, area, &snap.breakpoints);
        }
        if self.editor_popup.is_open() {
            self.editor_popup
                .draw(f, area, snap.ci, snap.a, &snap.store);
        }

        // While scrubbing the timeline, show the current cycle above the cursor
        if self.scrubbing {
            self.draw_scrub_popup(f, area, snap);
        }
    }

    /// Draw the top bar with status indicators.
    fn draw_top_bar(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let status = if self.scrubbing {
            "REWIND"
        } else if snap.stop_flag {
            "STOPPED"
        } else {
            "RUNNING"
        };

        let last_instr = snap
            .last_instruction
            .map(|li| li.to_string())
            .unwrap_or_else(|| "-- -- --".to_string());

        let mut text = format!(
            "[ STATUS: {status} ]  [ CYCLES: {} ]  [ {last_instr} ]  [ SPEED: {:2}/{} ips ] ",
            snap.cycle_count, self.displayed_speed, snap.speed
        );

        if !snap.breakpoints.is_empty() {
            text.push_str(&format!(" [ BREAKPOINTS: {} ] ", snap.breakpoints.len()));
        }

        if self.show_heatmap {
            text.push_str(" [ HEATMAP ] ");
        }

        let line = Line::from(Span::styled(text, Style::default().fg(Color::Blue)));
        f.render_widget(Paragraph::new(line), area);
    }

    /// Draw the main body: CI, accumulator, and store words.
    fn draw_main_body(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(CONTENT_HEIGHT);

        // Spacer to align CI/A bits with store-word bits when the heatmap is on
        let heatmap_spacer = if self.show_heatmap {
            " ".repeat(HEATMAP_EXTRA_WIDTH as usize)
        } else {
            String::new()
        };

        // Row 0: blank
        lines.push(Line::from(""));

        // Row 1: CI
        let ci_bits = self.format_bits(snap.ci);
        lines.push(Line::from(vec![
            Span::styled(
                format!("{heatmap_spacer} {ci_bits}"),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" CI: {:>11}", snap.ci)),
        ]));

        // Row 2: Accumulator
        let a_bits = self.format_bits(snap.a);
        lines.push(Line::from(vec![
            Span::styled(
                format!("{heatmap_spacer} {a_bits}"),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" A : {:>11}", snap.a)),
        ]));

        // Row 3: blank
        lines.push(Line::from(""));

        // Rows 4-35: store words
        for (index, word) in snap.store.iter().enumerate() {
            let is_ci = index as i32 == snap.ci;
            let pointer = if is_ci { ">" } else { " " };
            let bits = self.format_bits(*word);
            let (opcode, data) = Store::decode_word(*word);

            let bits_style = if is_ci {
                Style::default().add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            };

            // Heatmap bar (empty span when the heatmap is off)
            let (bar, bar_color) = if self.show_heatmap {
                let (b, c) = self.heatmap_bar(index, &snap.exec_counts);
                (format!(" {b} "), c)
            } else {
                (String::new(), Color::Reset)
            };

            lines.push(Line::from(vec![
                Span::styled(
                    pointer.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(bar, Style::default().fg(bar_color)),
                Span::styled(bits, bits_style),
                Span::raw(format!(
                    " {:02}: {:>11}  {:<4} {:02}",
                    index,
                    word,
                    opcode.to_string(),
                    data
                )),
            ]));
        }

        let paragraph = Paragraph::new(lines).scroll((self.store_scroll, 0));
        f.render_widget(paragraph, area);
    }

    /// Draw the timeline bar: how much history is retained (filled bar) and
    /// where the machine currently is (marker). Dragging it scrubs the history.
    fn draw_timeline(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        if snap.history_capacity == 0 {
            let line = Line::from(Span::styled(
                " HISTORY [disabled]",
                Style::default().fg(Color::DarkGray),
            ));
            f.render_widget(Paragraph::new(line), area);
            return;
        }

        let (_, bar_width) = timeline_bar_geometry(area, snap.history_start, snap.history_head);

        // Filled portion = fraction of the capacity still retained
        let filled = (snap.history_len * u64::from(bar_width) / snap.history_capacity) as usize;

        // Marker at the current position (the tip, or the scrub position)
        let marker = timeline_marker_offset(
            bar_width,
            snap.history_start,
            snap.history_head,
            snap.cycle_count,
        );

        let blue = Style::default().fg(Color::Blue);
        let mut spans = vec![Span::styled(" HISTORY [".to_string(), blue)];
        if bar_width > 0 {
            let segment = |from: usize, to: usize| -> String {
                (from..to)
                    .map(|i| if i < filled { '\u{2588}' } else { '\u{b7}' })
                    .collect()
            };
            let marker = marker.unwrap_or(0);
            spans.push(Span::styled(segment(0, marker), blue));
            spans.push(Span::styled(
                ">".to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                segment(marker + 1, usize::from(bar_width)),
                blue,
            ));
        }
        spans.push(Span::styled(
            format!("] {}..{}", snap.history_start, snap.history_head),
            blue,
        ));

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Draw a small pop-up above the timeline cursor, showing the current cycle.
    fn draw_scrub_popup(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        if self.timeline_area.height == 0 {
            return;
        }

        // Center the pop-up on the bar's position marker, so it stays above
        // the marker even when the mouse is dragged past the bar's ends
        // (only the screen edges clamp it).
        let (bar_x, bar_width) =
            timeline_bar_geometry(self.timeline_area, snap.history_start, snap.history_head);
        let Some(offset) = timeline_marker_offset(
            bar_width,
            snap.history_start,
            snap.history_head,
            snap.cycle_count,
        ) else {
            return;
        };
        let center = bar_x + offset as u16;

        let text = format!(" cycle {} ", snap.cycle_count);
        let width = text.len() as u16;
        let x = center
            .saturating_sub(width / 2)
            .clamp(area.x, area.x + area.width.saturating_sub(width));
        let popup_area = Rect {
            x,
            y: self.timeline_area.y.saturating_sub(1),
            width,
            height: 1,
        };

        f.render_widget(Clear, popup_area);
        f.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::Black).bg(Color::Yellow)),
            popup_area,
        );
    }

    /// Draw the bottom bar with keyboard shortcuts.
    fn draw_bottom_bar(&self, f: &mut Frame, area: Rect) {
        let key_style = Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::REVERSED);
        let normal_style = Style::default();

        let mut spans: Vec<Span<'static>> = Vec::new();
        for (key, legend, _) in KEYMAP {
            if legend.is_empty() {
                continue;
            }
            let key_text = if key.chars().count() == 1 {
                format!(" {key} ")
            } else {
                (*key).to_string()
            };
            spans.push(Span::styled(key_text, key_style));
            spans.push(Span::styled(format!(" {legend}  "), normal_style));
        }

        let line = Line::from(spans);
        f.render_widget(Paragraph::new(line), area);
    }
}

/// The machine thread: runs the simulation at the target speed.
///
/// Executes cycles in time-limited batches (~1 ms), releasing the lock between
/// batches so the UI thread can acquire it. Sleeps until the batch deadline to
/// maintain the target speed. Uses absolute deadlines so that `sleep()` overshoot
/// does not accumulate over time.
fn machine_thread(sim: Arc<Mutex<Simulator>>, quit: Arc<AtomicBool>, max_cycles: u64) {
    let mut next_deadline = Instant::now();
    let mut last_speed: u32 = 0;

    loop {
        if quit.load(Ordering::Relaxed) {
            break;
        }

        // Read the control state under a brief lock
        let (should_run, speed) = {
            let s = sim.lock().unwrap();
            (!s.stop_flag, s.speed)
        };

        if !should_run {
            next_deadline = Instant::now();
            last_speed = 0;
            thread::sleep(IDLE_SLEEP);
            continue;
        }

        // Resynchronize when the speed changes
        if speed != last_speed {
            next_deadline = Instant::now();
            last_speed = speed;
        }

        // Calculate batch size: enough cycles to fill ~1 ms at the target speed
        let speed = speed.max(1);
        let period = 1.0 / speed as f64;
        let batch_size = ((BATCH_DURATION / period) as u64).clamp(1, MAX_BATCH_SIZE);
        let deadline = next_deadline + Duration::from_secs_f64(batch_size as f64 * period);

        // Execute the batch (lock held for the duration of the batch)
        {
            let mut s = sim.lock().unwrap();
            for _ in 0..batch_size {
                if s.stop_flag {
                    break;
                }
                if s.cycle_count >= max_cycles {
                    s.stop_flag = true;
                    break;
                }
                s.instruction_cycle();
                s.check_breakpoints();
            }
        }

        // Sleep until the absolute deadline, so sleep() overshoot does not
        // accumulate. If execution fell behind schedule (e.g. speed set beyond
        // what the machine can deliver), resynchronize instead of catching up
        // in a burst.
        let now = Instant::now();
        if deadline > now {
            thread::sleep(deadline - now);
            next_deadline = deadline;
        } else {
            next_deadline = now + Duration::from_secs_f64(batch_size as f64 * period);
        }
    }
}

/// Run the interactive TUI.
///
/// Takes ownership of the simulator and returns it when the user quits,
/// so the caller can print the final state.
pub fn run(simulator: Simulator, max_cycles: u32) -> io::Result<Simulator> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let result = run_app(&mut terminal, simulator, max_cycles);

    // Always restore the terminal, even on error
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    simulator: Simulator,
    max_cycles: u32,
) -> io::Result<Simulator> {
    // Shared state between the machine and UI threads
    let sim = Arc::new(Mutex::new(simulator));
    let quit = Arc::new(AtomicBool::new(false));

    // Start in the stopped state; the user presses P to run
    sim.lock().unwrap().stop_flag = true;

    // Spawn the machine thread
    let machine_sim = Arc::clone(&sim);
    let machine_quit = Arc::clone(&quit);
    let machine_handle = thread::spawn(move || {
        machine_thread(machine_sim, machine_quit, max_cycles as u64);
    });

    // UI thread (main thread)
    let mut app = App::new();

    loop {
        // Handle input (non-blocking with a short timeout)
        if event::poll(FRAME_INTERVAL)? {
            match event::read()? {
                Event::Key(key) => {
                    let mut s = sim.lock().unwrap();
                    app.handle_key(key, &mut s);
                }
                Event::Mouse(mouse) => {
                    let mut s = sim.lock().unwrap();
                    app.handle_mouse(mouse, &mut s);
                }
                _ => {}
            }
        }

        // Take a snapshot under a brief lock, then draw without holding it.
        // This keeps the lock available for the machine thread during terminal I/O.
        let snapshot = {
            let s = sim.lock().unwrap();
            app.update_speed_display(!s.stop_flag, s.cycle_count);
            Snapshot::from_sim(&s)
        };

        terminal.draw(|f| app.draw(f, &snapshot))?;

        if app.should_quit {
            quit.store(true, Ordering::Relaxed);
            break;
        }
    }

    // Signal the machine thread to exit and wait for it
    quit.store(true, Ordering::Relaxed);
    machine_handle.join().unwrap();

    // Recover the simulator from the Arc<Mutex<>>
    let sim = Arc::try_unwrap(sim)
        .ok()
        .expect("machine thread should have released its Arc reference")
        .into_inner()
        .unwrap_or_else(|e| e.into_inner());
    Ok(sim)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    /// A simulator that never halts: an empty store executes `JMP 0` forever.
    fn looping_sim(cycles: u32) -> Simulator {
        let mut sim = Simulator::new();
        sim.enable_history(100);
        sim.run(cycles);
        sim
    }

    /// A snapshot with history enabled, as the UI would see it while paused.
    fn snapshot_with_history(cycle_count: u64) -> Snapshot {
        Snapshot {
            ci: 0,
            a: 0,
            store: [0; 32],
            cycle_count,
            speed: 700,
            stop_flag: true,
            last_instruction: None,
            breakpoints: Vec::new(),
            exec_counts: [0; 32],
            history_capacity: 100,
            history_start: 0,
            history_head: 99,
            history_len: 100,
        }
    }

    /// Render the app into a test terminal and return the resulting buffer.
    fn render(app: &mut App, snap: &Snapshot, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| app.draw(f, snap)).unwrap();
        terminal.backend().buffer().clone()
    }

    /// The text of one screen row, one character per cell.
    fn row_text(buffer: &Buffer, row: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, row)].symbol())
            .collect()
    }

    #[test]
    fn timeline_column_maps_to_cycle() {
        let mut sim = looping_sim(10);
        let mut app = App::new();
        app.timeline_area = Rect::new(0, 10, 80, 1);

        let (bar_x, bar_width) = timeline_bar_geometry(app.timeline_area, 0, 10);
        assert_eq!(10, bar_x);
        assert!(bar_width > 1);

        // Far right of the bar: the tip
        app.scrub_to_column(bar_x + bar_width - 1, &mut sim);
        assert_eq!(10, sim.cycle_count);

        // Left edge: the oldest retained cycle
        app.scrub_to_column(bar_x, &mut sim);
        assert_eq!(0, sim.cycle_count);

        // Middle
        app.scrub_to_column(bar_x + bar_width / 2, &mut sim);
        assert_eq!(5, sim.cycle_count);

        // Columns past the bar's ends clamp to the retained range,
        // so the marker (and the pop-up centered on it) stays on the bar
        app.scrub_to_column(bar_x + bar_width + 50, &mut sim);
        assert_eq!(10, sim.cycle_count);
        app.scrub_to_column(0, &mut sim);
        assert_eq!(0, sim.cycle_count);
        app.scrub_to_column(bar_x.saturating_sub(50), &mut sim);
        assert_eq!(0, sim.cycle_count);
    }

    #[test]
    fn scrub_relative_clamps_to_history_range() {
        let mut sim = looping_sim(10);
        let mut app = App::new();

        app.scrub_relative(-1000, &mut sim);
        assert_eq!(0, sim.cycle_count);
        assert!(sim.stop_flag, "scrubbing pauses the machine");

        app.scrub_relative(1000, &mut sim);
        assert_eq!(10, sim.cycle_count);

        // At the tip, stepping forward is a no-op and does not pause
        sim.stop_flag = false;
        app.scrub_relative(1, &mut sim);
        assert_eq!(10, sim.cycle_count);
        assert!(!sim.stop_flag);
    }

    #[test]
    fn scrub_keys_step_through_history() {
        let mut sim = looping_sim(10);
        let mut app = App::new();

        app.handle_scrub_key(KeyCode::Char(','), &mut sim);
        assert_eq!(9, sim.cycle_count);
        app.handle_scrub_key(KeyCode::Char('.'), &mut sim);
        assert_eq!(10, sim.cycle_count);
        app.handle_scrub_key(KeyCode::Home, &mut sim);
        assert_eq!(0, sim.cycle_count);
        app.handle_scrub_key(KeyCode::End, &mut sim);
        assert_eq!(10, sim.cycle_count);
    }

    #[test]
    fn scrub_without_history_is_noop() {
        let mut sim = Simulator::new();
        sim.run(10);
        let mut app = App::new();

        app.handle_scrub_key(KeyCode::Char(','), &mut sim);
        assert_eq!(10, sim.cycle_count);
        assert!(!sim.stop_flag);
    }

    #[test]
    fn scrub_popup_centers_on_the_bar_marker() {
        // On an 80-column terminal the bar spans x=10, width 63
        let timeline = Rect::new(0, 18, 80, 1);
        let (bar_x, bar_width) = timeline_bar_geometry(timeline, 0, 99);
        assert_eq!(10, bar_x);
        assert_eq!(63, bar_width);

        for cycle_count in [0, 42, 99] {
            let mut app = App::new();
            app.scrubbing = true;
            let snap = snapshot_with_history(cycle_count);
            let buffer = render(&mut app, &snap, 80, 20);

            // The timeline row is 18 on a 20-row terminal; the pop-up sits on row 17
            let row = row_text(&buffer, 17);
            let text = format!(" cycle {cycle_count} ");
            let pos = row
                .find(&text)
                .unwrap_or_else(|| panic!("pop-up missing at cycle {cycle_count}, row: {row:?}"));
            let center = pos as u16 + text.len() as u16 / 2;
            let marker =
                bar_x + timeline_marker_offset(bar_width, 0, 99, cycle_count).unwrap() as u16;
            assert_eq!(
                marker, center,
                "pop-up should center on the marker at cycle {cycle_count}"
            );
        }
    }

    #[test]
    fn scrub_popup_hidden_when_not_scrubbing() {
        let mut app = App::new();
        let snap = snapshot_with_history(42);
        let buffer = render(&mut app, &snap, 80, 20);

        let all: String = (0..buffer.area.height)
            .map(|y| row_text(&buffer, y))
            .collect();
        assert!(!all.contains(" cycle 42 "), "no pop-up when not scrubbing");
    }

    #[test]
    fn scrub_popup_stays_on_screen_at_the_bar_ends() {
        // Narrowest supported terminal, pop-up at both ends of the bar
        for cycle_count in [0, 99] {
            let mut app = App::new();
            app.scrubbing = true;
            let snap = snapshot_with_history(cycle_count);
            let buffer = render(&mut app, &snap, MIN_WIDTH, 20);

            let row = row_text(&buffer, 17);
            let text = format!(" cycle {cycle_count} ");
            let pos = row
                .find(&text)
                .unwrap_or_else(|| panic!("pop-up missing at cycle {cycle_count}, row: {row:?}"));
            assert!(
                pos + text.len() <= buffer.area.width as usize,
                "pop-up must stay on screen at cycle {cycle_count}"
            );
        }
    }
}
