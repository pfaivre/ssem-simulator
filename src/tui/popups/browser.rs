use super::*;
use rust_i18n::t;

use crate::ssem::is_loadable_file;

/// The maximum number of directory entries shown at once; longer lists
/// scroll so the selection stays visible.
const MAX_LIST_ROWS: usize = 20;

/// One row of the file browser: the `..` parent entry, a subdirectory, or a
/// loadable file (`.asm`/`.snp`).
#[derive(Clone)]
struct Entry {
    /// Display name (`..` for the parent entry).
    name: String,
    /// The path to enter (directory) or load (file).
    path: PathBuf,
    is_dir: bool,
}

/// The open pop-up: a file browser over the directory tree. It lists the
/// current folder's subdirectories (with a trailing `/`, sorted first) and
/// the loadable files (`.asm`/`.snp`), plus a `..` entry to go up; hidden
/// entries are skipped. Arrows move the highlight, Enter opens the
/// highlighted file or enters the highlighted directory, Left/Backspace
/// goes up; the same works with mouse clicks. Esc is closed centrally by
/// `App::route_popup_key`.
///
/// While open, the browser is modal: `App` routes all key and mouse events
/// to it.
pub struct OpenPopup {
    pub(super) frame: PopupFrame,
    /// The folder currently listed (canonicalized, so absolute).
    dir: PathBuf,
    entries: Vec<Entry>,
    /// The highlighted entry.
    selected: usize,
    /// The first entry shown (scroll offset).
    scroll: usize,
    /// How many entry rows fit in the last drawn content area.
    view_rows: usize,
    /// The first entry drawn in the last draw (entry_cells index base).
    list_start: usize,
    message: String,
    /// The entry rows from the last draw, for mouse hit-testing.
    entry_cells: Vec<Rect>,
}

impl OpenPopup {
    pub fn new() -> Self {
        OpenPopup {
            frame: PopupFrame::new(),
            dir: PathBuf::from("."),
            entries: Vec::new(),
            selected: 0,
            scroll: 0,
            // Until the first draw computes the real value, assume the
            // whole capped list fits, so refresh() does not scroll
            view_rows: MAX_LIST_ROWS,
            list_start: 0,
            message: String::new(),
            entry_cells: Vec::new(),
        }
    }

    /// The folder currently listed (tests only).
    #[cfg(test)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The highlighted entry's display name (tests only).
    #[cfg(test)]
    pub fn selected_name(&self) -> Option<&str> {
        self.entries.get(self.selected).map(|e| e.name.as_str())
    }

    /// Every entry's display name, in list order (tests only).
    #[cfg(test)]
    pub fn entry_names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// The last message (tests only).
    #[cfg(test)]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Open the browser. With a source file, start in its folder with the
    /// file highlighted; without one, start in the current working
    /// directory.
    pub fn open(&mut self, source: Option<&Path>) {
        self.frame.open = true;
        self.message.clear();
        let (dir, preselect) = match source {
            Some(path) => (
                parent_dir_of(path),
                path.file_name().map(|n| n.to_os_string()),
            ),
            None => (
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
                None,
            ),
        };
        self.dir = canonical_dir(dir);
        self.refresh(preselect.as_deref());
    }

    /// Re-read the current folder into `entries`: the `..` parent entry
    /// first (when there is one), then subdirectories, then loadable files,
    /// each group sorted alphabetically (case-insensitive). Hidden entries
    /// (names starting with `.`) are skipped. `preselect` highlights the
    /// entry with that name, if present.
    fn refresh(&mut self, preselect: Option<&std::ffi::OsStr>) {
        self.entries.clear();
        if let Some(parent) = self.dir.parent() {
            self.entries.push(Entry {
                name: "..".to_string(),
                path: parent.to_path_buf(),
                is_dir: true,
            });
        }
        match std::fs::read_dir(&self.dir) {
            Ok(read_dir) => {
                let mut dirs: Vec<Entry> = Vec::new();
                let mut files: Vec<Entry> = Vec::new();
                for entry in read_dir.flatten() {
                    let name = entry.file_name();
                    if name.to_str().is_some_and(|n| n.starts_with('.')) {
                        continue;
                    }
                    let path = entry.path();
                    let is_dir = path.is_dir();
                    // Only loadable files pass the filter; directories all do
                    if !is_dir && !is_loadable_file(&path) {
                        continue;
                    }
                    let entry = Entry {
                        name: name.to_string_lossy().into_owned(),
                        path,
                        is_dir,
                    };
                    (if is_dir { &mut dirs } else { &mut files }).push(entry);
                }
                let by_name =
                    |a: &Entry, b: &Entry| a.name.to_lowercase().cmp(&b.name.to_lowercase());
                dirs.sort_by(by_name);
                files.sort_by(by_name);
                self.entries.extend(dirs);
                self.entries.extend(files);
            }
            Err(e) => {
                self.message = ts!("popup.open.error", error = e.to_string());
            }
        }
        self.selected = preselect
            .and_then(|n| n.to_str())
            .and_then(|n| self.entries.iter().position(|e| e.name == n))
            .unwrap_or(0);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        self.scroll = 0;
        self.ensure_visible();
    }

