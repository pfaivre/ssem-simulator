use super::*;
use rust_i18n::t;

const MAX_PATH_INPUT: usize = 1024;

/// The open pop-up: a single path field; Enter loads the file into the
/// machine, replacing the current state. Unlike the save name, the field
/// accepts `/` so any path can be typed.
pub struct OpenPopup {
    pub(super) frame: PopupFrame,
    pub(super) buffer: String,
    pub(super) message: String,
}

impl OpenPopup {
    pub fn new() -> Self {
        OpenPopup {
            frame: PopupFrame::new(),
            buffer: String::new(),
            message: String::new(),
        }
    }

    /// Open the pop-up, resetting the input and message; the field is
    /// prefilled with the current source file's path, if any.
    pub fn open(&mut self, source: Option<&Path>) {
        self.frame.open = true;
        self.message.clear();
        self.buffer = source.map_or_else(String::new, |p| p.display().to_string());
    }

    /// The typed path (tests only).
    #[cfg(test)]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// The last message (tests only).
    #[cfg(test)]
    pub fn message(&self) -> &str {
        &self.message
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
                self.load(sim);
            }
            KeyCode::Char(c) => {
                // A digit typed with Shift on a French AZERTY keyboard arrives
                // as the unshifted top-row char + SHIFT; translate it first
                let c = digit_of(&key).unwrap_or(c);
                // Whitelist only: alphanumerics, dash, underscore, dot, slash
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/') {
                    if self.buffer.chars().count() < MAX_PATH_INPUT {
                        self.buffer.push(c);
                        self.message.clear();
                    }
                } else if !c.is_control() {
                    self.message = ts!("popup.open.chars_allowed");
                }
            }
            _ => {}
        }
    }

    /// Load the file at the typed path into the machine. On success the
    /// pop-up closes; on error the message is shown and the pop-up stays
    /// open with the machine untouched.
    fn load(&mut self, sim: &mut Simulator) {
        let path = self.buffer.trim();
        if path.is_empty() {
            self.message = ts!("popup.open.type_path");
            return;
        }
        match sim.load_file(Path::new(path)) {
            Ok(()) => self.close(),
            Err(e) => self.message = ts!("popup.open.error", error = e.to_string()),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let explanation = t!("popup.open.explanation");
        let footer = t!("popup.open.footer");
        let input = t!("popup.open.file", buffer = self.buffer.as_str());

        let content_width = display_width(&explanation)
            .max(display_width(&footer))
            .max(display_width(&input))
            .max(display_width(&self.message));

        let width = popup_width(content_width);
        // Title in border. Interior: explanation, blank, input, message,
        // footer = 5. Plus 2 border rows.
        let height = 7u16;

        let rect = popup_rect(height, width, area);
        let content = self.frame.begin_popup(
            f,
            rect,
            t!("popup.open.title"),
            Some(palette.save_border),
            palette,
        );

        let lines: Vec<Line<'static>> = vec![
            Line::from(explanation.to_string()),
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

impl Popup for OpenPopup {
    fn frame(&self) -> &PopupFrame {
        &self.frame
    }
    fn frame_mut(&mut self) -> &mut PopupFrame {
        &mut self.frame
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Esc closes the pop-up (on a press); every other key types into the
        // path
        if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
            self.close();
            return;
        }
        OpenPopup::handle_key(self, key, sim);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        OpenPopup::draw(self, f, ctx);
    }
}
