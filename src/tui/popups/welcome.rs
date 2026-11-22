use super::*;
use crate::tui::palette::blend;
use rust_i18n::t;
#[cfg(test)]
use std::time::Duration;

/// The steps of the first-run tour, in order: an intro, then one step per
/// main element, each pointing at it on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TourStep {
    /// What this app simulates, which program was loaded, the quick-start keys.
    Intro,
    /// The store: the main body with CI, the accumulator, and the 32 words.
    Store,
    /// The history: the timeline scrub bar.
    History,
    /// The Run/Stop button in the menu bar.
    Run,
}

const TOUR_STEPS: [TourStep; 4] = [
    TourStep::Intro,
    TourStep::Store,
    TourStep::History,
    TourStep::Run,
];

/// The arrow drawn on the pop-up's border, pointing at the highlighted
/// element: a screen cell and the glyph drawn on it.
struct Arrow {
    x: u16,
    y: u16,
    glyph: &'static str,
}

/// The first-run welcome pop-up, as a short tour: a succession of steps that
/// present the main elements (the store, the history, the Run button), each
/// drawn as a pop-up pointing an arrow at the highlighted element. Shown once
/// — the `welcome_seen` config flag records that it was displayed — and
/// navigated with Enter/Space/Right (next, closing on the last step) and
/// Left (back); Esc closes it from any step, and a click outside the pop-up
/// advances to the next step instead of ending the tour.
pub struct WelcomePopup {
    pub(super) frame: PopupFrame,
    /// The current tour step; reset to the intro when the pop-up opens.
    step: usize,
    /// Swallows OS key-repeat on the tour's navigation keys (its interval
    /// is zeroed in tests).
    nav_key_guard: KeyRepeatGuard,
}

/// A pop-up rect of the given size with (hint_x, hint_y) as its desired
/// top-left cell, clamped into `area` (and shrunk to fit it).
fn placed(width: u16, height: u16, hint_x: i32, hint_y: i32, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = hint_x.clamp(
        area.x as i32,
        (area.x + area.width).saturating_sub(width) as i32,
    ) as u16;
    let y = hint_y.clamp(
        area.y as i32,
        (area.y + area.height).saturating_sub(height) as i32,
    ) as u16;
    Rect::new(x, y, width, height)
}

/// Dim everything in `area` outside `spotlight` (all of it when None), so
/// the highlighted element — and the pop-up, which clears its own rect
/// right after — stands out. Each cell's foreground goes to the dim color
/// with the DIM modifier, and its background is blended halfway toward the
/// base background, so the solid bars (menu, status) recede as well while
/// keeping a hint of their block structure.
fn dim_spotlight(f: &mut Frame, area: Rect, spotlight: Option<Rect>, palette: &Palette) {
    let hole = spotlight
        .map(|r| r.intersection(area))
        .filter(|r| r.width > 0 && r.height > 0)
        .unwrap_or(Rect::new(area.x, area.y, 0, 0));
    // The up-to-four strips around the hole: above, below, left, right
    let strips = [
        Rect::new(area.x, area.y, area.width, hole.y.saturating_sub(area.y)),
        Rect::new(
            area.x,
            hole.bottom(),
            area.width,
            area.bottom().saturating_sub(hole.bottom()),
        ),
        Rect::new(area.x, hole.y, hole.x.saturating_sub(area.x), hole.height),
        Rect::new(
            hole.right(),
            hole.y,
            area.right().saturating_sub(hole.right()),
            hole.height,
        ),
    ];
    let buf = f.buffer_mut();
    for strip in strips {
        for y in strip.y..strip.bottom() {
            for x in strip.x..strip.right() {
                let cell = &mut buf[(x, y)];
                cell.fg = palette.dim;
                cell.modifier.insert(Modifier::DIM);
                cell.bg = blend(cell.bg, palette.bg, 1, 2);
            }
        }
    }
}

