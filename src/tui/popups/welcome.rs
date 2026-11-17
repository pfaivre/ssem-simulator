use super::*;
use ratatui::widgets::Borders;
use rust_i18n::t;

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
/// Left (back); Esc or a click outside closes it from any step.
pub struct WelcomePopup {
    pub(super) frame: PopupFrame,
    /// The current tour step; reset to the intro when the pop-up opens.
    step: usize,
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
        }
    }

    pub fn open(&mut self) {
        self.frame.open = true;
        self.step = 0;
    }

    /// The current tour step (0 = intro).
    #[cfg(test)]
    pub fn step(&self) -> usize {
        self.step
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
                let body = ctx.body_area;
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
                let timeline = ctx.timeline_area;
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
                let button = ctx.run_stop_button;
                let (hint_x, arrow_x) = match button {
                    Some(b) => (
                        b.x as i32 + b.width as i32 / 2 - width as i32 / 2,
                        b.x as i32 + b.width as i32 / 2,
                    ),
                    None => (center_x - width as i32 / 2, center_x),
                };
                let hint_y = button.map_or(1, |b| b.bottom()) as i32;
                let rect = placed(width, height, hint_x, hint_y, area);
                let arrow = arrow_for(rect, arrow_x, rect.y, "\u{25b2}"); // ▲
                (rect, arrow)
            }
        }
    }

    /// Highlight the element the current step presents: side bars around the
    /// store view, a reversed highlight on the one-row timeline bar and on
    /// the Run/Stop button. The intro highlights nothing.
    fn draw_highlight(&self, f: &mut Frame, ctx: &PopupDrawCtx) {
        let palette = ctx.palette;
        match self.current_step() {
            TourStep::Intro => {}
            TourStep::Store => {
                // Side bars only: a full border would overwrite the CI row
                // and the store's last column
                let block = Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT)
                    .border_style(Style::default().fg(palette.highlight));
                f.render_widget(block, ctx.body_area);
            }
            TourStep::History => {
                f.buffer_mut().set_style(
                    ctx.timeline_area,
                    Style::default().add_modifier(Modifier::REVERSED),
                );
            }
            TourStep::Run => {
                if let Some(button) = ctx.run_stop_button {
                    f.buffer_mut()
                        .set_style(button, Style::default().add_modifier(Modifier::REVERSED));
                }
            }
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
    fn handle_key(&mut self, key: KeyEvent, _sim: &mut Simulator) -> PopupEvent {
        // Enter/Space/Right advance the tour (closing it on the last step),
        // Left goes back. Esc is closed centrally by App::route_popup_key,
        // a click outside by App::handle_mouse.
        if key.kind == KeyEventKind::Press {
            match key.code {
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Right => {
                    if self.step + 1 >= TOUR_STEPS.len() {
                        self.close();
                    } else {
                        self.step += 1;
                    }
                }
                KeyCode::Left => {
                    self.step = self.step.saturating_sub(1);
                }
                _ => {}
            }
        }
        PopupEvent::None
    }
    fn draw(&mut self, f: &mut Frame, ctx: &PopupDrawCtx) {
        WelcomePopup::draw(self, f, ctx);
    }
}
