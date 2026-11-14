use super::*;
use rust_i18n::t;

/// The destructive action a confirmation pop-up guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Clear the machine (store, registers, history, breakpoints).
    Clear,
}

/// A button of the confirmation pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmButton {
    Yes,
    No,
}

/// A confirmation pop-up shown before a destructive menu action: a question
/// with [Yes]/[No] buttons. Confirming runs the pending action; canceling
/// (No, N, ESC, or a click anywhere but Yes) just closes the pop-up.
pub struct ConfirmPopup {
    pub(super) frame: PopupFrame,
    /// The action to run when the user confirms.
    pub(super) action: ConfirmAction,
    /// The buttons' cells from the last draw: (rect, button).
    pub(super) buttons: Vec<(Rect, ConfirmButton)>,
    /// The button under the mouse cursor, for the hover highlight.
    pub(super) hover: Option<ConfirmButton>,
}

impl ConfirmPopup {
    pub fn new() -> Self {
        ConfirmPopup {
            frame: PopupFrame::new(),
            action: ConfirmAction::Clear,
            buttons: Vec::new(),
            hover: None,
        }
    }

    /// Open the pop-up guarding `action`.
    pub fn open(&mut self, action: ConfirmAction) {
        self.frame.open = true;
        self.action = action;
        self.hover = None;
    }

    /// Run the pending action and close the pop-up.
    pub fn confirm(&mut self, sim: &mut Simulator) {
        match self.action {
            ConfirmAction::Clear => sim.clear(),
        }
        self.close();
    }

    /// The button drawn under the given screen cell, if any.
    pub fn button_at(&self, column: u16, row: u16) -> Option<ConfirmButton> {
        self.buttons
            .iter()
            .find(|(rect, _)| rect.contains(Position::new(column, row)))
            .map(|(_, button)| *button)
    }

    /// Record the button under the mouse cursor (for the hover highlight).
    pub fn set_hover(&mut self, hover: Option<ConfirmButton>) {
        self.hover = hover;
    }

    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Esc => self.close(),
            KeyCode::Enter => self.confirm(sim),
            KeyCode::Char(c) => match c.to_ascii_lowercase() {
                'y' => self.confirm(sim),
                'n' => self.close(),
                _ => {}
            },
            _ => {}
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let question = match self.action {
            ConfirmAction::Clear => t!("popup.confirm.question_clear"),
        };
        let yes = t!("popup.confirm.yes");
        let no = t!("popup.confirm.no");
        let footer = t!("popup.confirm.footer");

        let buttons_line = format!("{yes}  {no}");
        let content_width = display_width(&question)
            .max(display_width(&buttons_line))
            .max(display_width(&footer));
        let width = popup_width(content_width);
        // Interior: question, blank, buttons, footer = 4. Plus 2 border rows.
        let height = 6u16;

        let rect = popup_rect(height, width, area);
        let content = self.frame.begin_popup(
            f,
            rect,
            t!("popup.confirm.title"),
            Some(palette.confirm_border),
            palette,
        );

        let button_style = Style::default().fg(palette.key_hint);
        let hover_style = Style::default().fg(palette.hover_fg).bg(palette.hover);

        self.buttons.clear();
        let buttons_row = content.y + 2;
        let mut x = content.x + 1;
        let mut spans = vec![Span::raw(" ")];
        for (text, button) in [
            (yes.as_ref(), ConfirmButton::Yes),
            (no.as_ref(), ConfirmButton::No),
        ] {
            spans.push(Span::raw("  "));
            x += 2;
            let w = display_width(text) as u16;
            let style = if self.hover == Some(button) {
                hover_style
            } else {
                button_style
            };
            spans.push(Span::styled(text.to_string(), style));
            self.buttons.push((Rect::new(x, buttons_row, w, 1), button));
            x += w;
        }

        let lines: Vec<Line<'static>> = vec![
            Line::from(question.to_string()),
            Line::from(""),
            Line::from(spans),
            Line::from(Span::styled(
                footer.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];

        f.render_widget(Paragraph::new(lines), content);
    }
}

impl Popup for ConfirmPopup {
    fn frame(&self) -> &PopupFrame {
        &self.frame
    }
    fn frame_mut(&mut self) -> &mut PopupFrame {
        &mut self.frame
    }
    fn close(&mut self) {
        self.frame_mut().close();
        self.hover = None;
    }
    fn handles_own_clicks(&self) -> bool {
        true
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        ConfirmPopup::handle_key(self, key, sim);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        ConfirmPopup::draw(self, f, ctx);
    }
}