/// Fill `rect` with the tour color (its text in the base background color),
/// bold: the marker drawn on the element the current step presents.
fn fill_highlight(f: &mut Frame, rect: Rect, palette: &Palette) {
    f.buffer_mut().set_style(
        rect,
        Style::default()
            .fg(palette.bg)
            .bg(palette.highlight)
            .add_modifier(Modifier::BOLD),
    );
}

/// Highlight the one-row timeline bar: the whole row glows in the tour
/// color (its text in the base background color), and the bar's empty
/// track is painted in a dimmer variant of that color instead of the flat
/// background, so the bar reads as part of the highlighted row. The
/// retained fill keeps the dark foreground, contrasting on the glowing row.
fn highlight_timeline(f: &mut Frame, area: Rect, snap: &Snapshot, palette: &Palette) {
    fill_highlight(f, area, palette);
    let (bar_x, bar_width) = timeline_bar_geometry(area, snap.history_start, snap.history_head);
    if snap.history_capacity == 0 || bar_width == 0 {
        return;
    }
    let filled = timeline_filled_len(bar_width, snap.history_len, snap.history_capacity);
    let dim_highlight = blend(palette.highlight, palette.bg, 1, 2);
    let buf = f.buffer_mut();
    for y in area.y..area.bottom() {
        for x in bar_x.saturating_add(filled as u16)..bar_x.saturating_add(bar_width) {
            buf[(x, y)].fg = dim_highlight;
        }
    }
}

/// The arrow for a pop-up rect at border row `y`, or None when the rect is
/// too small to carry one (narrower than 3 cells or zero rows — a degenerate
/// terminal size): there is then no border segment between the corners to
/// replace, and no cell to draw on.
fn arrow_for(rect: Rect, x: i32, y: u16, glyph: &'static str) -> Option<Arrow> {
    if rect.width < 3 || rect.height < 1 {
        return None;
    }
    // Keep one cell away from the corners so the arrow replaces a border
    // segment, not a corner. Order-safe for any rect width.
    let lo = rect.x as i32 + 1;
    let hi = (rect.right().saturating_sub(2) as i32).max(lo);
    Some(Arrow {
        x: x.clamp(lo, hi) as u16,
        y,
        glyph,
    })
}

impl WelcomePopup {
    pub fn new() -> Self {
        WelcomePopup {
            frame: PopupFrame::new(),
            step: 0,
            nav_key_guard: KeyRepeatGuard::default(),
        }
    }

    pub fn open(&mut self) {
        self.frame.open = true;
        self.step = 0;
        self.nav_key_guard.reset();
    }

    /// The current tour step (0 = intro).
    #[cfg(test)]
    pub fn step(&self) -> usize {
        self.step
    }

    /// Set the navigation key-repeat guard (tests press keys in quick
    /// succession on purpose and zero it).
    #[cfg(test)]
    pub fn set_nav_key_guard(&mut self, guard: Duration) {
        self.nav_key_guard.set_interval(guard);
    }

    /// Advance the tour to the next step, closing it on the last step.
    fn advance(&mut self) {
        if self.step + 1 >= TOUR_STEPS.len() {
            self.close();
        } else {
            self.step += 1;
        }
    }

    /// The current tour step, clamped to the tour's range.
    fn current_step(&self) -> TourStep {
        TOUR_STEPS[self.step.min(TOUR_STEPS.len() - 1)]
    }

