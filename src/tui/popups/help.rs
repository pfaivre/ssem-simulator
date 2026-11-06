use super::*;
use rust_i18n::t;

pub struct HelpPopup {
    pub(super) open: bool,
    pub(super) scroll: usize,
    /// Maximum scroll offset, refreshed on every draw so key handling can clamp.
    pub(super) max_scroll: usize,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    pub(super) last_rect: Rect,
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

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let keymap = keymap();
        let key_column = keymap
            .iter()
            .map(|(key, _, _)| key.chars().count())
            .max()
            .unwrap_or(0)
            + 2;

        let title = t!("help.title", version = env!("CARGO_PKG_VERSION"));
        let footer = t!("help.footer_close");
        let footer_scrollable = t!("help.footer_scroll");

        let key_style = Style::default()
            .fg(palette.key_hint)
            .add_modifier(Modifier::REVERSED);

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Blank line after the border title
        lines.push(Line::from(""));

        // One line per key
        for (key, _, description) in keymap {
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
        lines.push(Line::from(t!("help.mnemonic_colors")));
        for (mnemonics, opcode, description) in [
            ("JMP JRP", Opcode::JMP, t!("help.op_jump")),
            ("LDN STO", Opcode::LDN, t!("help.op_memory")),
            ("SUB SUB2", Opcode::SUB, t!("help.op_arithmetic")),
            ("CMP", Opcode::CMP, t!("help.op_test")),
            ("STP", Opcode::STP, t!("help.op_halt")),
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
        lines.push(Line::from(t!("help.markers")));
        lines.push(Line::from(vec![
            Span::raw(t!("help.marker_last")),
            Span::styled(
                "\u{bb}",
                Style::default()
                    .fg(palette.next_marker)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(t!("help.marker_next")),
            Span::styled("\u{2022}", Style::default().fg(palette.dim)),
            Span::raw(t!("help.marker_executed")),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from(t!("help.heatmap_title")));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                t!("mode.heatmap_exec"),
                Style::default()
                    .fg(palette.exec_ramp[2])
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(t!("help.heatmap_exec_desc")),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                t!("help.heatmap_r"),
                Style::default()
                    .fg(palette.read_ramp[1])
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("/"),
            Span::styled(
                t!("help.heatmap_w"),
                Style::default()
                    .fg(palette.write_ramp[2])
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(t!("help.heatmap_rw_desc")),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from(t!("help.graph_title")));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                t!("help.graph_jump"),
                Style::default().fg(palette.edge_jump),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                t!("help.graph_skip"),
                Style::default().fg(palette.edge_skip),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(t!("help.graph_halt"), Style::default().fg(palette.op_halt)),
        ]));

        // Blank line before the footer, which is pinned to the bottom row
        lines.push(Line::from(""));

        let content_width = display_width(&title)
            .max(display_width(&footer))
            .max(display_width(&footer_scrollable))
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

impl Popup for HelpPopup {
    fn is_open(&self) -> bool {
        HelpPopup::is_open(self)
    }
    fn close(&mut self) {
        HelpPopup::close(self)
    }
    fn contains(&self, column: u16, row: u16) -> bool {
        HelpPopup::contains(self, column, row)
    }
    fn handle_key(&mut self, key: KeyEvent, _sim: &mut Simulator) {
        // H toggles the pop-up closed (on a press, like every close key)
        if key.kind == KeyEventKind::Press
            && matches!(key.code, KeyCode::Char('h' | 'H') | KeyCode::Esc)
        {
            HelpPopup::close(self);
            return;
        }
        HelpPopup::handle_key(self, key);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        HelpPopup::draw(self, f, ctx);
    }
}