    /// Keep the highlight inside the visible window.
    fn ensure_visible(&mut self) {
        let view = self.view_rows.max(1);
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if self.selected >= self.scroll + view {
            self.scroll = self.selected + 1 - view;
        }
    }

    /// Scroll the list view by `delta` rows (negative scrolls up), clamped
    /// to the list; the highlight is kept inside the visible window.
    fn scroll_by(&mut self, delta: isize) {
        let view = self.view_rows.max(1);
        let max_scroll = self.entries.len().saturating_sub(view);
        self.scroll = (self.scroll as isize + delta).clamp(0, max_scroll as isize) as usize;
        if self.selected < self.scroll {
            self.selected = self.scroll;
        }
        if self.selected >= self.scroll + view {
            self.selected = self.scroll + view - 1;
        }
    }

    /// Go up one folder, highlighting the folder just left.
    fn go_up(&mut self) {
        let Some(parent) = self.dir.parent() else {
            return;
        };
        let back = self.dir.file_name().map(|n| n.to_os_string());
        self.dir = parent.to_path_buf();
        self.message.clear();
        self.refresh(back.as_deref());
    }

    /// Activate the highlighted entry: enter a directory, or load a file
    /// (closing the pop-up on success, showing the error otherwise).
    fn activate_selected(&mut self, sim: &mut Simulator) {
        let Some(entry) = self.entries.get(self.selected).cloned() else {
            return;
        };
        if entry.is_dir {
            self.dir = canonical_dir(entry.path);
            self.message.clear();
            self.refresh(None);
        } else {
            match sim.load_file(&entry.path) {
                Ok(()) => self.close(),
                Err(e) => self.message = ts!("popup.open.error", error = e.to_string()),
            }
        }
    }

