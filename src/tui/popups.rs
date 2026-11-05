//! Pop-up windows for the TUI: help, breakpoint editor, word editor, and save.

use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};

use crate::ssem::Opcode;
use crate::ssem::Store;
use crate::ssem::simulator::{Breakpoint, Simulator};

use super::app::{BIT_REPRESENTATIONS, KEYMAP};
use super::layout::{FlowMode, HeatmapMode, MAX_COMMENT_WIDTH};
use super::palette::{Palette, Theme};

/// Compute a centered, clamped pop-up rectangle within the given area.
fn popup_rect(height: u16, width: u16, area: Rect) -> Rect {
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
/// to `1 2 3 4 5 6 7 8 9 0`. With `path_chars`, `;`/`:` + Shift also map to
/// `.`/`/` so file paths can be typed in the save pop-up.
fn shifted_char_of(key: &KeyEvent, path_chars: bool) -> Option<char> {
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
        ':' if path_chars => '/',
        _ => return None,
    })
}

/// The character a key event represents when it is a digit, if any
/// (AZERTY top row included), plus the `.`/`/` path characters for the
/// save pop-up.
fn digit_of(key: &KeyEvent) -> Option<char> {
    shifted_char_of(key, true)
}

/// The digit a key event represents, if any, for free-text fields: the
/// AZERTY top row is translated, but not the `;`/`:` path characters,
/// which keep their literal meaning in a comment.
fn azerty_digit_of(key: &KeyEvent) -> Option<char> {
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

/// Display width of a string in terminal columns.
///
/// Uses character count, which is correct for the characters used in the UI
/// (no double-width CJK or emoji). `.len()` would return byte count and
/// overestimate strings containing multi-byte characters like `·` or `↑`.
fn display_width(s: &str) -> usize {
    s.chars().count()
}

/// The help pop-up, listing all keyboard shortcuts.
///
/// When the terminal is too small to show the whole text, the body scrolls
/// with the arrow keys; the footer stays pinned to the bottom row.
/// H or Esc closes it, as does a mouse click outside the pop-up.
pub struct HelpPopup {
    open: bool,
    scroll: usize,
    /// Maximum scroll offset, refreshed on every draw so key handling can clamp.
    max_scroll: usize,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    last_rect: Rect,
}

impl HelpPopup {
    pub fn new() -> Self {
        HelpPopup {
            open: false,
            scroll: 0,
            max_scroll: 0,
            last_rect: Rect::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.scroll = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Scroll by `delta` lines (negative goes up), clamped to the content.
    pub fn scroll_by(&mut self, delta: i32) {
        self.scroll = (self.scroll as i64 + delta as i64).clamp(0, self.max_scroll as i64) as usize;
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// Handle a key press while the pop-up is open: arrows scroll the text.
    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        match key.code {
            KeyCode::Up => self.scroll_by(-1),
            KeyCode::Down => self.scroll_by(1),
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = self.max_scroll,
            _ => {}
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, palette: &Palette) {
        let key_column = KEYMAP
            .iter()
            .map(|(key, _, _)| key.chars().count())
            .max()
            .unwrap_or(0)
            + 2;

        let title = format!("SSEM SIMULATOR v{}", env!("CARGO_PKG_VERSION"));
        let footer = "Press H to close";
        let footer_scrollable = "\u{2191}/\u{2193} scroll \u{b7} H close";

        let key_style = Style::default()
            .fg(palette.key_hint)
            .add_modifier(Modifier::REVERSED);

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Blank line after the border title
        lines.push(Line::from(""));

        // One line per key
        for (key, _, description) in KEYMAP {
            let key_width = key.chars().count();
            let padding = key_column.saturating_sub(key_width);
            let left_pad = padding / 2;
            let right_pad = padding - left_pad;
            let key_text = format!("{}{}{}", " ".repeat(left_pad), key, " ".repeat(right_pad));

            lines.push(Line::from(vec![
                Span::styled(key_text, key_style),
                Span::raw(format!(" {description}")),
            ]));
        }

        // Color legend, drawn from the palette so it cannot drift out of sync
        lines.push(Line::from(""));
        lines.push(Line::from("Mnemonic colors:"));
        for (mnemonics, opcode, description) in [
            ("JMP JRP", Opcode::JMP, "jump"),
            ("LDN STO", Opcode::LDN, "memory access"),
            ("SUB SUB2", Opcode::SUB, "arithmetic"),
            ("CMP", Opcode::CMP, "test"),
            ("STP", Opcode::STP, "halt"),
        ] {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{mnemonics:<13}"),
                    Style::default()
                        .fg(palette.opcode(opcode))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(description),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Markers when paused:"));
        lines.push(Line::from(vec![
            Span::raw("  > last  "),
            Span::styled(
                "\u{bb}",
                Style::default()
                    .fg(palette.next_marker)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" next  "),
            Span::styled("\u{2022}", Style::default().fg(palette.dim)),
            Span::raw(" executed"),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from("Heatmap:"));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "EXEC",
                Style::default()
                    .fg(palette.exec_ramp[2])
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" execution word frequency"),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "R",
                Style::default()
                    .fg(palette.read_ramp[1])
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "W",
                Style::default()
                    .fg(palette.write_ramp[2])
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  store reads vs writes per word"),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from("Control-flow graph:"));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "\u{25c0}\u{2500}\u{2500} jump",
                Style::default().fg(palette.edge_jump),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "\u{25c0}\u{2500}\u{2500} CMP skip",
                Style::default().fg(palette.edge_skip),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled("■   halt", Style::default().fg(palette.op_halt)),
        ]));

        // Blank line before the footer, which is pinned to the bottom row
        lines.push(Line::from(""));

        let content_width = display_width(&title)
            .max(display_width(footer))
            .max(display_width(footer_scrollable))
            .max(lines.iter().map(Line::width).max().unwrap_or(0));

        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width + 4) as u16;
        // 1 footer row + 2 border rows around the content
        let height = (lines.len() + 3) as u16;

        let rect = popup_rect(height, width, area);
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(format!(" {title} "))
            .border_type(BorderType::Rounded);
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        // The footer takes the bottom content row; the body scrolls above it.
        let body_height = content.height.saturating_sub(1) as usize;
        let max_scroll = lines.len().saturating_sub(body_height);
        self.max_scroll = max_scroll;
        self.scroll = self.scroll.min(max_scroll);

        let body_area = Rect {
            x: content.x,
            y: content.y,
            width: content.width,
            height: body_height as u16,
        };
        let footer_area = Rect {
            x: content.x,
            y: content.y + body_height as u16,
            width: content.width,
            height: content.height - body_height as u16,
        };

        f.render_widget(
            Paragraph::new(lines).scroll((self.scroll as u16, 0)),
            body_area,
        );

        let footer = if max_scroll > 0 {
            footer_scrollable
        } else {
            footer
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                footer.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            ))),
            footer_area,
        );
    }
}

