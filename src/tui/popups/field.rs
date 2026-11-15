use super::*;
use rust_i18n::t;

/// Which single-field file pop-up a [`FieldPopup`] is: save (a file name,
/// written into a folder) or open (a path, loaded into the machine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    Save,
    Open,
}

impl FieldKind {
    /// The i18n key prefix of the pop-up; keys are resolved dynamically under
    /// it with `t!(format!(...))` (the locale is fixed at startup).
    fn prefix(self) -> &'static str {
        match self {
            FieldKind::Save => "popup.save",
            FieldKind::Open => "popup.open",
        }
    }

    /// Whether `/` is accepted in the input (a path, not a file name).
    fn allow_slash(self) -> bool {
        matches!(self, FieldKind::Open)
    }

    /// The maximum input length, in characters.
    fn max_len(self) -> usize {
        match self {
            FieldKind::Save => 256,
            FieldKind::Open => 1024,
        }
    }
}

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

/// A single-field input pop-up — the shared skeleton of the save and open
/// pop-ups: a buffer with a character whitelist, Enter submits, and the
/// bordered draw. The [`FieldKind`] picks the i18n keys, the `/` whitelist
/// bit, the length cap, the prefill, the extra dir line (save only), and
/// the submit action. Esc is closed centrally by `App::route_popup_key`.
pub struct FieldPopup {
    pub(super) frame: PopupFrame,
    pub(super) buffer: String,
    pub(super) message: String,
    kind: FieldKind,
    /// The save pop-up's target folder (the open file's folder,
    /// canonicalized); unused by the open pop-up.
    pub(super) dir: PathBuf,
}

impl FieldPopup {
    pub fn new(kind: FieldKind) -> Self {
        FieldPopup {
            frame: PopupFrame::new(),
            buffer: String::new(),
            message: String::new(),
            kind,
            dir: PathBuf::from("."),
        }
    }

    /// The typed input (tests only).
    #[cfg(test)]
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// The last message (tests only).
    #[cfg(test)]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Open the pop-up, resetting the input and message.
    ///
    /// Save: the target folder is the source file's folder — canonicalized,
    /// so it is absolute — and the field is prefilled with `<name>_edited.<ext>`;
    /// without a source file, the folder is the cwd and the field is empty.
    /// Open: the field is prefilled with the source file's path, if any.
    pub fn open(&mut self, source: Option<&Path>) {
        self.frame.open = true;
        self.message.clear();
        match self.kind {
            FieldKind::Save => {
                let dir = match source {
                    // A bare file name has an empty parent; use "." then
                    Some(path) => path
                        .parent()
                        .filter(|p| !p.as_os_str().is_empty())
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| PathBuf::from(".")),
                    None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                };
                // Canonicalize so the target folder is shown and saved as absolute
                self.dir = std::fs::canonicalize(&dir).unwrap_or(dir);
                self.buffer = source.map_or_else(String::new, default_save_name);
            }
            FieldKind::Open => {
                self.buffer = source.map_or_else(String::new, |p| p.display().to_string());
            }
        }
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
                let allowed = c.is_ascii_alphanumeric()
                    || matches!(c, '-' | '_' | '.')
                    || (self.kind.allow_slash() && c == '/');
                if allowed {
                    if self.buffer.chars().count() < self.kind.max_len() {
                        self.buffer.push(c);
                        self.message.clear();
                    }
                } else if !c.is_control() {
                    self.message = ts!(format!("{}.chars_allowed", self.kind.prefix()));
                }
            }
            _ => {}
        }
    }

    /// Submit the field. Save writes the machine state to `<dir>/<input>` and
    /// reports the outcome in the message; open loads the file, closing the
    /// pop-up on success and showing the error otherwise.
    fn submit(&mut self, sim: &mut Simulator) {
        let prefix = self.kind.prefix();
        let input = self.buffer.trim();
        if input.is_empty() {
            self.message = ts!(format!("{prefix}.type_name"));
            return;
        }
        match self.kind {
            FieldKind::Save => {
                // The input whitelist admits no path separator, so the joined
                // path always stays inside the target folder
                let path = self.dir.join(input);
                self.message = match sim.save(&path) {
                    Ok(()) => ts!(
                        format!("{prefix}.saved_to"),
                        path = path.display().to_string()
                    ),
                    Err(e) => ts!(format!("{prefix}.error"), error = e.to_string()),
                };
            }
            FieldKind::Open => match sim.load_file(Path::new(input)) {
                Ok(()) => self.close(),
                Err(e) => self.message = ts!(format!("{prefix}.error"), error = e.to_string()),
            },
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let palette = ctx.palette;
        let prefix = self.kind.prefix();
        let explanation = t!(format!("{prefix}.explanation"));
        let footer = t!(format!("{prefix}.footer"));
        let input = t!(format!("{prefix}.file"), buffer = self.buffer.as_str());

        let mut lines: Vec<Line<'static>> = vec![Line::from(explanation.to_string())];
        // The save pop-up shows its target folder under the explanation
        if self.kind == FieldKind::Save {
            let dir_line = t!(
                format!("{prefix}.dir"),
                dir = self.dir.display().to_string()
            );
            lines.push(Line::from(Span::styled(
                dir_line.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(input));
        lines.push(Line::from(self.message.clone()));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

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
            t!(format!("{prefix}.title")),
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
