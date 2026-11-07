use super::*;
use rust_i18n::t;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum EditTarget {
    /// Edit the program counter (CI).
    Ci,
    /// Edit the accumulator (A).
    A,
    /// Edit a store word.
    Word,
}

/// Which field is being typed in the editor.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum EditField {
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
/// The editor pop-up for modifying registers and store words.
pub struct EditorPopup {
    pub(super) frame: PopupFrame,
    pub(super) target: EditTarget,
    pub(super) field: EditField,
    pub(super) buffer: String,
    pub(super) address: usize,
    pub(super) message: String,
}

impl EditorPopup {
    pub fn new() -> Self {
        EditorPopup {
            frame: PopupFrame::new(),
            target: EditTarget::Word,
            field: EditField::Address,
            buffer: String::new(),
            address: 0,
            message: String::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.frame.is_open()
    }

    /// Open the pop-up, resetting the editing state.
    pub fn open(&mut self) {
        self.frame.open = true;
        self.target = EditTarget::Word;
        self.field = EditField::Address;
        self.buffer.clear();
        self.message.clear();
    }

    /// Open the pop-up pre-configured to edit CI (double-click on the CI line).
    pub fn open_for_ci(&mut self) {
        self.frame.open = true;
        self.target = EditTarget::Ci;
        self.field = EditField::Value;
        self.buffer.clear();
        self.message.clear();
    }

    /// Open the pop-up pre-configured to edit the accumulator (double-click on the A line).
    pub fn open_for_a(&mut self) {
        self.frame.open = true;
        self.target = EditTarget::A;
        self.field = EditField::Value;
        self.buffer.clear();
        self.message.clear();
    }

    /// Open the pop-up pre-configured to edit a store word (double-click on a store line).
    pub fn open_for_word(&mut self, address: usize) {
        self.frame.open = true;
        self.target = EditTarget::Word;
        self.field = EditField::Value;
        self.address = address;
        self.buffer.clear();
        self.message.clear();
    }

    pub fn close(&mut self) {
        self.frame.close();
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.frame.contains(column, row)
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
                        self.message = ts!("popup.type_number");
                        return;
                    }
                };
                if !(0..32).contains(&value) {
                    self.message = ts!("popup.address_range");
                    return;
                }
                sim.ci = value;
                // The recorded future no longer matches the edited machine
                sim.restart_history();
                self.message = ts!("popup.editor.applied_ci", value = value);
            }
            EditTarget::A => {
                let value: i32 = match buffer.parse() {
                    Ok(n) => n,
                    Err(_) => {
                        self.message = ts!("popup.type_number");
                        return;
                    }
                };
                sim.a = value;
                sim.restart_history();
                self.message = ts!("popup.editor.applied_a", value = value);
            }
            EditTarget::Word => {
                if self.field == EditField::Address {
                    let value: i32 = match buffer.parse() {
                        Ok(n) => n,
                        Err(_) => {
                            self.message = ts!("popup.type_number");
                            return;
                        }
                    };
                    if !(0..32).contains(&value) {
                        self.message = ts!("popup.address_range");
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
                        self.message = ts!(
                            "popup.editor.applied_word",
                            address = self.address : {:02},
                            word = word,
                            opcode = opcode.to_string(),
                            data = data : {:02}
                        );
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
            EditTarget::Ci => ts!("popup.editor.ci_value", buffer = self.buffer.as_str()),
            EditTarget::A => ts!("popup.editor.a_value", buffer = self.buffer.as_str()),
            EditTarget::Word => match self.field {
                EditField::Address => {
                    ts!("popup.editor.word_address", buffer = self.buffer.as_str())
                }
                EditField::Value => ts!(
                    "popup.editor.word_value",
                    address = self.address : {:02},
                    buffer = self.buffer.as_str()
                ),
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
                    ts!("popup.editor.preview_value", word = word : {:>12})
                } else {
                    let (opcode, data) = Store::decode_word(word);
                    ts!(
                        "popup.editor.preview_instr",
                        word = word : {:>12},
                        opcode = opcode.to_string(),
                        data = data : {:02}
                    )
                }
            }
            Err(_) => String::new(),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let ci = ctx.snap.ci;
        let a = ctx.snap.a;
        let store = &ctx.snap.store;
        let explanation = t!("popup.editor.explanation");
        let footer = t!("popup.editor.footer");

        let input = self.input_line();
        let preview = self.preview_line();

        // Show the current value of the selected target
        let current = match self.target {
            EditTarget::Ci => ts!("popup.editor.current_ci", value = ci : {:>12}),
            EditTarget::A => ts!("popup.editor.current_a", value = a : {:>12}),
            EditTarget::Word => {
                let word = store.get(self.address).copied().unwrap_or(0);
                ts!(
                    "popup.editor.current_word",
                    address = self.address : {:02},
                    value = word : {:>12}
                )
            }
        };

        let content_width = display_width(&explanation)
            .max(display_width(&footer))
            .max(display_width(&input))
            .max(display_width(&current))
            .max(display_width(&preview));

        let width = popup_width(content_width);
        // Title in border. Interior: explanation, blank, current, blank, input,
        // preview, message, blank, footer = 9. Plus 2 border rows.
        let height = 11u16;

        let rect = popup_rect(height, width, area);
        let content = self.frame.begin_popup(
            f,
            rect,
            t!("popup.editor.title"),
            Some(palette.editor_border),
            palette,
        );

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

impl Popup for EditorPopup {
    fn is_open(&self) -> bool {
        EditorPopup::is_open(self)
    }
    fn close(&mut self) {
        EditorPopup::close(self)
    }
    fn contains(&self, column: u16, row: u16) -> bool {
        EditorPopup::contains(self, column, row)
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // E or Esc closes the pop-up (on a press, like every close key)
        if key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Esc | KeyCode::Char('e' | 'E'))
        {
            EditorPopup::close(self);
            return;
        }
        EditorPopup::handle_key(self, key, sim);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        EditorPopup::draw(self, f, ctx);
    }
}