/// The condition type of a new breakpoint being edited.
#[derive(Clone, Copy, PartialEq)]
enum BreakpointKind {
    /// Pause when a store word holds a given value.
    Word,
    /// Pause when the program counter holds a given address.
    Ci,
}

/// Which field of the breakpoint input is being typed.
#[derive(Clone, Copy, PartialEq)]
enum InputField {
    Address,
    Value,
}

/// The breakpoint editor pop-up.
///
/// Lists existing breakpoints and provides an input line to add new ones.
/// While open, all key events are routed here by the main event loop.
pub struct BreakpointPopup {
    open: bool,
    kind: BreakpointKind,
    field: InputField,
    buffer: String,
    address: usize,
    selected: usize,
    message: String,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    last_rect: Rect,
}

impl BreakpointPopup {
    pub fn new() -> Self {
        BreakpointPopup {
            open: false,
            kind: BreakpointKind::Word,
            field: InputField::Address,
            buffer: String::new(),
            address: 0,
            selected: 0,
            message: String::new(),
            last_rect: Rect::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the pop-up, resetting the editing state.
    pub fn open(&mut self) {
        self.open = true;
        self.kind = BreakpointKind::Word;
        self.field = InputField::Address;
        self.buffer.clear();
        self.message.clear();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// Handle a key press while the pop-up is open.
    pub fn handle_key(&mut self, key: KeyEvent, breakpoints: &mut Vec<Breakpoint>) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                self.message.clear();
            }
            KeyCode::Enter => {
                self.validate_field(breakpoints);
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down => {
                if !breakpoints.is_empty() {
                    self.selected = (self.selected + 1).min(breakpoints.len() - 1);
                }
            }
            KeyCode::Delete => {
                self.delete_selected(breakpoints);
            }
            KeyCode::Char(c) => {
                // A digit, possibly typed with Shift on a French AZERTY
                // keyboard (checked first so Shift+"-" is the digit 6,
                // not the minus sign)
                if let Some(d) = digit_of(&key) {
                    if self.buffer.len() < 11 {
                        self.buffer.push(d);
                    }
                    self.message.clear();
                } else {
                    match c {
                        't' | 'T' => {
                            // Cycle the condition type
                            self.kind = match self.kind {
                                BreakpointKind::Word => BreakpointKind::Ci,
                                BreakpointKind::Ci => BreakpointKind::Word,
                            };
                            self.field = InputField::Address;
                            self.buffer.clear();
                            self.message.clear();
                        }
                        'x' | 'X' => self.delete_selected(breakpoints),
                        '-' if self.field == InputField::Value && self.buffer.is_empty() => {
                            self.buffer.push('-');
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    /// Validate the current input field: move to the next one, or add the breakpoint.
    fn validate_field(&mut self, breakpoints: &mut Vec<Breakpoint>) {
        let buffer = std::mem::take(&mut self.buffer);

        let number: i32 = match buffer.parse() {
            Ok(n) => n,
            Err(_) => {
                self.message = "TYPE A NUMBER".to_string();
                return;
            }
        };

        if self.field == InputField::Address {
            if !(0..32).contains(&number) {
                self.message = "ADDRESS MUST BE 0 TO 31".to_string();
                return;
            }
            self.address = number as usize;

            // CI condition only takes an address
            if self.kind == BreakpointKind::Ci {
                let bp = Breakpoint::CiValue {
                    address: self.address as i32,
                    was_met: false,
                };
                self.add(bp, breakpoints);
                return;
            }

            self.field = InputField::Value;
            return;
        }

        // Value field: add a WordValue breakpoint
        let bp = Breakpoint::WordValue {
            address: self.address,
            value: number,
            was_met: false,
        };
        self.add(bp, breakpoints);
    }

    /// Add a breakpoint unless an identical one already exists.
    fn add(&mut self, bp: Breakpoint, breakpoints: &mut Vec<Breakpoint>) {
        if breakpoints.contains(&bp) {
            self.message = "BREAKPOINT ALREADY SET".to_string();
        } else {
            breakpoints.push(bp);
            if !breakpoints.is_empty() {
                self.selected = self.selected.min(breakpoints.len() - 1);
            }
            self.message.clear();
        }
        self.field = InputField::Address;
    }

    /// Delete the selected breakpoint.
    fn delete_selected(&mut self, breakpoints: &mut Vec<Breakpoint>) {
        if !breakpoints.is_empty() && self.selected < breakpoints.len() {
            breakpoints.remove(self.selected);
            if !breakpoints.is_empty() {
                self.selected = self.selected.min(breakpoints.len() - 1);
            } else {
                self.selected = 0;
            }
            self.message.clear();
        }
    }

    /// The input line showing the current editing state.
    fn input_line(&self) -> String {
        match self.kind {
            BreakpointKind::Ci => {
                format!("NEW: CI: {}_", self.buffer)
            }
            BreakpointKind::Word => match self.field {
                InputField::Address => {
                    format!("NEW: WORD: {}_  VALUE:", self.buffer)
                }
                InputField::Value => {
                    format!("NEW: WORD: {:02}  VALUE: {}_", self.address, self.buffer)
                }
            },
        }
    }

    pub fn draw(
        &mut self,
        f: &mut Frame,
        area: Rect,
        breakpoints: &[Breakpoint],
        palette: &Palette,
    ) {
        let explanation = "Pause the machine when a condition is met";
        let footer = "ENTER add \u{b7} T type \u{b7} \u{2191}/\u{2193} select \u{b7} X delete \u{b7} ESC close";

        let input = self.input_line();
        let descriptions: Vec<String> = breakpoints.iter().map(|bp| bp.description()).collect();

        let content_width = display_width(explanation)
            .max(display_width(footer))
            .max(display_width(&input))
            .max(
                descriptions
                    .iter()
                    .map(|d| display_width(d))
                    .max()
                    .unwrap_or(0),
            );

        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width + 4) as u16;
        // Title is in the border. Interior rows: explanation, blank, N breakpoints,
        // blank, input, message, footer = N + 6. Plus 2 border rows.
        let height = (breakpoints.len() + 8) as u16;

        let rect = popup_rect(height, width, area);
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(" BREAKPOINTS ")
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(palette.breakpoint_border));
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        let selected_style = Style::default()
            .fg(palette.selection)
            .add_modifier(Modifier::REVERSED);

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Explanation and blank line
        lines.push(Line::from(explanation.to_string()));
        lines.push(Line::from(""));

        // Breakpoint list, windowed on the selection.
        // 6 non-list rows: explanation, blank, blank, input, message, footer.
        let visible = (content.height as usize).saturating_sub(6);
        let first = if breakpoints.is_empty() {
            0
        } else {
            self.selected
                .saturating_sub(visible.saturating_sub(1))
                .min(breakpoints.len().saturating_sub(visible))
        };
        let end = (first + visible).min(breakpoints.len());

        for (i, desc) in descriptions
            .iter()
            .enumerate()
            .skip(first)
            .take(end - first)
        {
            let is_selected = i == self.selected;
            let pointer = if is_selected { ">" } else { " " };
            let text = format!("{pointer} {desc}");
            if is_selected {
                lines.push(Line::from(Span::styled(text, selected_style)));
            } else {
                lines.push(Line::from(text));
            }
        }

        // Fill remaining list space with blank lines
        while lines.len() < 2 + visible {
            lines.push(Line::from(""));
        }

        // Blank, input line, message, footer
        lines.push(Line::from(""));
        lines.push(Line::from(input));
        lines.push(Line::from(self.message.clone()));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines), content);
    }
}

/// The target of an edit operation.
#[derive(Clone, Copy, PartialEq)]
enum EditTarget {
    /// Edit the program counter (CI).
    Ci,
    /// Edit the accumulator (A).
    A,
    /// Edit a store word.
    Word,
}

/// Which field is being typed in the editor.
#[derive(Clone, Copy, PartialEq)]
enum EditField {
    /// The store word address (only for Word target).
    Address,
    /// The value to write.
    Value,
}

/// The editor pop-up for modifying registers and store words.
///
/// While open, all key events are routed here by the main event loop.
/// Press T to cycle the target (CI → A → WORD → CI) — except in the WORD
/// value field, where T types the letter so mnemonics like STP can be
/// entered — then type a number or instruction and press Enter to apply.
pub struct EditorPopup {
    open: bool,
    target: EditTarget,
    field: EditField,
    buffer: String,
    address: usize,
    message: String,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    last_rect: Rect,
}

impl EditorPopup {
    pub fn new() -> Self {
        EditorPopup {
            open: false,
            target: EditTarget::Word,
            field: EditField::Address,
            buffer: String::new(),
            address: 0,
            message: String::new(),
            last_rect: Rect::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the pop-up, resetting the editing state.
    pub fn open(&mut self) {
        self.open = true;
        self.target = EditTarget::Word;
        self.field = EditField::Address;
        self.buffer.clear();
        self.message.clear();
    }

    /// Open the pop-up pre-configured to edit CI (double-click on the CI line).
    pub fn open_for_ci(&mut self) {
        self.open = true;
        self.target = EditTarget::Ci;
        self.field = EditField::Value;
        self.buffer.clear();
        self.message.clear();
    }

    /// Open the pop-up pre-configured to edit the accumulator (double-click on the A line).
    pub fn open_for_a(&mut self) {
        self.open = true;
        self.target = EditTarget::A;
        self.field = EditField::Value;
        self.buffer.clear();
        self.message.clear();
    }

    /// Open the pop-up pre-configured to edit a store word (double-click on a store line).
    pub fn open_for_word(&mut self, address: usize) {
        self.open = true;
        self.target = EditTarget::Word;
        self.field = EditField::Value;
        self.address = address;
        self.buffer.clear();
        self.message.clear();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// Handle a key press while the pop-up is open.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                self.message.clear();
            }
            KeyCode::Enter => {
                self.apply(sim);
            }
            // T cycles the target — except in the WORD value/instruction
            // field, where letters are typed (mnemonics like STP contain
            // a T); there the key falls through to the Char arm below
            KeyCode::Char('t') | KeyCode::Char('T')
                if !matches!(
                    (self.target, self.field),
                    (EditTarget::Word, EditField::Value)
                ) =>
            {
                self.target = match self.target {
                    EditTarget::Ci => EditTarget::A,
                    EditTarget::A => EditTarget::Word,
                    EditTarget::Word => EditTarget::Ci,
                };
                self.field = if self.target == EditTarget::Word {
                    EditField::Address
                } else {
                    EditField::Value
                };
                self.buffer.clear();
                self.message.clear();
            }
            KeyCode::Char(ch) => {
                // A digit typed with Shift on a French AZERTY keyboard arrives
                // as the unshifted top-row char + SHIFT; translate it first
                let ch = digit_of(&key).unwrap_or(ch);
                // Accept digits everywhere; letters, spaces, and minus only in Value field
                let accepted = match ch {
                    '-' => self.field == EditField::Value && self.buffer.is_empty(),
                    ' ' => {
                        self.field == EditField::Value
                            && !self.buffer.is_empty()
                            && !self.buffer.ends_with(' ')
                    }
                    c if c.is_ascii_digit() => true,
                    c if c.is_ascii_alphabetic() => self.field == EditField::Value,
                    _ => false,
                };
                if accepted && self.buffer.len() < 15 {
                    let ch = if ch.is_ascii_alphabetic() {
                        ch.to_ascii_uppercase()
                    } else {
                        ch
                    };
                    self.buffer.push(ch);
                    self.message.clear();
                }
            }
            _ => {}
        }
    }

    /// Parse the buffer and apply the edit to the simulator.
    fn apply(&mut self, sim: &mut Simulator) {
        let buffer = std::mem::take(&mut self.buffer);

        match self.target {
            EditTarget::Ci => {
                let value: i32 = match buffer.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        self.message = "TYPE A NUMBER".to_string();
                        return;
                    }
                };
                sim.ci = value;
                // The recorded future no longer matches the edited machine
                sim.restart_history();
                self.message = format!("CI = {value}");
            }
            EditTarget::A => {
                let value: i32 = match buffer.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        self.message = "TYPE A NUMBER".to_string();
                        return;
                    }
                };
                sim.a = value;
                sim.restart_history();
                self.message = format!("A = {value}");
            }
            EditTarget::Word => {
                if self.field == EditField::Address {
                    let value: i32 = match buffer.parse() {
                        Ok(n) => n,
                        Err(_) => {
                            self.message = "TYPE A NUMBER".to_string();
                            return;
                        }
                    };
                    if !(0..32).contains(&value) {
                        self.message = "ADDRESS MUST BE 0 TO 31".to_string();
                        return;
                    }
                    self.address = value as usize;
                    self.field = EditField::Value;
                    return;
                }
                // Value field: accept a number or an instruction (LDN 24, STO 26, …)
                match Store::parse_word(&buffer) {
                    Ok(word) => {
                        sim.store.words[self.address] = word;
                        sim.restart_history();
                        let (opcode, data) = Store::decode_word(word);
                        self.message =
                            format!("WORD {:02} = {word}  ({opcode} {data:02})", self.address);
                        self.field = EditField::Address;
                    }
                    Err(msg) => {
                        self.message = msg;
                    }
                }
            }
        }
    }

    /// The input line showing the current editing state.
    fn input_line(&self) -> String {
        match self.target {
            EditTarget::Ci => format!("CI   VALUE: {}_", self.buffer),
            EditTarget::A => format!("A    VALUE: {}_", self.buffer),
            EditTarget::Word => match self.field {
                EditField::Address => format!("WORD {}_  VALUE/INSTR:", self.buffer),
                EditField::Value => {
                    format!("WORD {:02}  VALUE/INSTR: {}_", self.address, self.buffer)
                }
            },
        }
    }

    /// A live preview of what the current input would parse to.
    ///
    /// Only shown for the WORD value field when the buffer is non-empty
    /// and parses successfully. For plain numbers, shows just the value;
    /// for instructions, shows the decoded opcode and operand.
    fn preview_line(&self) -> String {
        if self.target != EditTarget::Word
            || self.field != EditField::Value
            || self.buffer.is_empty()
        {
            return String::new();
        }

        match Store::parse_word(&self.buffer) {
            Ok(word) => {
                if self.buffer.parse::<i32>().is_ok() {
                    // Plain number — no need to decode
                    format!("\u{2192} {word:>12}")
                } else {
                    let (opcode, data) = Store::decode_word(word);
                    format!("\u{2192} {word:>12}  ({opcode} {data:02})")
                }
            }
            Err(_) => String::new(),
        }
    }

    pub fn draw(
        &mut self,
        f: &mut Frame,
        area: Rect,
        ci: i32,
        a: i32,
        store: &[i32; 32],
        palette: &Palette,
    ) {
        let explanation = "Edit a register or store word";
        let footer = "ENTER apply \u{b7} T target \u{b7} ESC close";

        let input = self.input_line();
        let preview = self.preview_line();

        // Show the current value of the selected target
        let current = match self.target {
            EditTarget::Ci => format!("current CI:  {ci:>12}"),
            EditTarget::A => format!("current A:   {a:>12}"),
            EditTarget::Word => {
                let word = store.get(self.address).copied().unwrap_or(0);
                format!("current [{:02}]: {word:>12}", self.address)
            }
        };

        let content_width = display_width(explanation)
            .max(display_width(footer))
            .max(display_width(&input))
            .max(display_width(&current))
            .max(display_width(&preview));

        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width + 4) as u16;
        // Title in border. Interior: explanation, blank, current, blank, input,
        // preview, message, blank, footer = 9. Plus 2 border rows.
        let height = 11u16;

        let rect = popup_rect(height, width, area);
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(" EDIT ")
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(palette.editor_border));
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        let lines: Vec<Line<'static>> = vec![
            Line::from(explanation.to_string()),
            Line::from(""),
            Line::from(Span::styled(
                current,
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(""),
            Line::from(input),
            Line::from(Span::styled(
                preview,
                Style::default()
                    .fg(palette.preview)
                    .add_modifier(Modifier::DIM),
            )),
            Line::from(self.message.clone()),
            Line::from(""),
            Line::from(Span::styled(
                footer.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];

        f.render_widget(Paragraph::new(lines), content);
    }
}

/// Maximum length of a comment typed in the inline editor: the comment
/// column caps at `MAX_COMMENT_WIDTH` cells, so typing past it would be
/// invisible anyway.
const MAX_COMMENT_INPUT: usize = MAX_COMMENT_WIDTH;

/// The inline editor for a store word's source comment.
///
/// Opened by double-clicking the comment column of a store row; the text
/// box is drawn inline in the column on that row (not as a centered
/// pop-up). The buffer is prefilled with the word's current comment.
/// Enter validates: the buffer is written to the store (`Store::set_comment`;
/// a blank buffer clears the comment) and the editor closes. Esc cancels
/// (routed by `App::handle_key`, which closes without applying), as does a
/// mouse click outside the text box.
/// Backspace deletes one character, Ctrl+Backspace a whole word.
///
/// While open, all key events are routed here by the main event loop;
/// repeats are accepted so holding Backspace deletes several characters.
pub struct CommentEditor {
    open: bool,
    address: usize,
    buffer: String,
    /// Rectangle of the inline text box from the last draw, for mouse
    /// hit-testing.
    last_rect: Rect,
}

impl CommentEditor {
    pub fn new() -> Self {
        CommentEditor {
            open: false,
            address: 0,
            buffer: String::new(),
            last_rect: Rect::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Record the text box's screen rectangle (called by the draw code,
    /// which knows where the box is drawn).
    pub fn set_last_rect(&mut self, rect: Rect) {
        self.last_rect = rect;
    }

    /// Whether the given screen cell lies inside the text box.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// The text typed so far.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Display width the inline editor needs in the comment column: the
    /// buffer plus one cell for the cursor, so the text box stays fully
    /// visible while typing. Zero when the editor is closed.
    pub fn edit_width(&self) -> usize {
        if self.open {
            (self.buffer.chars().count() + 1).min(MAX_COMMENT_WIDTH)
        } else {
            0
        }
    }

    /// Whether the editor is open on the given store word (that row draws
    /// the inline text box instead of the comment).
    pub fn is_open_for(&self, index: usize) -> bool {
        self.open && self.address == index
    }

    /// Open on the given word, prefilled with its current comment.
    pub fn open_for(&mut self, address: usize, current: Option<&str>) {
        self.open = true;
        self.address = address;
        self.buffer = current.unwrap_or_default().to_string();
    }

    /// Close without applying (Esc).
    pub fn close(&mut self) {
        self.open = false;
        self.buffer.clear();
    }

    /// Handle a key press while the editor is open. Repeats are accepted
    /// so holding Backspace deletes several characters.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        match key.code {
            // Ctrl+Backspace erases the whole word before the cursor
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_word();
            }
            KeyCode::Backspace => {
                self.buffer.pop();
            }
            KeyCode::Enter => self.apply(sim),
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+<key> arrives as the unshifted char + CONTROL, and
                // many terminals send Ctrl+Backspace as byte 0x08, which
                // crossterm reports as 'h' + CONTROL (0x08 is Ctrl+H).
                // The raw 0x08/0x7F bytes are covered too. Other Ctrl
                // combos have no binding here: ignored, not typed.
                if matches!(c, 'h' | '\u{8}' | '\u{7f}') {
                    self.delete_word();
                }
            }
            KeyCode::Char(c) => {
                // AZERTY digits arrive as the unshifted top-row char plus
                // SHIFT; translate them, keep every other printable char
                // (including `;` and `:`) literal
                let c = azerty_digit_of(&key).unwrap_or(c);
                if !c.is_control() && self.buffer.chars().count() < MAX_COMMENT_INPUT {
                    self.buffer.push(c);
                }
            }
            _ => {}
        }
    }

    /// Erase the word before the cursor (Ctrl+Backspace): the run of
    /// trailing whitespace first, then the run of non-whitespace
    /// characters, so "hello world  |" becomes "hello " in one press.
    fn delete_word(&mut self) {
        while self
            .buffer
            .chars()
            .last()
            .is_some_and(|c| c.is_whitespace())
        {
            self.buffer.pop();
        }
        while self
            .buffer
            .chars()
            .last()
            .is_some_and(|c| !c.is_whitespace())
        {
            self.buffer.pop();
        }
    }

    /// Validate the edit: write the buffer to the word's comment and close.
    /// A blank buffer clears the comment; trailing whitespace is dropped,
    /// like the loaders do (leading indentation is kept).
    fn apply(&mut self, sim: &mut Simulator) {
        let text = self.buffer.trim_end();
        let comment = if text.trim().is_empty() {
            None
        } else {
            Some(text.to_string())
        };
        sim.store.set_comment(self.address, comment);
        self.close();
    }
}

