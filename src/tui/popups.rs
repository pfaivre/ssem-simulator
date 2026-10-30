//! Pop-up windows for the TUI: help and breakpoint editor.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::ssem::Opcode;
use crate::ssem::Store;
use crate::ssem::simulator::{Breakpoint, Simulator};

use super::{KEYMAP, opcode_color};

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
pub struct HelpPopup {
    open: bool,
}

impl HelpPopup {
    pub fn new() -> Self {
        HelpPopup { open: false }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn draw(&self, f: &mut Frame, area: Rect) {
        let key_column = KEYMAP
            .iter()
            .map(|(key, _, _)| key.chars().count())
            .max()
            .unwrap_or(0)
            + 2;

        let title = "SSEM SIMULATOR";
        let footer = "Press H to close";

        let key_style = Style::default()
            .fg(Color::Blue)
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
            ("LDN STO  JMP", Opcode::LDN, "data movement"),
            ("SUB SUB2 JRP", Opcode::SUB, "arithmetic"),
            ("CMP", Opcode::CMP, "test"),
            ("STP", Opcode::STP, "halt"),
        ] {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{mnemonics:<13}"),
                    Style::default()
                        .fg(opcode_color(opcode))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(description),
            ]));
        }
        lines.push(Line::from(
            "Markers when paused: > last  \u{bb} next  \u{2022} executed",
        ));
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::raw("Heatmap, M cycles off/exec/rw: "),
            Span::styled(
                "exec",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" line frequency \u{b7} "),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                "w",
                Style::default()
                    .fg(Color::LightRed)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" store reads vs writes per word"),
        ]));

        // Blank line before footer
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        let content_width = display_width(title)
            .max(display_width(footer))
            .max(lines.iter().map(Line::width).max().unwrap_or(0));

        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width + 4) as u16;
        // 2 border rows around the content
        let height = (lines.len() + 2) as u16;

        let rect = popup_rect(height, width, area);
        f.render_widget(Clear, rect);

        let block = Block::default()
            .title(format!(" {title} "))
            .borders(Borders::ALL);
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        f.render_widget(Paragraph::new(lines), content);
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
            KeyCode::Char('t') | KeyCode::Char('T') => {
                // Cycle the condition type
                self.kind = match self.kind {
                    BreakpointKind::Word => BreakpointKind::Ci,
                    BreakpointKind::Ci => BreakpointKind::Word,
                };
                self.field = InputField::Address;
                self.buffer.clear();
                self.message.clear();
            }
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down => {
                if !breakpoints.is_empty() {
                    self.selected = (self.selected + 1).min(breakpoints.len() - 1);
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => {
                self.delete_selected(breakpoints);
            }
            KeyCode::Char('-') => {
                if self.field == InputField::Value && self.buffer.is_empty() {
                    self.buffer.push('-');
                }
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if self.buffer.len() < 11 {
                    self.buffer.push(c);
                }
                self.message.clear();
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

    pub fn draw(&self, f: &mut Frame, area: Rect, breakpoints: &[Breakpoint]) {
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
        f.render_widget(Clear, rect);

        let block = Block::default()
            .title(" BREAKPOINTS ")
            .borders(Borders::ALL);
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        let selected_style = Style::default()
            .fg(Color::Blue)
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
/// Press T to cycle the target (CI → A → WORD → CI), type a number, and
/// press Enter to apply.
pub struct EditorPopup {
    open: bool,
    target: EditTarget,
    field: EditField,
    buffer: String,
    address: usize,
    message: String,
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
            KeyCode::Char('t') | KeyCode::Char('T') => {
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

    pub fn draw(&self, f: &mut Frame, area: Rect, ci: i32, a: i32, store: &[i32; 32]) {
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
        f.render_widget(Clear, rect);

        let block = Block::default().title(" EDIT ").borders(Borders::ALL);
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
                    .fg(Color::Green)
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
