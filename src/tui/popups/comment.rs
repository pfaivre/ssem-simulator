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
    /// Insertion point within the buffer, in chars: 0..=buffer length.
    /// Left/Right move it; typing inserts at it.
    pub(super) cursor: usize,
}

impl CommentEditor {
    pub fn new() -> Self {
        CommentEditor {
            frame: PopupFrame::new(),
            address: 0,
            buffer: String::new(),
            cursor: 0,
        }
    }

    /// Record the text box's screen rectangle (called by the draw code,
    /// which knows where the box is drawn).
    pub fn set_last_rect(&mut self, rect: Rect) {
        self.frame.last_rect = rect;
    }

    /// The text typed so far. Test-only: the draw code projects the buffer
    /// through [`visible_text`](Self::visible_text) instead.
    #[cfg(test)]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// The insertion point within the buffer, in chars.
    #[cfg(test)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// The text box shown in the comment column: the buffer with the cursor
    /// cell ('_') at the insertion point, windowed to `width` cells so the
    /// cursor stays visible when the column is narrower than the text (narrow
    /// terminal or the width cap). One cell of the window is the cursor
    /// itself.
    pub fn visible_text(&self, width: usize) -> String {
        let chars: Vec<char> = self.buffer.chars().collect();
        let window = width.max(1);
        let virtual_len = chars.len() + 1;
        let cursor = self.cursor;
        let start = cursor.min(virtual_len.saturating_sub(window));
        let end = (start + window).min(virtual_len);
        let mut visible = String::new();
        for i in start..end {
            if i == cursor {
                visible.push('_');
            } else {
                visible.push(chars[if i < cursor { i } else { i - 1 }]);
            }
        }
        visible
    }

    /// Number of characters in the buffer.
    fn len(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Byte index of the char at `char_idx` (the end of the buffer when
    /// `char_idx` is past the last character).
    fn byte_index(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map_or(self.buffer.len(), |(i, _)| i)
    }

    /// Delete the char at `char_idx`, if any (no-op past the end). The range
    /// is bounded by the next char's byte index, so it is char-boundary safe
    /// for multibyte chars.
    fn delete_char_at(&mut self, char_idx: usize) {
        if char_idx < self.len() {
            let start = self.byte_index(char_idx);
            let end = self.byte_index(char_idx + 1);
            self.buffer.replace_range(start..end, "");
        }
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

    /// Open on the given word, prefilled with its current comment. The
    /// cursor starts at the end of the text.
    pub fn open_for(&mut self, address: usize, current: Option<&str>) {
        self.frame.open = true;
        self.address = address;
        self.buffer = current.unwrap_or_default().to_string();
        self.cursor = self.len();
    }

    /// Handle a key press while the editor is open. Repeats are accepted
    /// so holding Backspace deletes several characters.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        match key.code {
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.len());
            }
            KeyCode::Home => {
                self.cursor = 0;
            }
            KeyCode::End => {
                self.cursor = self.len();
            }
            KeyCode::Up => self.move_line(sim, -1),
            KeyCode::Down => self.move_line(sim, 1),
            // Ctrl+Backspace erases the whole word before the cursor
            KeyCode::Backspace if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.delete_word();
            }
            KeyCode::Backspace => {
                self.delete_before_cursor();
            }
            KeyCode::Delete => {
                self.delete_at_cursor();
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
                if !c.is_control() && self.len() < MAX_COMMENT_INPUT {
                    self.buffer.insert(self.byte_index(self.cursor), c);
                    self.cursor += 1;
                }
            }
            _ => {}
        }
    }

    /// Up/Down: commit the current edit, then open the editor on the word
    /// above/below, with the cursor at the same position in the line
    /// (clamped when that line is shorter). At the first/last word the
    /// key is a no-op.
    fn move_line(&mut self, sim: &mut Simulator, delta: isize) {
        let target = self.address as isize + delta;
        if target < 0 || target as usize >= sim.store.comments.len() {
            return;
        }
        self.commit(sim);
        let target = target as usize;
        let cursor = self.cursor; // captured before open_for resets it to the end
        let current = sim.store.comments[target].clone();
        self.open_for(target, current.as_deref());
        self.cursor = cursor.min(self.len());
    }

    /// Delete the character at the cursor (Del), if any; the cursor stays
    /// put, so Del at the end of the line is a no-op.
    fn delete_at_cursor(&mut self) {
        self.delete_char_at(self.cursor);
    }

    /// Delete the character before the cursor, if any.
    fn delete_before_cursor(&mut self) {
        if self.cursor > 0 {
            self.delete_char_at(self.cursor - 1);
            self.cursor -= 1;
        }
    }

    /// Erase the word before the cursor (Ctrl+Backspace): the run of
    /// whitespace ending at the cursor first, then the run of
    /// non-whitespace characters, so "hello world  |" becomes "hello "
    /// in one press. Text after the cursor is kept.
    fn delete_word(&mut self) {
        let mut head: String = self.buffer.chars().take(self.cursor).collect();
        while head.chars().last().is_some_and(|c| c.is_whitespace()) {
            head.pop();
        }
        while head.chars().last().is_some_and(|c| !c.is_whitespace()) {
            head.pop();
        }
        let tail: String = self.buffer.chars().skip(self.cursor).collect();
        self.cursor = head.chars().count();
        self.buffer = format!("{head}{tail}");
    }

    /// Write the buffer to the word's comment. A blank buffer clears the
    /// comment; trailing whitespace is dropped, like the loaders do
    /// (leading indentation is kept).
    fn commit(&mut self, sim: &mut Simulator) {
        let text = self.buffer.trim_end();
        let comment = if text.trim().is_empty() {
            None
        } else {
            Some(text.to_string())
        };
        sim.store.set_comment(self.address, comment);
    }

    /// Validate the edit: write the buffer to the word's comment and close.
    fn apply(&mut self, sim: &mut Simulator) {
        self.commit(sim);
        self.close();
    }
}