/// Which field of a store word the picker rewrites.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PickerKind {
    /// The opcode (mnemonic) field, bits 13-15.
    Mnemonic,
    /// The data (operand/address) field, bits 0-4.
    Data,
}

/// Compute a pop-up rectangle whose left edge is at `anchor_col` (just right
/// of the store text), vertically centered on `anchor_row`, clamped to the
/// area: when the popup would overflow the right edge, it is shifted left.
fn side_rect(width: u16, height: u16, area: Rect, anchor_col: u16, anchor_row: u16) -> Rect {
    let height = height.min(area.height);
    let width = width.min(area.width);
    let y = anchor_row
        .saturating_sub(height / 2)
        .clamp(area.y, area.y + area.height - height);
    let x = anchor_col.max(area.x).min(area.x + area.width - width);
    Rect::new(x, y, width, height)
}

/// A side list of choices that rewrites one field of a store word.
///
/// Opened by double-clicking the mnemonic or the data part of a store row;
/// the list appears just right of the store text, vertically anchored on
/// that row.
/// The mnemonic list is the eight opcodes in a single column, colored like
/// in the store view; the data list is the 32 possible addresses in a grid
/// of 8 columns. The entry matching the word's current value is marked
/// with `>`. Clicking an entry — or moving the selection with the arrow
/// keys and pressing Enter — rewrites only that field of the word, keeping
/// every other bit (`Store::with_opcode` / `Store::with_data`), and closes
/// the list. Esc closes without changes.
///
/// While open, the picker is modal: `App` routes all key and mouse events
/// to it.
pub struct PickerPopup {
    open: bool,
    kind: PickerKind,
    address: usize,
    /// The highlighted entry (arrow keys move it, Enter applies it).
    selected: usize,
    /// Screen row the list is anchored on (the double-clicked row).
    anchor_row: u16,
    /// The entry cells from the last draw, for mouse hit-testing; the
    /// position in the vector is the entry index.
    entry_cells: Vec<Rect>,
}