    /// One quick-start row: the key in a highlighted chip, then its action.
    fn key_row(
        key: &str,
        description: std::borrow::Cow<'static, str>,
        key_width: usize,
        palette: &Palette,
    ) -> Line<'static> {
        let padding = key_width.saturating_sub(key.chars().count());
        let left_pad = padding / 2;
        let right_pad = padding - left_pad;
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!("{}{}{}", " ".repeat(left_pad), key, " ".repeat(right_pad)),
                Style::default()
                    .fg(palette.key_hint)
                    .add_modifier(Modifier::REVERSED),
            ),
            Span::raw(format!(" {description}")),
        ])
    }

    /// The pop-up rect and the arrow for the current step: the intro is
    /// centered with no arrow; the other steps sit next to the element they
    /// present, with an arrow on the border facing it.
    fn placement(&self, width: u16, height: u16, ctx: &PopupDrawCtx) -> (Rect, Option<Arrow>) {
        let area = ctx.area;
        let center_x = area.x as i32 + area.width as i32 / 2;
        match self.current_step() {
            TourStep::Intro => (popup_rect(height, width, area), None),
            TourStep::Store => {
                // At the bottom of the store view, arrow up into the store
                let body = ctx.tour.body;
                let rect = placed(
                    width,
                    height,
                    center_x - width as i32 / 2,
                    body.bottom() as i32 - height as i32,
                    area,
                );
                let arrow = arrow_for(rect, center_x, rect.y, "\u{25b2}"); // ▲
                (rect, arrow)
            }
            TourStep::History => {
                // Just above the timeline bar, arrow down at the bar
                let timeline = ctx.tour.timeline;
                let (bar_x, bar_width) =
                    timeline_bar_geometry(timeline, ctx.snap.history_start, ctx.snap.history_head);
                let bar_center = bar_x as i32 + bar_width as i32 / 2;
                let rect = placed(
                    width,
                    height,
                    bar_center - width as i32 / 2,
                    timeline.y as i32 - height as i32,
                    area,
                );
                let arrow = arrow_for(
                    rect,
                    bar_center,
                    rect.bottom().saturating_sub(1),
                    "\u{25bc}",
                ); // ▼
                (rect, arrow)
            }
            TourStep::Run => {
                // Just below the Run/Stop button, arrow up at it
                let button = ctx.tour.run_button;
                let button_center = button.x as i32 + button.width as i32 / 2;
                let rect = placed(
                    width,
                    height,
                    button_center - width as i32 / 2,
                    button.bottom() as i32,
                    area,
                );
                let arrow = arrow_for(rect, button_center, rect.y, "\u{25b2}"); // ▲
                (rect, arrow)
            }
        }
    }

    /// Highlight the element the current step presents: a spotlight dims
    /// everything else on screen, then the element itself is marked in the
    /// tour color — the one-row timeline bar glows, with its empty track in
    /// a dimmer variant, and the Run/Stop button is filled. The store needs
    /// no drawn marker: the spotlight alone frames it (a border would
    /// overwrite the store's text), and the intro highlights nothing but
    /// the pop-up itself (the whole screen is dimmed).
    fn draw_highlight(&self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let palette = ctx.palette;
        let spotlight = match self.current_step() {
            TourStep::Intro => None,
            TourStep::Store => Some(ctx.tour.body),
            TourStep::History => Some(ctx.tour.timeline),
            TourStep::Run => Some(ctx.tour.run_button),
        };
        dim_spotlight(f, ctx.area, spotlight, palette);
        match self.current_step() {
            TourStep::Intro | TourStep::Store => {}
            TourStep::History => highlight_timeline(f, ctx.tour.timeline, ctx.snap, palette),
            TourStep::Run => fill_highlight(f, ctx.tour.run_button, palette),
        }
    }

    pub fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let palette = ctx.palette;
        let step_index = self.step.min(TOUR_STEPS.len() - 1);
        let step = TOUR_STEPS[step_index];
        let total = TOUR_STEPS.len();

        let title = match step {
            TourStep::Intro => t!("popup.welcome.title", version = env!("CARGO_PKG_VERSION")),
            TourStep::Store => t!("popup.welcome.step_store"),
            TourStep::History => t!("popup.welcome.step_history"),
            TourStep::Run => t!("popup.welcome.step_run"),
        };
        let footer = if step_index + 1 == total {
            t!(
                "popup.welcome.tour_footer_last",
                step = step_index + 1,
                total = total
            )
        } else {
            t!(
                "popup.welcome.tour_footer",
                step = step_index + 1,
                total = total
            )
        };

        let mut lines: Vec<Line<'static>> = match step {
            TourStep::Intro => {
                let loaded = match &ctx.snap.source_file {
                    Some(file) => t!("popup.welcome.loaded", file = file.clone()),
                    None => t!("popup.welcome.empty_store"),
                };
                let keys = [
                    ("Space", t!("popup.welcome.run")),
                    ("F10", t!("popup.welcome.step")),
                    ("\u{2190}/\u{2192}", t!("popup.welcome.scrub")),
                    ("H", t!("popup.welcome.help")),
                ];
                let key_width = keys
                    .iter()
                    .map(|(k, _)| k.chars().count())
                    .max()
                    .unwrap_or(0);
                let mut lines = vec![
                    Line::from(""),
                    Line::from(t!("popup.welcome.about")),
                    Line::from(t!("popup.welcome.about2")),
                    Line::from(""),
                    Line::from(Span::styled(
                        loaded.into_owned(),
                        Style::default().fg(palette.value),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        t!("popup.welcome.quickstart").into_owned(),
                        Style::default()
                            .fg(palette.highlight)
                            .add_modifier(Modifier::BOLD),
                    )),
                ];
                for (key, description) in &keys {
                    lines.push(Self::key_row(key, description.clone(), key_width, palette));
                }
                lines
            }
            TourStep::Store => vec![
                Line::from(""),
                Line::from(t!("popup.welcome.store1")),
                Line::from(t!("popup.welcome.store2")),
                Line::from(t!("popup.welcome.store3")),
            ],
            TourStep::History => vec![
                Line::from(""),
                Line::from(t!("popup.welcome.history1")),
                Line::from(t!("popup.welcome.history2")),
            ],
            TourStep::Run => vec![
                Line::from(""),
                Line::from(t!("popup.welcome.run1")),
                Line::from(t!("popup.welcome.run2")),
            ],
        };
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        let content_width = display_width(&title)
            .max(display_width(&footer))
            .max(lines.iter().map(Line::width).max().unwrap_or(0));
        let width = popup_width(content_width);
        let height = (lines.len() + 2) as u16;

        let (rect, arrow) = self.placement(width, height, ctx);
        self.draw_highlight(f, ctx);
        let content = self.frame.begin_popup(
            f,
            rect,
            format!(" {title} "),
            Some(palette.highlight),
            palette,
        );
        f.render_widget(Paragraph::new(lines), content);
        if let Some(arrow) = arrow {
            let cell = &mut f.buffer_mut()[(arrow.x, arrow.y)];
            cell.set_symbol(arrow.glyph);
            cell.set_fg(palette.highlight);
            cell.modifier.insert(Modifier::BOLD);
        }
    }
}

impl Popup for WelcomePopup {
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
        // While the tour is open, the menu bar stays inert: a click anywhere
        // only advances or closes the tour, it never triggers the UI behind
        true
    }
    fn handle_key(&mut self, key: KeyEvent, _sim: &mut Simulator) -> PopupEvent {
        // Enter/Space/Right advance the tour (closing it on the last step),
        // Left goes back — each guarded against OS key-repeat, so holding a
        // key does not rush the tour. Esc is closed centrally by
        // App::route_popup_key; a click outside advances via handle_click.
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right
                    if self.nav_key_guard.accepted(key.code) =>
                {
                    self.advance();
                }
                KeyCode::Left if self.nav_key_guard.accepted(key.code) => {
                    self.step = self.step.saturating_sub(1);
                }
                _ => {}
            }
        }
        PopupEvent::None
    }
    fn handle_click(&mut self, column: u16, row: u16, _sim: &mut Simulator) -> PopupEvent {
        // A click inside the pop-up is consumed; a click outside advances
        // the tour (closing it on the last step) instead of ending it
        if !self.frame.contains(column, row) {
            self.advance();
        }
        PopupEvent::None
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        WelcomePopup::draw(self, f, ctx);
    }
}
