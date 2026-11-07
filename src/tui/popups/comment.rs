use super::*;

const MAX_COMMENT_INPUT: usize = MAX_COMMENT_WIDTH;

/// The inline editor for a store word's source comment.
///
/// Opened by double-clicking the comment column of a store row; the text
/// box is drawn inline in the column on that row (not as a centered
/// pop-up).
///
/// While open, all key events are routed here by the main event loop;
/// repeats are accepted so holding Backspace deletes several characters.
pub struct CommentEditor {
    pub(super) frame: PopupFrame,
    pub(super) address: usize,
    pub(super) buffer: String,
}

impl CommentEditor {
    pub fn new() -> Self {
        CommentEditor {
            frame: PopupFrame::new(),
            address: 0,
            buffer: String::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.frame.is_open()
    }

    /// Record the text box's screen rectangle (called by the draw code,
    /// which knows where the box is drawn).
    pub fn set_last_rect(&mut self, rect: Rect) {
        self.frame.last_rect = rect;
    }

    /// Whether the given screen cell lies inside the text box.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.frame.contains(column, row)
    }

    /// The text typed so far.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Display width the inline editor needs in the comment column: the
    /// buffer plus one cell for the cursor, so the text box stays fully
    /// visible while typing. Zero when the editor is closed.
    pub fn edit_width(&self) -> usize {
        if self.frame.is_open() {
            (self.buffer.chars().count() + 1).min(MAX_COMMENT_WIDTH)
        } else {
            0
        }
    }

    /// Whether the editor is open on the given store word (that row draws
    /// the inline text box instead of the comment).
    pub fn is_open_for(&self, index: usize) -> bool {
        self.frame.is_open() && self.address == index
    }

    /// Open on the given word, prefilled with its current comment.
    pub fn open_for(&mut self, address: usize, current: Option<&str>) {
        self.frame.open = true;
        self.address = address;
        self.buffer = current.unwrap_or_default().to_string();
    }

    /// Close without applying (Esc).
    pub fn close(&mut self) {
        self.frame.close();
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

impl Popup for CommentEditor {
    fn is_open(&self) -> bool {
        CommentEditor::is_open(self)
    }
    fn close(&mut self) {
        CommentEditor::close(self)
    }
    fn contains(&self, column: u16, row: u16) -> bool {
        CommentEditor::contains(self, column, row)
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Esc cancels (on a press, like every close key)
        if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
            CommentEditor::close(self);
            return;
        }
        CommentEditor::handle_key(self, key, sim);
    }
    // No `draw`: the text box is drawn inline in the comment column.
}