impl PickerPopup {
    pub fn new() -> Self {
        PickerPopup {
            open: false,
            kind: PickerKind::Mnemonic,
            address: 0,
            selected: 0,
            anchor_row: 0,
            entry_cells: Vec::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Which field of the word the open picker rewrites (tests only).
    #[cfg(test)]
    pub fn kind(&self) -> PickerKind {
        self.kind
    }

    /// How many entries the list has: 8 opcodes or 32 addresses.
    fn entry_count(&self) -> usize {
        match self.kind {
            PickerKind::Mnemonic => Opcode::ALL.len(),
            PickerKind::Data => 32,
        }
    }

    /// Entries per row in the list layout: one column of opcodes, or a
    /// grid of 8 addresses per row.
    fn columns(&self) -> usize {
        match self.kind {
            PickerKind::Mnemonic => 1,
            PickerKind::Data => 8,
        }
    }

    /// Open on the mnemonic field of the word at `address`, preselecting
    /// the word's current opcode.
    pub fn open_for_mnemonic(&mut self, address: usize, word: i32, anchor_row: u16) {
        self.open = true;
        self.kind = PickerKind::Mnemonic;
        self.address = address;
        self.anchor_row = anchor_row;
        let (opcode, _) = Store::decode_word(word);
        self.selected = Opcode::ALL.iter().position(|&o| o == opcode).unwrap_or(0);
        self.entry_cells.clear();
    }

    /// Open on the data field of the word at `address`, preselecting the
    /// word's current data value.
    pub fn open_for_data(&mut self, address: usize, word: i32, anchor_row: u16) {
        self.open = true;
        self.kind = PickerKind::Data;
        self.address = address;
        self.anchor_row = anchor_row;
        let (_, data) = Store::decode_word(word);
        self.selected = (data as usize).min(31);
        self.entry_cells.clear();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// The entry drawn under the given screen cell, if any.
    pub fn entry_at(&self, column: u16, row: u16) -> Option<usize> {
        self.entry_cells
            .iter()
            .position(|r| r.contains(Position::new(column, row)))
    }

    /// Handle a key press while the picker is open: the arrows move the
    /// selection (clamped to the list), Enter applies it. Repeats are
    /// accepted so holding an arrow moves quickly.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        let len = self.entry_count();
        let cols = self.columns();
        let row = self.selected / cols;
        let col = self.selected % cols;
        match key.code {
            KeyCode::Up => {
                if row > 0 {
                    self.selected -= cols;
                }
            }
            KeyCode::Down => {
                if self.selected + cols < len {
                    self.selected += cols;
                }
            }
            KeyCode::Left => {
                if col > 0 {
                    self.selected -= 1;
                }
            }
            KeyCode::Right => {
                if col + 1 < cols && self.selected + 1 < len {
                    self.selected += 1;
                }
            }
            KeyCode::Enter => self.apply(sim),
            _ => {}
        }
    }

    /// Select the given entry and apply it (mouse click).
    pub fn select_and_apply(&mut self, entry: usize, sim: &mut Simulator) {
        self.selected = entry;
        self.apply(sim);
    }

    /// Rewrite the word's field with the selected entry, keeping every
    /// other bit of the word, and close.
    fn apply(&mut self, sim: &mut Simulator) {
        let word = sim.store.words[self.address];
        sim.store.words[self.address] = match self.kind {
            PickerKind::Mnemonic => Store::with_opcode(word, Opcode::ALL[self.selected]),
            PickerKind::Data => Store::with_data(word, self.selected as i32),
        };
        sim.restart_history();
        self.close();
    }

    /// Draw the list. `anchor_col` is the screen column just right of the
    /// store text (the caller computes it from the current heatmap width,
    /// so the popup sits next to the mnemonic/data fields being edited).
    pub fn draw(
        &mut self,
        f: &mut Frame,
        area: Rect,
        store: &[i32; 32],
        palette: &Palette,
        anchor_col: u16,
    ) {
        let (cols, entry_width, title) = match self.kind {
            PickerKind::Mnemonic => (1usize, 6u16, format!(" MNEMONIC [{:02}] ", self.address)),
            PickerKind::Data => (8, 4u16, format!(" DATA [{:02}] ", self.address)),
        };
        let len = self.entry_count();
        let rows = len.div_ceil(cols);
        let footer = "CLICK or ENTER apply \u{b7} ESC close";

        let content_width = cols * entry_width as usize;
        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width
            .max(display_width(footer))
            .max(display_width(&title))
            + 4) as u16;
        // Entries + footer row, plus the 2 border rows
        let height = (rows + 3) as u16;

        let rect = side_rect(width, height, area, anchor_col, self.anchor_row);
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(title)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(palette.editor_border));
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        // The entry matching the word's current value, marked with '>'
        let (current_op, current_data) = Store::decode_word(store[self.address]);
        let current = match self.kind {
            PickerKind::Mnemonic => Opcode::ALL.iter().position(|&o| o == current_op),
            PickerKind::Data => Some((current_data as usize).min(31)),
        };

        let selected_style = Style::default()
            .fg(palette.selection)
            .add_modifier(Modifier::REVERSED);

        self.entry_cells.clear();
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(rows + 1);
        for r in 0..rows {
            let mut spans: Vec<Span<'static>> = Vec::with_capacity(cols);
            for c in 0..cols {
                let i = r * cols + c;
                let pointer = if Some(i) == current { ">" } else { " " };
                let text = match self.kind {
                    PickerKind::Mnemonic => format!("{pointer}{:<4} ", Opcode::ALL[i]),
                    PickerKind::Data => format!("{pointer}{i:02} "),
                };
                let style = if i == self.selected {
                    selected_style
                } else {
                    match self.kind {
                        // Same syntax highlighting as the store view
                        PickerKind::Mnemonic => Style::default()
                            .fg(palette.opcode(Opcode::ALL[i]))
                            .add_modifier(Modifier::BOLD),
                        PickerKind::Data => Style::default(),
                    }
                };
                spans.push(Span::styled(text, style));
                self.entry_cells.push(Rect {
                    x: content.x + (c * entry_width as usize) as u16,
                    y: content.y + r as u16,
                    width: entry_width,
                    height: 1,
                });
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines), content);
    }
}

