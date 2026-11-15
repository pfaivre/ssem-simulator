use super::*;
use rust_i18n::t;

/// The destructive action a confirmation pop-up guards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Clear the machine (store, registers, history, breakpoints).
    Clear,
    /// Quit the application.
    Quit,
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

    /// The action the pop-up currently guards.
    pub fn action(&self) -> ConfirmAction {
        self.action
    }

    /// Run the pending action and close the pop-up, returning it so the
    /// caller can apply any app-level effect (Quit sets App's quit flag).
    pub fn confirm(&mut self, sim: &mut Simulator) -> ConfirmAction {
        let action = self.action;
        if action == ConfirmAction::Clear {
            sim.clear();
        }
        self.close();
        action
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

    /// Handle a key press; returns the action if it was confirmed.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) -> Option<ConfirmAction> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        match key.code {
            // Esc closes centrally (App::route_popup_key)
            KeyCode::Enter => Some(self.confirm(sim)),
            KeyCode::Char(c) => match c.to_ascii_lowercase() {
                'y' => Some(self.confirm(sim)),
                'n' => {
                    self.close();
                    None
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let question = match self.action {
            ConfirmAction::Clear => t!("popup.confirm.question_clear"),
            ConfirmAction::Quit => t!("popup.confirm.question_quit"),
        };
        let yes = t!("popup.confirm.yes");
        let no = t!("popup.confirm.no");
        let footer = t!("popup.confirm.footer");

        // The buttons row exactly as drawn below: a leading space, then each
        // button preceded by two spaces — measured so the pop-up is wide
        // enough for the row it actually renders
        let parts = [
            (yes.as_ref(), ConfirmButton::Yes),
            (no.as_ref(), ConfirmButton::No),
        ];
        let buttons_line = parts.iter().fold(String::from(" "), |line, (text, _)| {
            format!("{line}  {text}")
        });
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
        for (text, button) in parts {
            spans.push(Span::raw("  "));
            x += 2;
            let w = display_width(text) as u16;
            let style = if self.hover == Some(button) {
                hover_style
            } else {
                button_style
            };
            spans.push(Span::styled(text.to_string(), style));
            let rect = Rect::new(x, buttons_row, w, 1);
            // Record the button only where it is actually drawn: at tiny
            // terminal sizes the pop-up is clamped and the row may be
            // clipped — a click on a clipped cell must not confirm
            if content.intersection(rect) == rect {
                self.buttons.push((rect, button));
            }
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
        // The confirmed action (if any) is picked up by App, which handles
        // this pop-up itself in route_popup_key
        let _ = ConfirmPopup::handle_key(self, key, sim);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        ConfirmPopup::draw(self, f, ctx);
    }
}
