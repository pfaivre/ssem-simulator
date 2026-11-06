use super::*;
use rust_i18n::t;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum BreakpointKind {
    /// Pause when a store word holds a given value.
    Word,
    /// Pause when the program counter holds a given address.
    Ci,
}

/// Which field of the breakpoint input is being typed.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum InputField {
    Address,
    Value,
}

/// The breakpoint editor pop-up.
///
/// Lists existing breakpoints and provides an input line to add new ones.
/// While open, all key events are routed here by the main event loop.
/// The breakpoint editor pop-up.
pub struct BreakpointPopup {
    pub(super) open: bool,
    pub(super) kind: BreakpointKind,
    pub(super) field: InputField,
    pub(super) buffer: String,
    pub(super) address: usize,
    pub(super) selected: usize,
    pub(super) message: String,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    pub(super) last_rect: Rect,
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
                self.message = ts!("popup.type_number");
                return;
            }
        };

        if self.field == InputField::Address {
            if !(0..32).contains(&number) {
                self.message = ts!("popup.address_range");
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
            self.message = ts!("popup.breakpoints.already_set");
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
                ts!("popup.breakpoints.new_ci", buffer = self.buffer.as_str())
            }
            BreakpointKind::Word => match self.field {
                InputField::Address => {
                    ts!(
                        "popup.breakpoints.new_word_address",
                        buffer = self.buffer.as_str()
                    )
                }
                InputField::Value => {
                    ts!(
                        "popup.breakpoints.new_word_value",
                        address = self.address : {:02},
                        buffer = self.buffer.as_str()
                    )
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
        let explanation = t!("popup.breakpoints.explanation");
        let footer = t!("popup.breakpoints.footer");

        let input = self.input_line();
        let descriptions: Vec<String> = breakpoints.iter().map(|bp| bp.description()).collect();

        let content_width = display_width(&explanation)
            .max(display_width(&footer))
            .max(display_width(&input))
            .max(descriptions.iter().map(display_width).max().unwrap_or(0));

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
            .title(t!("popup.breakpoints.title"))
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