/// Maximum length of the file name typed in the save pop-up.
const MAX_PATH_INPUT: usize = 256;

/// The proposed save name for a loaded file: `<stem>_edited.<ext>`
/// (e.g. `samples/factorct.asm` → `factorct_edited.asm`). A stem that
/// already ends with `_edited` is kept as-is, so re-saving an edited file
/// does not pile up suffixes (`factorct_edited.asm` stays `factorct_edited.asm`).
fn default_save_name(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("state");
    let stem = if stem.ends_with("_edited") {
        stem.to_string()
    } else {
        format!("{stem}_edited")
    };
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem,
    }
}

/// The save pop-up: a single name field; Enter writes the current machine
/// state to `<dir>/<field>`.
///
/// It opens on the (absolute) folder of the file the machine was loaded
/// from — shown as the `DIR:` line — with the field prefilled with
/// `<name>_edited.<ext>` (e.g. `factorct.asm` → `factorct_edited.asm`);
/// without a source file it opens on the current working directory with an
/// empty field. The format follows the extension (`.asm` assembly or `.snp`
/// snapshot), and each word's source comment is saved along. The field
/// accepts only alphanumerics, `-`, `_` and `.`, so the entered name is
/// always a plain file name inside the target folder.
///
/// While open, all key events are routed here by the main event loop. Only
/// Esc or a mouse click outside the pop-up closes it — every letter can
/// appear in a file name, so no letter key closes it. Enter saves but keeps
/// the pop-up open, so a failed write (bad extension, unwritable path) can
/// be fixed and retried, or another file saved from the same state.
pub struct SavePopup {
    open: bool,
    /// The folder names are resolved against (the open file's folder).
    dir: PathBuf,
    buffer: String,
    message: String,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    last_rect: Rect,
}

