use super::*;
use rust_i18n::t;

const MAX_PATH_INPUT: usize = 256;

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

/// The save pop-up: a single name field; Enter writes the current machine
/// state to `<dir>/<field>`.
pub struct SavePopup {
    pub(super) frame: PopupFrame,
    /// The folder names are resolved against (the open file's folder).
    pub(super) dir: PathBuf,
    pub(super) buffer: String,
    pub(super) message: String,
}

impl SavePopup {
    pub fn new() -> Self {
        SavePopup {
            frame: PopupFrame::new(),
            dir: PathBuf::from("."),
            buffer: String::new(),
            message: String::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.frame.is_open()
    }

    /// Open the pop-up, resetting the input and message.
    ///
    /// When the machine was loaded from a file (`source`), the target folder
    /// is that file's folder — canonicalized, so it is absolute — and the
    /// `FILE:` field is prefilled with `<name>_edited.<ext>` (e.g.
    /// `factorct.asm` → `factorct_edited.asm`). Without a source file, the
    /// folder is the current working directory and the field is empty.
    pub fn open(&mut self, source: Option<&Path>) {
        self.frame.open = true;
        self.message.clear();
        let dir = match source {
            Some(path) => {
                // A bare file name has an empty parent; use "." then
                path.parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| PathBuf::from("."))
            }
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        // Canonicalize so the target folder is shown and saved as absolute
        self.dir = std::fs::canonicalize(&dir).unwrap_or(dir);
        self.buffer = match source {
            Some(path) => default_save_name(path),
            None => String::new(),
        };
    }

    pub fn close(&mut self) {
        self.frame.close();
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.frame.contains(column, row)
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
                self.save(sim);
            }
            KeyCode::Char(c) => {
                // A digit typed with Shift on a French AZERTY keyboard arrives
                // as the unshifted top-row char + SHIFT; translate it first
                let c = digit_of(&key).unwrap_or(c);
                // Whitelist only: alphanumerics, dash, underscore, dot. With no
                // path separator accepted, the name can never escape the
                // target folder (`..` alone just fails to write, see below)
                if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                    if self.buffer.chars().count() < MAX_PATH_INPUT {
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

    /// Write the machine state to `<dir>/<buffer>` and report the outcome.
    fn save(&mut self, sim: &mut Simulator) {
        let name = self.buffer.trim();
        if name.is_empty() {
            self.message = ts!("popup.save.type_name");
            return;
        }
        // The input whitelist admits no path separator, so the joined path
        // always stays inside the target folder
        let path = self.dir.join(name);
        match sim.save(&path) {
            Ok(()) => self.message = ts!("popup.save.saved_to", path = path.display().to_string()),
            Err(e) => self.message = ts!("popup.save.error", error = e.to_string()),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let explanation = t!("popup.save.explanation");
        let dir_line = t!("popup.save.dir", dir = self.dir.display().to_string());
        let footer = t!("popup.save.footer");
        let input = t!("popup.save.file", buffer = self.buffer.as_str());

        let content_width = display_width(&explanation)
            .max(display_width(&dir_line))
            .max(display_width(&footer))
            .max(display_width(&input))
            .max(display_width(&self.message));

        let width = popup_width(content_width);
        // Title in border. Interior: explanation, dir, blank, input, message,
        // footer = 6. Plus 2 border rows.
        let height = 8u16;

        let rect = popup_rect(height, width, area);
        let content = self.frame.begin_popup(
            f,
            rect,
            t!("popup.save.title"),
            Some(palette.save_border),
            palette,
        );

        let lines: Vec<Line<'static>> = vec![
            Line::from(explanation.to_string()),
            Line::from(Span::styled(
                dir_line,
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

        f.render_widget(Paragraph::new(lines), content);
    }
}

impl Popup for SavePopup {
    fn is_open(&self) -> bool {
        SavePopup::is_open(self)
    }
    fn close(&mut self) {
        SavePopup::close(self)
    }
    fn contains(&self, column: u16, row: u16) -> bool {
        SavePopup::contains(self, column, row)
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        // Esc closes the pop-up (on a press); every other key types into the
        // file name
        if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
            SavePopup::close(self);
            return;
        }
        SavePopup::handle_key(self, key, sim);
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        SavePopup::draw(self, f, ctx);
    }
}