impl Popup for CommentEditor {
    fn frame(&self) -> &PopupFrame {
        &self.frame
    }
    fn frame_mut(&mut self) -> &mut PopupFrame {
        &mut self.frame
    }
    fn close(&mut self) {
        self.frame_mut().close();
        self.buffer.clear();
        self.cursor = 0;
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Esc cancels (on a press, like every close key)
        if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
            self.close();
            return;
        }
        CommentEditor::handle_key(self, key, sim);
    }
    // No `draw`: the text box is drawn inline in the comment column.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor(buffer: &str, cursor: usize) -> CommentEditor {
        let mut e = CommentEditor::new();
        e.buffer = buffer.to_string();
        e.cursor = cursor;
        e
    }

    #[test]
    fn visible_text_shows_the_whole_buffer_when_it_fits() {
        assert_eq!(editor(" A = -a", 7).visible_text(10), " A = -a_");
    }

    #[test]
    fn visible_text_puts_the_cursor_mid_text() {
        assert_eq!(editor(" A = -a", 5).visible_text(10), " A = _-a");
    }

    #[test]
    fn visible_text_slides_the_window_to_keep_the_cursor_visible() {
        let buf = format!(" A = -a{}", "0123456789".repeat(2)); // 27 chars
        // Cursor at 22, width 10: chars[18..22] + cursor + chars[22..27]
        assert_eq!(editor(&buf, 22).visible_text(10), "1234_56789");
        // Cursor at the end: the tail plus the cursor
        assert_eq!(editor(&buf, 27).visible_text(10), "123456789_");
    }

    #[test]
    fn visible_text_edge_cases() {
        assert_eq!(editor("", 0).visible_text(5), "_");
        // A zero width still shows the cursor cell (guard on the parameter)
        assert_eq!(editor("abc", 3).visible_text(0), "_");
        // Cursor clamped at the start shows the head plus the cursor
        assert_eq!(editor("abc", 0).visible_text(2), "_a");
    }

    #[test]
    fn byte_index_is_correct_for_multibyte_chars() {
        let e = editor("aéb", 0); // é is 2 bytes: a=0, é=1..3, b=3
        assert_eq!(e.byte_index(0), 0);
        assert_eq!(e.byte_index(1), 1);
        assert_eq!(e.byte_index(2), 3);
        assert_eq!(e.byte_index(3), 4); // past the end -> buffer length
    }

    #[test]
    fn delete_is_char_boundary_safe_for_multibyte_chars() {
        let mut e = editor("aéb", 3); // cursor at the end
        e.delete_before_cursor(); // delete 'b'
        assert_eq!(e.buffer, "aé");
        assert_eq!(e.cursor, 2);
        e.delete_before_cursor(); // delete 'é' (multibyte)
        assert_eq!(e.buffer, "a");
        assert_eq!(e.cursor, 1);

        let mut e = editor("aéb", 1); // cursor between 'a' and 'é'
        e.delete_at_cursor(); // delete 'é'
        assert_eq!(e.buffer, "ab");
        assert_eq!(e.cursor, 1);
        e.delete_at_cursor(); // delete 'b'
        assert_eq!(e.buffer, "a");
        e.delete_at_cursor(); // at the end: no-op
        assert_eq!(e.buffer, "a");
        assert_eq!(e.cursor, 1);
    }
}
