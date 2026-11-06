use super::*;
use rust_i18n::t;

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
///
/// While open, the picker is modal: `App` routes all key and mouse events
/// to it.
pub struct PickerPopup {
    pub(super) open: bool,
    pub(super) kind: PickerKind,
    pub(super) address: usize,
    /// The highlighted entry (arrow keys move it, Enter applies it).
    pub(super) selected: usize,
    /// Screen row the list is anchored on (the double-clicked row).
    pub(super) anchor_row: u16,
    /// The entry cells from the last draw, for mouse hit-testing; the
    /// position in the vector is the entry index.
    pub(super) entry_cells: Vec<Rect>,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    pub(super) last_rect: Rect,
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
            last_rect: Rect::default(),
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

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
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
    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let store = &ctx.snap.store;
        let anchor_col = ctx.picker_anchor_col;
        let (cols, entry_width, title) = match self.kind {
            PickerKind::Mnemonic => (
                1usize,
                6u16,
                t!("popup.picker.mnemonic_title", address = self.address : {:02}),
            ),
            PickerKind::Data => (
                8,
                4u16,
                t!("popup.picker.data_title", address = self.address : {:02}),
            ),
        };
        let len = self.entry_count();
        let rows = len.div_ceil(cols);
        let footer = t!("popup.picker.footer");

        let content_width = cols * entry_width as usize;
        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width
            .max(display_width(&footer))
            .max(display_width(&title))
            + 4) as u16;
        // Entries + footer row, plus the 2 border rows
        let height = (rows + 3) as u16;

        let rect = side_rect(width, height, area, anchor_col, self.anchor_row);
        self.last_rect = rect;
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

impl Popup for PickerPopup {
    fn is_open(&self) -> bool {
        PickerPopup::is_open(self)
    }
    fn close(&mut self) {
        PickerPopup::close(self)
    }
    fn contains(&self, column: u16, row: u16) -> bool {
        PickerPopup::contains(self, column, row)
    }
    fn handles_own_clicks(&self) -> bool {
        true
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Esc cancels, on press and repeat (the picker is modal)
        if key.code == KeyCode::Esc {
            PickerPopup::close(self);
            return;
        }
        PickerPopup::handle_key(self, key, sim);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        PickerPopup::draw(self, f, ctx);
    }
}
