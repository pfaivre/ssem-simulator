use super::*;
use rust_i18n::t;

pub struct SettingsView {
    /// The smoothed speed shown by the UI thread, and the target speed.
    pub displayed_speed: u32,
    pub target_speed: u32,
    pub heatmap: HeatmapMode,
    pub flow: FlowMode,
    pub comments: bool,
    pub bit_representation: usize,
    pub theme: Theme,
}

/// One row of the settings pop-up: the label, the keyboard shortcut shown
/// dimmed in its own column left of the controls, then its spans — each
/// either a clickable button (text + control) or plain value text.
type SettingsRow = (String, &'static str, Vec<(String, Option<SettingControl>)>);

/// A clickable control of the settings pop-up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingControl {
    SpeedDown,
    SpeedUp,
    Heatmap,
    Flow,
    Comments,
    Bits,
    Theme,
}

/// The settings pop-up, opened from the menu bar.
///
/// One row per setting, each with its keyboard shortcut on the left of a
/// clickable control.
pub struct SettingsPopup {
    pub(super) open: bool,
    /// Rectangle of the pop-up from the last draw, for mouse hit-testing.
    pub(super) last_rect: Rect,
    /// The control buttons' cells from the last draw: (rect, control).
    pub(super) buttons: Vec<(Rect, SettingControl)>,
    /// The control under the mouse cursor, for the hover highlight.
    pub(super) hover: Option<SettingControl>,
}

impl SettingsPopup {
    pub fn new() -> Self {
        SettingsPopup {
            open: false,
            last_rect: Rect::default(),
            buttons: Vec::new(),
            hover: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self) {
        self.open = true;
        self.hover = None;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.hover = None;
    }

    /// Whether the given screen cell lies inside the pop-up.
    pub fn contains(&self, column: u16, row: u16) -> bool {
        self.last_rect.contains(Position::new(column, row))
    }

    /// The control drawn under the given screen cell, if any.
    pub fn control_at(&self, column: u16, row: u16) -> Option<SettingControl> {
        self.buttons
            .iter()
            .find(|(rect, _)| rect.contains(Position::new(column, row)))
            .map(|(_, control)| *control)
    }

    /// The control under the mouse cursor (tests only).
    #[cfg(test)]
    pub fn hover(&self) -> Option<SettingControl> {
        self.hover
    }

    /// Record the control under the mouse cursor: the one drawn under it is
    /// highlighted, the others are not.
    pub fn set_hover(&mut self, hover: Option<SettingControl>) {
        self.hover = hover;
    }

    pub fn draw(&mut self, f: &mut Frame, area: Rect, view: &SettingsView, palette: &Palette) {
        let footer = t!("popup.settings.footer");

        let speed_text = t!(
            "popup.settings.speed_value",
            displayed = view.displayed_speed,
            target = view.target_speed
        );
        let (on, off) =
            BIT_REPRESENTATIONS[view.bit_representation.min(BIT_REPRESENTATIONS.len() - 1)];
        // Each row: the label, the shortcut, then its spans — a button (text
        // + control) or plain value text between the speed buttons
        let rows: [SettingsRow; 6] = [
            (
                ts!("popup.settings.speed"),
                "I/K",
                vec![
                    ("[-]".to_string(), Some(SettingControl::SpeedDown)),
                    (format!(" {speed_text} "), None),
                    ("[+]".to_string(), Some(SettingControl::SpeedUp)),
                ],
            ),
            (
                ts!("popup.settings.heatmap"),
                "M",
                vec![(
                    format!(
                        "[{}]",
                        view.heatmap
                            .label()
                            .map_or_else(|| ts!("mode.off"), str::to_string)
                    ),
                    Some(SettingControl::Heatmap),
                )],
            ),
            (
                ts!("popup.settings.flow"),
                "F",
                vec![(
                    format!(
                        "[{}]",
                        view.flow
                            .label()
                            .map_or_else(|| ts!("mode.off"), str::to_string)
                    ),
                    Some(SettingControl::Flow),
                )],
            ),
            (
                ts!("popup.settings.comments"),
                "C",
                vec![(
                    format!(
                        "[{}]",
                        if view.comments {
                            ts!("mode.on")
                        } else {
                            ts!("mode.off")
                        }
                    ),
                    Some(SettingControl::Comments),
                )],
            ),
            (
                ts!("popup.settings.bits"),
                "D",
                vec![(format!("[{on}{off}]"), Some(SettingControl::Bits))],
            ),
            (
                ts!("popup.settings.theme"),
                "T",
                vec![(
                    format!("[{}]", view.theme.label()),
                    Some(SettingControl::Theme),
                )],
            ),
        ];

        let label_width = rows
            .iter()
            .map(|(label, _, _)| display_width(label))
            .max()
            .unwrap_or(0);
        let hint_width = rows
            .iter()
            .map(|(_, hint, _)| display_width(hint))
            .max()
            .unwrap_or(0);
        let control_width = rows
            .iter()
            .map(|(_, _, segs)| {
                segs.iter()
                    .map(|(text, _)| display_width(text))
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        // Leading margin (2) + label + gap (2) + shortcut column + gap (2)
        // + the widest row's controls, so every column is vertically aligned
        let content_width = 2 + label_width + 2 + hint_width + 2 + control_width;
        // 2 border columns, 1 left margin, 1 right margin
        let width = (content_width.max(display_width(&footer)) + 4) as u16;
        // The rows + a blank line + the footer, plus the 2 border rows
        let height = (rows.len() + 4) as u16;

        let rect = popup_rect(height, width, area);
        self.last_rect = rect;
        f.render_widget(Clear, rect);
        // Clear resets the cells to the terminal default; restore the theme's
        // base colors under the pop-up
        f.buffer_mut()
            .set_style(rect, Style::default().fg(palette.fg).bg(palette.bg));

        let block = Block::bordered()
            .title(t!("popup.settings.title"))
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(palette.editor_border));
        let content = content_area(&block, rect);
        f.render_widget(block, rect);

        // A control is highlighted (same hover style as the menu bar) only
        // while the mouse is over it; otherwise it is plain key-hint text
        let button_style = Style::default().fg(palette.key_hint);
        let hover_style = Style::default().fg(palette.hover_fg).bg(palette.hover);
        let value_style = Style::default()
            .fg(palette.value)
            .add_modifier(Modifier::BOLD);

        self.buttons.clear();
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(rows.len() + 2);
        for (i, (label, hint, segs)) in rows.iter().enumerate() {
            // Label, then the dimmed shortcut right-aligned in its own
            // column, then the controls — each column starts at a fixed x
            let mut spans = vec![
                Span::raw(format!("  {label:<label_width$}  ")),
                Span::styled(
                    format!("{hint:>hint_width$}"),
                    Style::default().add_modifier(Modifier::DIM),
                ),
                Span::raw("  "),
            ];
            let mut x = content.x + 2 + label_width as u16 + 2 + hint_width as u16 + 2;
            let y = content.y + i as u16;
            for (text, control) in segs {
                let w = display_width(text) as u16;
                let style = match control {
                    Some(c) if self.hover == Some(*c) => hover_style,
                    Some(_) => button_style,
                    None => value_style,
                };
                spans.push(Span::styled(text.clone(), style));
                if let Some(control) = control {
                    self.buttons.push((Rect::new(x, y, w, 1), *control));
                }
                x += w;
            }
            lines.push(Line::from(spans));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            footer.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        )));

        f.render_widget(Paragraph::new(lines), content);
    }
}