impl SavePopup {
    pub fn new() -> Self {
        SavePopup {
            open: false,
            dir: PathBuf::from("."),
            buffer: String::new(),
            message: String::new(),
            last_rect: Rect::default(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Open the pop-up, resetting the input and message.
    ///
    /// When the machine was loaded from a file (`source`), the target folder
    /// is that file's folder — canonicalized, so it is absolute — and the
    /// `FILE:` field is prefilled with `<name>_edited.<ext>` (e.g.
    /// `factorct.asm` → `factorct_edited.asm`). Without a source file, the
    /// folder is the current working directory and the field is empty.
    pub fn open(&mut self, source: Option<&Path>) {
        self.open = true;
        self.message.clear();
        let dir = match source {
            Some(path) => {
                // A bare file name has an empty parent; use "." then
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            }
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        // Canonicalize so the target folder is shown and saved as absolute
        self.dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        self.buffer = match source {
            Some(path) => default_save_name(path),
            None => String::new(),
        };
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// Handle a key press while the pop-up is open. Repeats are accepted so
    /// holding Backspace deletes several characters.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }

        match key.code {
            KeyCode::Backspace => {
                self.buffer.pop();
                self.message.clear();
            }
            KeyCode::Enter => {
                self.save(sim);
            }
            KeyCode::Char(c) => {
                // A digit typed with Shift on a French AZERTY keyboard arrives
                // as the unshifted top-row char + SHIFT; translate it first
                let c = digit_of(&key).unwrap_or(c);
                // Whitelist only: alphanumerics, dash, underscore, dot. With no
                // path separator accepted, the name can never escape the
                // target folder (`..` alone just fails to write, see below)
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    if self.buffer.chars().count() < MAX_PATH_INPUT {
                        self.buffer.push(c);
                        self.message.clear();
                    }
                } else if !c.is_control() {
                    self.message = "ONLY A-Z 0-9 - _ . ALLOWED".to_string();
                }
            }
            _ => {}
        }
    }

    /// Write the machine state to `<dir>/<buffer>` and report the outcome.
    fn save(&mut self, sim: &mut Simulator) {
        let name = self.buffer.trim();
        if name.is_empty() {
            self.message = "TYPE A FILE NAME".to_string();
            return;
        }
        // The input whitelist admits no path separator, so the joined path
        // always stays inside the target folder
        let path = self.dir.join(name);
        match sim.save(&path) {
            Ok(()) => self.message = format!("SAVED TO {}", path.display()),
            Err(e) => self.message = format!("ERROR: {e}"),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, palette: &Palette) {
        let explanation = "Save the machine state to a .asm or .snp file";
        let dir_line = format!("DIR: {}", self.dir.display());
        let footer = "ENTER save \u{b7} ESC close";
        let input = format!("FILE: {}_", self.buffer);

        let content_width = display_width(explanation)
            .max(display_width(&dir_line))
            .max(display_width(footer))
            .max(display_width(&input))
            .max(display_width(&self.message));

        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width + 4) as u16;
        // Title in border. Interior: explanation, dir, blank, input, message,
        // footer = 6. Plus 2 border rows.
        let height = 8u16;

        let rect = popup_rect(height, width, area);
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(" SAVE ")
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(palette.save_border));
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        let lines: Vec<Line<'static>> = vec![
            Line::from(explanation.to_string()),
            Line::from(Span::styled(
                dir_line,
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(""),
            Line::from(input),
            Line::from(self.message.clone()),
            Line::from(Span::styled(
                footer.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];

        f.render_widget(Paragraph::new(lines), content);
    }
}

/// The values the settings pop-up shows, copied from the app and the
/// snapshot. Kept separate from `App` so `App::draw` can build it from
/// disjoint fields while the pop-up is mutably borrowed for drawing.
pub struct SettingsView {
    /// The smoothed speed shown by the UI thread, and the target speed.
    pub displayed_speed: u32,
    pub target_speed: u32,
    pub heatmap: HeatmapMode,
    pub flow: FlowMode,
    pub comments: bool,
    pub bit_representation: usize,
    pub theme: Theme,
}

/// One row of the settings pop-up: the label, the keyboard shortcut shown
/// dimmed in its own column left of the controls, then its spans — each
/// either a clickable button (text + control) or plain value text.
type SettingsRow = (
    &'static str,
    &'static str,
    Vec<(String, Option<SettingControl>)>,
);

/// A clickable control of the settings pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingControl {
    SpeedDown,
    SpeedUp,
    Heatmap,
    Flow,
    Comments,
    Bits,
    Theme,
}

/// The settings pop-up, opened from the menu bar.
///
/// One row per setting, each with its keyboard shortcut on the left of a
/// clickable control: machine speed (`[-]` / `[+]` around the current
/// value, `I/K` or `+/-`), and the current value of the heatmap mode,
/// flow mode, comments toggle, store bits display, and theme (`M`/`F`/`C`/`D`/
/// `T`) — clicking a value cycles it, exactly like its keyboard key.
///
/// While open, `App` routes the matching keys here and clicks on the
/// controls to `App::perform_setting`; Esc or a click outside closes it.
/// A control is highlighted only while the mouse hovers over it.
pub struct SettingsPopup {
    open: bool,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    last_rect: Rect,
    /// The control buttons' cells from the last draw: (rect, control).
    buttons: Vec<(Rect, SettingControl)>,
    /// The control under the mouse cursor, for the hover highlight.
    hover: Option<SettingControl>,
}

impl SettingsPopup {
    pub fn new() -> Self {
        SettingsPopup {
            open: false,
            last_rect: Rect::default(),
            buttons: Vec::new(),
            hover: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.hover = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hover = None;
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// The control drawn under the given screen cell, if any.
    pub fn control_at(&self, column: u16, row: u16) -> Option<SettingControl> {
        self.buttons
            .iter()
            .find(|(rect, _)| rect.contains(Position::new(column, row)))
            .map(|(_, control)| *control)
    }

    /// The control under the mouse cursor (tests only).
    #[cfg(test)]
    pub fn hover(&self) -> Option<SettingControl> {
        self.hover
    }

    /// Record the control under the mouse cursor: the one drawn under it is
    /// highlighted, the others are not.
    pub fn set_hover(&mut self, hover: Option<SettingControl>) {
        self.hover = hover;
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, view: &SettingsView, palette: &Palette) {
        let footer = "CLICK a control \u{b7} ESC close";

        let speed_text = format!("{}/{} ips", view.displayed_speed, view.target_speed);
        let (on, off) =
            BIT_REPRESENTATIONS[view.bit_representation.min(BIT_REPRESENTATIONS.len() - 1)];
        // Each row: the label, the shortcut, then its spans — a button (text
        // + control) or plain value text between the speed buttons
        let rows: [SettingsRow; 6] = [
            (
                "Machine speed",
                "I/K",
                vec![
                    ("[-]".to_string(), Some(SettingControl::SpeedDown)),
                    (format!(" {speed_text} "), None),
                    ("[+]".to_string(), Some(SettingControl::SpeedUp)),
                ],
            ),
            (
                "Heatmap mode",
                "M",
                vec![(
                    format!("[{}]", view.heatmap.label().unwrap_or("OFF")),
                    Some(SettingControl::Heatmap),
                )],
            ),
            (
                "Flow mode",
                "F",
                vec![(
                    format!("[{}]", view.flow.label().unwrap_or("OFF")),
                    Some(SettingControl::Flow),
                )],
            ),
            (
                "Comments",
                "C",
                vec![(
                    format!("[{}]", if view.comments { "ON" } else { "OFF" }),
                    Some(SettingControl::Comments),
                )],
            ),
            (
                "Store bits",
                "D",
                vec![(format!("[{on}{off}]"), Some(SettingControl::Bits))],
            ),
            (
                "Theme",
                "T",
                vec![(
                    format!("[{}]", view.theme.label()),
                    Some(SettingControl::Theme),
                )],
            ),
        ];

        let label_width = rows
            .iter()
            .map(|(label, _, _)| display_width(label))
            .max()
            .unwrap_or(0);
        let hint_width = rows
            .iter()
            .map(|(_, hint, _)| display_width(hint))
            .max()
            .unwrap_or(0);
        let control_width = rows
            .iter()
            .map(|(_, _, segs)| {
                segs.iter()
                    .map(|(text, _)| display_width(text))
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        // Leading margin (2) + label + gap (2) + shortcut column + gap (2)
        // + the widest row's controls, so every column is vertically aligned
        let content_width = 2 + label_width + 2 + hint_width + 2 + control_width;
        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width.max(display_width(footer)) + 4) as u16;
        // The rows + a blank line + the footer, plus the 2 border rows
        let height = (rows.len() + 4) as u16;

        let rect = popup_rect(height, width, area);
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(" SETTINGS ")
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(palette.editor_border));
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        // A control is highlighted (same hover style as the menu bar) only
        // while the mouse is over it; otherwise it is plain key-hint text
        let button_style = Style::default().fg(palette.key_hint);
        let hover_style = Style::default().fg(palette.hover_fg).bg(palette.hover);
        let value_style = Style::default()
            .fg(palette.value)
            .add_modifier(Modifier::BOLD);

        self.buttons.clear();
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(rows.len() + 2);
        for (i, (label, hint, segs)) in rows.iter().enumerate() {
            // Label, then the dimmed shortcut right-aligned in its own
            // column, then the controls — each column starts at a fixed x
            let mut spans = vec![
                Span::raw(format!("  {label:<label_width$}  ")),
                Span::styled(
                    format!("{hint:>hint_width$}"),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::raw("  "),
            ];
            let mut x = content.x + 2 + label_width as u16 + 2 + hint_width as u16 + 2;
            let y = content.y + i as u16;
            for (text, control) in segs {
                let w = display_width(text) as u16;
                let style = match control {
                    Some(c) if self.hover == Some(*c) => hover_style,
                    Some(_) => button_style,
                    None => value_style,
                };
                spans.push(Span::styled(text.clone(), style));
                if let Some(control) = control {
                    self.buttons.push((Rect::new(x, y, w, 1), *control));
                }
                x += w;
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines), content);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(c: char, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new_with_kind(KeyCode::Char(c), mods, KeyEventKind::Press)
    }

    #[test]
    fn breakpoint_popup_accepts_plain_and_azerty_shifted_digits() {
        let mut popup = BreakpointPopup::new();
        popup.open();
        let mut breakpoints = Vec::new();

        popup.handle_key(press('9', KeyModifiers::NONE), &mut breakpoints);
        // AZERTY top row with Shift, as CSI-u terminals deliver it
        // (unshifted char + SHIFT modifier)
        popup.handle_key(press('&', KeyModifiers::SHIFT), &mut breakpoints); // -> 1
        popup.handle_key(press('\u{e9}', KeyModifiers::SHIFT), &mut breakpoints); // é -> 2
        popup.handle_key(press('"', KeyModifiers::SHIFT), &mut breakpoints); // -> 3
        popup.handle_key(press('\'', KeyModifiers::SHIFT), &mut breakpoints); // -> 4
        popup.handle_key(press('(', KeyModifiers::SHIFT), &mut breakpoints); // -> 5
        popup.handle_key(press('-', KeyModifiers::SHIFT), &mut breakpoints); // -> 6
        popup.handle_key(press('\u{e8}', KeyModifiers::SHIFT), &mut breakpoints); // è -> 7
        popup.handle_key(press('_', KeyModifiers::SHIFT), &mut breakpoints); // -> 8
        popup.handle_key(press('\u{e7}', KeyModifiers::SHIFT), &mut breakpoints); // ç -> 9
        popup.handle_key(press('\u{e0}', KeyModifiers::SHIFT), &mut breakpoints); // à -> 0
        assert_eq!("91234567890", popup.buffer); // 9 then AZERTY 1..0 (11-char cap fits)

        // Without Shift, the AZERTY chars are not digits
        popup.handle_key(press('&', KeyModifiers::NONE), &mut breakpoints);
        assert_eq!("91234567890", popup.buffer);
    }

    #[test]
    fn breakpoint_popup_minus_still_works_without_shift() {
        let mut popup = BreakpointPopup::new();
        popup.open();
        let mut breakpoints = Vec::new();

        // Enter an address to move to the Value field
        popup.handle_key(press('3', KeyModifiers::NONE), &mut breakpoints);
        popup.handle_key(
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
            &mut breakpoints,
        );
        assert!(matches!(popup.field, InputField::Value));

        popup.handle_key(press('-', KeyModifiers::NONE), &mut breakpoints);
        assert_eq!("-", popup.buffer);
    }

    #[test]
    fn editor_popup_accepts_azerty_shifted_digits() {
        let mut popup = EditorPopup::new();
        popup.open_for_ci();
        let mut sim = Simulator::new();

        popup.handle_key(press('\u{e0}', KeyModifiers::SHIFT), &mut sim); // à -> 0
        popup.handle_key(press('(', KeyModifiers::SHIFT), &mut sim); // ( -> 5
        popup.handle_key(press('7', KeyModifiers::NONE), &mut sim);
        assert_eq!("057", popup.buffer);
    }

    #[test]
    fn editor_popup_accepts_t_in_instruction_field() {
        let mut popup = EditorPopup::new();
        popup.open_for_word(5);
        let mut sim = Simulator::new();

        // A mnemonic containing T must be typable in the VALUE/INSTR field
        popup.handle_key(press('s', KeyModifiers::NONE), &mut sim);
        popup.handle_key(press('t', KeyModifiers::NONE), &mut sim);
        popup.handle_key(press('p', KeyModifiers::NONE), &mut sim);
        assert_eq!("STP", popup.buffer);
        assert!(matches!(popup.target, EditTarget::Word));

        // Enter applies it as the STP instruction
        popup.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &mut sim);
        assert_eq!(
            sim.store.words[5],
            Store::parse_word("STP").unwrap(),
            "message: {}",
            popup.message
        );
    }

    #[test]
    fn editor_popup_t_still_cycles_target_outside_instruction_field() {
        let mut popup = EditorPopup::new();
        popup.open(); // WORD target, ADDRESS field
        let mut sim = Simulator::new();

        popup.handle_key(press('t', KeyModifiers::NONE), &mut sim);
        assert!(matches!(popup.target, EditTarget::Ci));
        popup.handle_key(press('T', KeyModifiers::NONE), &mut sim);
        assert!(matches!(popup.target, EditTarget::A));
        popup.handle_key(press('t', KeyModifiers::NONE), &mut sim);
        assert!(matches!(popup.target, EditTarget::Word));
        assert!(matches!(popup.field, EditField::Address));
    }

    #[test]
    fn default_save_name_appends_edited_only_once() {
        assert_eq!(
            "factorct_edited.asm",
            default_save_name(Path::new("samples/factorct.asm"))
        );
        assert_eq!(
            "nightmare_edited.snp",
            default_save_name(Path::new("nightmare.snp"))
        );
        // Already edited: the suffix is not added again
        assert_eq!(
            "factorct_edited.asm",
            default_save_name(Path::new("/tmp/factorct_edited.asm"))
        );
        assert_eq!(
            "factorct_edited.snp",
            default_save_name(Path::new("factorct_edited.snp"))
        );
        // No extension: just the stem
        assert_eq!("state_edited", default_save_name(Path::new("state")));
        assert_eq!("_edited", default_save_name(Path::new("_edited")));
        // A stem merely containing "edited" (not the suffix) still gets it
        assert_eq!(
            "edited_edited.asm",
            default_save_name(Path::new("edited.asm"))
        );
    }
}