    /// Handle a key press while the browser is open: the arrows (and
    /// Home/End/PageUp/PageDown) move the highlight, Enter/Right activate
    /// the highlighted entry, Left/Backspace go up. Repeats are accepted so
    /// holding a key moves quickly.
    pub fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) {
        if key.kind != KeyEventKind::Press && key.kind != KeyEventKind::Repeat {
            return;
        }
        let len = self.entries.len();
        match key.code {
            KeyCode::Up => {
                self.selected = self.selected.saturating_sub(1);
                self.ensure_visible();
            }
            KeyCode::Down => {
                if self.selected + 1 < len {
                    self.selected += 1;
                }
                self.ensure_visible();
            }
            KeyCode::Home => {
                self.selected = 0;
                self.ensure_visible();
            }
            KeyCode::End => {
                self.selected = len.saturating_sub(1);
                self.ensure_visible();
            }
            KeyCode::PageUp => {
                self.selected = self.selected.saturating_sub(self.view_rows.max(1));
                self.ensure_visible();
            }
            KeyCode::PageDown => {
                if len > 0 {
                    self.selected = (self.selected + self.view_rows.max(1)).min(len - 1);
                }
                self.ensure_visible();
            }
            KeyCode::Left | KeyCode::Backspace => self.go_up(),
            KeyCode::Right | KeyCode::Enter => self.activate_selected(sim),
            _ => {}
        }
    }

    /// The entry drawn under the given screen cell, if any.
    pub fn entry_at(&self, column: u16, row: u16) -> Option<usize> {
        self.entry_cells
            .iter()
            .position(|r| r.contains(Position::new(column, row)))
            .map(|pos| self.list_start + pos)
    }

    /// Select and activate the given entry (mouse click).
    fn select_and_activate(&mut self, entry: usize, sim: &mut Simulator) {
        self.selected = entry;
        self.activate_selected(sim);
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let area = ctx.area;
        let palette = ctx.palette;
        let title = t!("popup.open.title");
        let dir_text = self.dir.display().to_string();
        let total = self.entries.len();
        let footer = t!(
            "popup.open.footer",
            shown = total.min(MAX_LIST_ROWS),
            total = total
        );
        let empty = t!("popup.open.empty");
        let has_files = self.entries.iter().any(|e| !e.is_dir);

        // The widest list row (" > name/"), the title with the current path,
        // the hint, the message, and the footer all size the pop-up
        let name_width = self
            .entries
            .iter()
            .map(|e| display_width(&e.name) + usize::from(e.is_dir))
            .max()
            .unwrap_or(0);
        let content_width = (name_width + 2)
            .max(display_width(&footer))
            .max(display_width(&self.message))
            .max(if has_files { 0 } else { display_width(&empty) })
            .max(display_width(&title) + display_width(&dir_text));
        let width = popup_width(content_width);
        // Entry rows (capped) + hint (when no files) + message + footer,
        // plus the 2 border rows
        let visible = self.entries.len().min(MAX_LIST_ROWS);
        let height = (visible + 2 + usize::from(!has_files)) as u16 + 2;

        let rect = popup_rect(height, width, area);
        let content = self.frame.begin_popup(
            f,
            rect,
            format!("{title}{dir_text}"),
            Some(palette.save_border),
            palette,
        );

        // The entry rows that fit above the hint/message/footer lines
        let non_list = (2 + usize::from(!has_files)) as u16;
        let view_rows = content.height.saturating_sub(non_list) as usize;
        self.view_rows = view_rows.max(1);
        self.ensure_visible();

        let selected_style = Style::default()
            .fg(palette.selection)
            .add_modifier(Modifier::REVERSED);
        let dir_style = Style::default().add_modifier(Modifier::BOLD);

        self.entry_cells.clear();
        self.list_start = self.scroll.min(self.entries.len());
        let mut lines: Vec<Line<'static>> = Vec::new();
        for (i, entry) in self
            .entries
            .iter()
            .enumerate()
            .skip(self.list_start)
            .take(view_rows)
        {
            let pointer = if i == self.selected { ">" } else { " " };
            let label = if entry.is_dir {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };
            let style = if i == self.selected {
                selected_style
            } else if entry.is_dir {
                dir_style
            } else {
                Style::default()
            };
            lines.push(Line::from(Span::styled(
                format!("{pointer} {label}"),
                style,
            )));
            self.entry_cells.push(Rect {
                x: content.x,
                y: content.y + (i - self.list_start) as u16,
                width: content.width,
                height: 1,
            });
        }
        if !has_files {
            lines.push(Line::from(Span::styled(
                empty.to_string(),
                Style::default().add_modifier(Modifier::DIM),
            )));
        }
        lines.push(Line::from(self.message.clone()));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

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
    fn handles_own_clicks(&self) -> bool {
        true
    }
    fn is_modal(&self) -> bool {
        true
    }
    fn handle_key(&mut self, key: KeyEvent, sim: &mut Simulator) -> PopupEvent {
        // Esc cancels centrally (App::route_popup_key)
        OpenPopup::handle_key(self, key, sim);
        PopupEvent::None
    }
    fn handle_click(&mut self, column: u16, row: u16, sim: &mut Simulator) -> PopupEvent {
        // A click on an entry activates it; any other click closes the
        // browser without changes
        match self.entry_at(column, row) {
            Some(entry) => self.select_and_activate(entry, sim),
            None => self.close(),
        }
        PopupEvent::None
    }
    fn handle_scroll(&mut self, up: bool) {
        // One row per wheel notch, like the help pop-up and the store view
        self.scroll_by(if up { -1 } else { 1 });
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        OpenPopup::draw(self, f, ctx);
    }
}
