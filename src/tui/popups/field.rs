use super::*;
use rust_i18n::t;

/// The maximum file-name length accepted in the save pop-up, in characters.
const MAX_NAME_LEN: usize = 256;

/// The proposed save name for a loaded file: `<stem>_edited.<ext>`
/// (e.g. `samples/factorct.asm` → `factorct_edited.asm`). A stem that
/// already ends with `_edited` is kept as-is, so re-saving an edited file
/// does not pile up suffixes (`factorct_edited.asm` stays `factorct_edited.asm`).
pub(super) fn default_save_name(path: &Path) -> String {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("state");
    let stem = if stem.ends_with("_edited") {
        stem.to_string()
    } else {
        format!("{stem}_edited")
    };
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => format!("{stem}.{ext}"),
        None => stem,
    }
}

/// The save pop-up: a single-field input for the file name, written into
/// the target folder — a buffer with a character whitelist, Enter submits,
/// and the bordered draw. Esc is closed centrally by `App::route_popup_key`.
pub struct FieldPopup {
    pub(super) frame: PopupFrame,
    pub(super) buffer: String,
    pub(super) message: String,
    /// The target folder (canonicalized, so absolute).
    pub(super) dir: PathBuf,
}

impl FieldPopup {
    pub fn new() -> Self {
        FieldPopup {
            frame: PopupFrame::new(),
            buffer: String::new(),
            message: String::new(),
            dir: PathBuf::from("."),
        }
    }

    /// Open the pop-up, resetting the input and message. The target folder
    /// is the source file's folder — canonicalized, so it is absolute —
    /// and the field is prefilled with `<name>_edited.<ext>`; without a
    /// source file, the folder is the cwd and the field is empty.
    pub fn open(&mut self, source: Option<&Path>) {
        self.frame.open = true;
        self.message.clear();
        let dir = match source {
            Some(path) => parent_dir_of(path),
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        // Canonicalize so the target folder is shown and saved as absolute
        self.dir = canonical_dir(dir);
        self.buffer = source.map_or_else(String::new, default_save_name);
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
                self.submit(sim);
            }
            KeyCode::Char(c) => {
                // A digit typed with Shift on a French AZERTY keyboard arrives
                // as the unshifted top-row char + SHIFT; translate it first
                let c = digit_of(&key).unwrap_or(c);
                let allowed = c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
                if allowed {
                    if self.buffer.chars().count() < MAX_NAME_LEN {
                        self.buffer.push(c);
                        self.message.clear();
                    }
                } else if !c.is_control() {
                    self.message = ts!("popup.save.chars_allowed");
                }
            }
            _ => {}
        }
    }

    /// Submit the field: write the machine state to `<dir>/<input>` and
    /// report the outcome in the message.
    fn submit(&mut self, sim: &mut Simulator) {
        let input = self.buffer.trim();
        if input.is_empty() {
            self.message = ts!("popup.save.type_name");
            return;
        }
        // The input whitelist admits no path separator, so the joined
        // path always stays inside the target folder
        let path = self.dir.join(input);
        self.message = match sim.save(&path) {
            Ok(()) => ts!("popup.save.saved_to", path = path.display().to_string()),
            Err(e) => ts!("popup.save.error", error = e.to_string()),
        };
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let palette = ctx.palette;
        let explanation = t!("popup.save.explanation");
        let dir_line = t!("popup.save.dir", dir = self.dir.display().to_string());
        let footer = t!("popup.save.footer");
        let input = t!("popup.save.file", buffer = self.buffer.as_str());

        let lines: Vec<Line<'static>> = vec![
            Line::from(explanation.to_string()),
            Line::from(Span::styled(
                dir_line.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
            Line::from(""),
            Line::from(input),
            Line::from(self.message.clone()),
            Line::from(Span::styled(
                footer.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )),
        ];

        // Measure the lines as drawn (the sum of their spans' widths), so the
        // pop-up is exactly wide enough for its content
        let content_width = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| display_width(span.content.as_ref()))
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        let width = popup_width(content_width);
        // Plus the 2 border rows
        let height = lines.len() as u16 + 2;

        let rect = popup_rect(height, width, ctx.area);
        let content = self.frame.begin_popup(
            f,
            rect,
            t!("popup.save.title"),
            Some(palette.save_border),
            palette,
        );

        f.render_widget(Paragraph::new(lines), content);
    }
}

impl Popup for FieldPopup {
    fn frame(&self) -> &PopupFrame {
        &self.frame
    }
    fn frame_mut(&mut self) -> &mut PopupFrame {
        &mut self.frame
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) -> PopupEvent {
        FieldPopup::handle_key(self, key, sim);
        PopupEvent::None
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        FieldPopup::draw(self, f, ctx);
    }
}
