//! Rendering of the whole interface: top bar, main body (CI, accumulator,
//! store words, heatmaps, flow arrows/graph), timeline, bottom bar, and the
//! scrub pop-up.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph};

use super::app::App;
use super::graph::{GRAPH_EXTRA_WIDTH, GraphGutter};
use super::layout::{
    A_ROW, CHROME_HEIGHT, CI_ROW, CONTENT_HEIGHT, MIN_HEIGHT, MIN_WIDTH, REGISTER_TEXT_WIDTH,
    STORE_ROW_OFFSET, STORE_TEXT_WIDTH, timeline_bar_geometry, timeline_marker_offset,
};
use super::snapshot::Snapshot;
use super::theme::{
    EXEC_RAMP, FlowMode, HEATMAP_BAR_WIDTH, HeatmapMode, KEYMAP, READ_RAMP, RW_BAR_WIDTH,
    WRITE_RAMP, opcode_color,
};
use crate::ssem::{Opcode, Store};

/// Extra width required when pointer arrows are shown
/// (separator + horizontal segment + arrowhead + gutter column).
pub(super) const ARROW_EXTRA_WIDTH: u16 = 4;

/// A pointer arrow: where it starts and ends (in content rows of the main
/// body) and its color.
struct PointerArrow {
    source: usize,
    target: usize,
    color: Color,
}

impl App {
    /// Compute one heatmap bar: `count` relative to `max`, `width` cells wide,
    /// filled proportionally and colored on the given 3-step `ramp`.
    /// A zero count (or an all-zero heatmap) is dim.
    fn heat_bar(count: u64, max: u64, width: usize, ramp: [Color; 3]) -> (String, Color) {
        if count == 0 || max == 0 {
            return ("\u{b7}".repeat(width), Color::DarkGray);
        }

        let ratio = count as f64 / max as f64;
        let filled = ((ratio * width as f64).ceil() as usize).clamp(1, width);

        let bar = format!(
            "{}{}",
            "\u{2588}".repeat(filled),
            "\u{b7}".repeat(width - filled),
        );

        let color = if ratio > 0.75 {
            ramp[2]
        } else if ratio > 0.25 {
            ramp[1]
        } else {
            ramp[0]
        };

        (bar, color)
    }

    /// The spans of the heatmap area for the store word at `index`, in the
    /// current mode. Empty when the heatmap is off.
    ///
    /// Exec mode draws one bar (how often the word has been executed);
    /// r/w mode draws two labeled bars: `r` for operand reads and `w` for
    /// writes, each scaled against the hottest word of its own category.
    fn heatmap_spans(&self, index: usize, snap: &Snapshot) -> Vec<Span<'static>> {
        match self.heatmap {
            HeatmapMode::Off => Vec::new(),
            HeatmapMode::Exec => {
                let max = snap.exec_counts.iter().max().copied().unwrap_or(0);
                let (bar, color) =
                    Self::heat_bar(snap.exec_counts[index], max, HEATMAP_BAR_WIDTH, EXEC_RAMP);
                vec![Span::styled(format!(" {bar} "), Style::default().fg(color))]
            }
            HeatmapMode::Rw => {
                let read_max = snap.read_counts.iter().max().copied().unwrap_or(0);
                let write_max = snap.write_counts.iter().max().copied().unwrap_or(0);
                let (rbar, rcolor) =
                    Self::heat_bar(snap.read_counts[index], read_max, RW_BAR_WIDTH, READ_RAMP);
                let (wbar, wcolor) = Self::heat_bar(
                    snap.write_counts[index],
                    write_max,
                    RW_BAR_WIDTH,
                    WRITE_RAMP,
                );
                vec![
                    Span::raw(" r"),
                    Span::styled(rbar, Style::default().fg(rcolor)),
                    Span::raw(" w"),
                    Span::styled(wbar, Style::default().fg(wcolor)),
                    Span::raw(" "),
                ]
            }
        }
    }

    /// The pointer arrow for the last executed instruction, following its
    /// data flow:
    ///
    /// - STO: from the accumulator to the destination word
    /// - LDN: from the source word to the accumulator
    /// - SUB/SUB2: like LDN, in a different color
    /// - JMP: from the word holding the target value to CI (a copy)
    /// - JRP: like JMP, in a different color (the value is added to CI)
    ///
    /// Every arrow is a data movement, so the color matches the operation:
    /// magenta when a value is loaded or stored, cyan when it is
    /// arithmetically combined. The arrow follows the executed instruction
    /// rather than CI: after a jump, CI holds the target value, so the word
    /// at CI is neither the last nor the next instruction to execute (CI is
    /// incremented before each fetch). Returns `None` when no instruction
    /// has executed yet, or when the instruction moves no store data (CMP
    /// and STP ignore their data field).
    fn current_arrow(snap: &Snapshot) -> Option<PointerArrow> {
        let li = snap.last_instruction?;
        let word_row = STORE_ROW_OFFSET + li.data as usize;
        let (source, target) = match li.opcode {
            Opcode::STO => (A_ROW, word_row),
            Opcode::LDN | Opcode::SUB | Opcode::SUB2 => (word_row, A_ROW),
            Opcode::JMP | Opcode::JRP => (word_row, CI_ROW),
            // CMP and STP ignore their data field
            _ => return None,
        };
        Some(PointerArrow {
            source,
            target,
            color: opcode_color(li.opcode),
        })
    }

    /// Draw the entire interface from a state snapshot.
    pub(super) fn draw(&mut self, f: &mut Frame, snap: &Snapshot) {
        let area = f.area();
        self.last_area = area;

        // Clamp scroll to keep the content within the visible area
        let visible_height = area.height.saturating_sub(CHROME_HEIGHT) as usize;
        let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
        self.store_scroll = (self.store_scroll as usize).min(max_scroll) as u16;

        // Check minimum terminal size (wider when a heatmap or a flow
        // visualization is shown)
        let mut min_width = MIN_WIDTH + self.heatmap.extra_width();
        match self.flow {
            FlowMode::Control => min_width += GRAPH_EXTRA_WIDTH,
            FlowMode::Data => min_width += ARROW_EXTRA_WIDTH,
            FlowMode::Off => {}
        }
        if area.width < min_width || area.height < MIN_HEIGHT {
            let msg = format!(
                "Terminal too small: {}x{}, needs at least {}x{}",
                area.width, area.height, min_width, MIN_HEIGHT
            );
            let paragraph = Paragraph::new(msg);
            f.render_widget(paragraph, area);
            self.timeline_area = Rect::default();
            return;
        }

        // Layout: top bar (1 line), main body (flexible), timeline (1 line), bottom bar (1 line)
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        self.timeline_area = chunks[2];
        self.draw_top_bar(f, chunks[0], snap);
        self.draw_main_body(f, chunks[1], snap);
        self.draw_timeline(f, chunks[2], snap);
        self.draw_bottom_bar(f, chunks[3]);

        // Draw pop-ups on top
        if self.help_popup.is_open() {
            self.help_popup.draw(f, area);
        }
        if self.breakpoint_popup.is_open() {
            self.breakpoint_popup.draw(f, area, &snap.breakpoints);
        }
        if self.editor_popup.is_open() {
            self.editor_popup
                .draw(f, area, snap.ci, snap.a, &snap.store);
        }

        // While scrubbing the timeline, show the current cycle above the cursor
        if self.scrubbing {
            self.draw_scrub_popup(f, area, snap);
        }
    }

    /// Draw the top bar with status indicators.
    fn draw_top_bar(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let status = if self.scrubbing {
            "REWIND"
        } else if snap.stop_flag {
            "STOPPED"
        } else {
            "RUNNING"
        };

        // The chrome keeps a single color, the last instruction included:
        // the syntax highlighting lives in the store view
        let last_instr = snap
            .last_instruction
            .map(|li| li.to_string())
            .unwrap_or_else(|| "-- -- --".to_string());

        let mut text = format!(
            "[ STATUS: {status} ]  [ CYCLES: {} ]  [ {last_instr} ]  [ SPEED: {:2}/{} ips ] ",
            snap.cycle_count, self.displayed_speed, snap.speed
        );

        if !snap.breakpoints.is_empty() {
            text.push_str(&format!(" [ BREAKPOINTS: {} ] ", snap.breakpoints.len()));
        }

        if let Some(label) = self.heatmap.label() {
            text.push_str(&format!(" [ HEATMAP: {label} ] "));
        }

        if let Some(label) = self.flow.label() {
            text.push_str(&format!(" [ FLOW: {label} ] "));
        }

        let line = Line::from(Span::styled(text, Style::default().fg(Color::Blue)));
        f.render_widget(Paragraph::new(line), area);
    }

    /// Draw the main body: CI, accumulator, and store words.
    fn draw_main_body(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(CONTENT_HEIGHT);

        // Spacer to align CI/A bits with store-word bits when a heatmap is on
        let heatmap_spacer = " ".repeat(self.heatmap.extra_width() as usize);

        // Row 0: blank
        lines.push(Line::from(""));

        // In graph mode the gutter draws the static control-flow edges
        // between the store lines instead of the data-flow arrow
        let graph = (self.flow == FlowMode::Control).then(|| GraphGutter::new(&snap.store));

        // Pointer arrow: gutter cells appended after a line's text, following
        // the data flow of the last executed instruction
        let arrow = if self.flow == FlowMode::Data {
            Self::current_arrow(snap)
        } else {
            None
        };
        let arrow_target = arrow.as_ref().map(|a| a.target);

        // Gutter cells for the rows whose text spans the full store width
        // (store words, and the blank row the arrow's vertical may cross).
        // The register rows' cells (CI, accumulator) are built separately,
        // since their text is shorter and the arrow must connect back to it.
        let heatmap_extra = self.heatmap.extra_width() as usize;
        let mut gutter: Vec<Option<Span<'static>>> = vec![None; CONTENT_HEIGHT];
        if let Some(arrow) = &arrow {
            let style = Style::default().fg(arrow.color);
            let (top, bottom) = (
                arrow.source.min(arrow.target),
                arrow.source.max(arrow.target),
            );
            // The corner leaving the source line turns toward the target;
            // the corner at the target line turns toward the arrowhead
            let (src_corner, dst_corner) = if arrow.target > arrow.source {
                ('\u{256e}', '\u{256f}') // ╮ ╯
            } else {
                ('\u{256f}', '\u{256e}') // ╯ ╮
            };
            if !matches!(arrow.source, CI_ROW | A_ROW) {
                gutter[arrow.source] = Some(Span::styled(format!(" ──{src_corner}"), style));
            }
            for cell in gutter.iter_mut().take(bottom).skip(top + 1) {
                *cell = Some(Span::styled("   \u{2502}".to_string(), style));
            }
            if !matches!(arrow.target, CI_ROW | A_ROW) {
                gutter[arrow.target] = Some(Span::styled(
                    format!(" \u{25c0}\u{2500}{dst_corner}"),
                    style,
                ));
            }
        }

        // A register row's (CI, accumulator) gutter cell: a run of dashes
        // connects the arrow back to the register's text. When the arrow's
        // vertical crosses a register row that is not an endpoint (an arrow
        // to CI crosses the accumulator), that row carries the vertical.
        let register_gutter = |row: usize| -> Option<Span<'static>> {
            let arrow = arrow.as_ref()?;
            let style = Style::default().fg(arrow.color);
            // Cells between the register's text and the store gutter
            let pad = STORE_TEXT_WIDTH - REGISTER_TEXT_WIDTH;
            if arrow.source == row {
                // The arrow leaves the register (STO)
                let corner = if arrow.target > arrow.source {
                    '\u{256e}'
                } else {
                    '\u{256f}'
                };
                Some(Span::styled(
                    format!(" {}{corner}", "\u{2500}".repeat(pad + 2)),
                    style,
                ))
            } else if arrow.target == row {
                // The arrow arrives at the register (LDN/SUB to A, JMP/JRP to CI)
                let corner = if arrow.target > arrow.source {
                    '\u{256f}'
                } else {
                    '\u{256e}'
                };
                Some(Span::styled(
                    format!(" \u{25c0}{}{corner}", "\u{2500}".repeat(pad + 1)),
                    style,
                ))
            } else if arrow.source.min(arrow.target) < row && row < arrow.source.max(arrow.target) {
                // The arrow's vertical crosses this register row
                Some(Span::styled(
                    format!("{}   \u{2502}", " ".repeat(pad)),
                    style,
                ))
            } else {
                None
            }
        };

        // Row 1: CI
        let ci_bits = self.format_bits(snap.ci);
        let ci_style = if arrow_target == Some(CI_ROW) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let mut ci_spans = vec![
            Span::styled(format!("{heatmap_spacer} {ci_bits}"), ci_style),
            Span::raw(format!(" CI: {:>11}", snap.ci)),
        ];
        if let Some(cell) = &register_gutter(CI_ROW) {
            ci_spans.push(cell.clone());
        }
        lines.push(Line::from(ci_spans));

        // Row 2: Accumulator
        let a_bits = self.format_bits(snap.a);
        let a_style = if arrow_target == Some(A_ROW) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        };
        let mut a_spans = vec![
            Span::styled(format!("{heatmap_spacer} {a_bits}"), a_style),
            Span::raw(format!(" A : {:>11}", snap.a)),
        ];
        if let Some(cell) = &register_gutter(A_ROW) {
            a_spans.push(cell.clone());
        }
        lines.push(Line::from(a_spans));

        // Row 3: blank (carries the arrow's vertical segment when it crosses)
        let mut blank_spans: Vec<Span<'static>> = Vec::new();
        if let Some(g) = &graph {
            blank_spans.extend(g.row_spans(3));
        } else if let Some(cell) = &gutter[3] {
            blank_spans.push(Span::raw(" ".repeat(STORE_TEXT_WIDTH + heatmap_extra)));
            blank_spans.push(cell.clone());
        }
        lines.push(Line::from(blank_spans));

        // The line of the last executed instruction, marked `>`: after a
        // jump CI holds the target value, so the marker follows the executed
        // line rather than CI. Nothing is marked before the first cycle.
        let current_line = snap
            .last_instruction
            .map(|li| (li.address & Store::INDEX_MASK) as usize);

        // When paused, the pointer column also shows the execution footprint
        // of the current run: `»` on the next line to execute (CI is
        // incremented before each fetch, so the next fetch is CI + 1), and
        // `•` on every other line executed at least once.
        let paused = snap.stop_flag;
        let next_line = paused.then(|| ((snap.ci + 1) & Store::INDEX_MASK) as usize);

        // Rows 4-35: store words
        for (index, word) in snap.store.iter().enumerate() {
            let row = STORE_ROW_OFFSET + index;
            let is_current = current_line == Some(index);
            let is_target = arrow_target == Some(row);
            let (pointer, pointer_style) = if is_current {
                (">", Style::default().add_modifier(Modifier::BOLD))
            } else if next_line == Some(index) {
                (
                    "\u{bb}",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )
            } else if paused && snap.exec_counts[index] > 0 {
                ("\u{2022}", Style::default().fg(Color::DarkGray))
            } else {
                (" ", Style::default())
            };
            let bits = self.format_bits(*word);
            let (opcode, data) = Store::decode_word(*word);

            let bits_style = if is_current {
                Style::default().add_modifier(Modifier::BOLD)
            } else if is_target {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            };

            // Heatmap bars (no span when the heatmap is off)
            let heatmap = self.heatmap_spans(index, snap);

            let mut spans = vec![
                Span::styled(pointer.to_string(), pointer_style),
                Span::styled(bits, bits_style),
                Span::raw(format!(" {:02}: {:>11}  ", index, word)),
                Span::styled(
                    format!("{:<4}", opcode.to_string()),
                    Style::default()
                        .fg(opcode_color(opcode))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {:02}", data)),
            ];
            spans.splice(1..1, heatmap);
            if let Some(g) = &graph {
                spans.extend(g.row_spans(row));
            } else if let Some(cell) = &gutter[row] {
                spans.push(cell.clone());
            }
            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines).scroll((self.store_scroll, 0));
        f.render_widget(paragraph, area);
    }

    /// Draw the timeline bar: how much history is retained (filled bar) and
    /// where the machine currently is (marker). Dragging it scrubs the history.
    fn draw_timeline(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        if snap.history_capacity == 0 {
            let line = Line::from(Span::styled(
                " HISTORY [disabled]",
                Style::default().fg(Color::DarkGray),
            ));
            f.render_widget(Paragraph::new(line), area);
            return;
        }

        let (_, bar_width) = timeline_bar_geometry(area, snap.history_start, snap.history_head);

        // Filled portion = fraction of the capacity still retained
        let filled = (snap.history_len * u64::from(bar_width) / snap.history_capacity) as usize;

        // Marker at the current position (the tip, or the scrub position)
        let marker = timeline_marker_offset(
            bar_width,
            snap.history_start,
            snap.history_head,
            snap.cycle_count,
        );

        let blue = Style::default().fg(Color::Blue);
        let mut spans = vec![Span::styled(" HISTORY [".to_string(), blue)];
        if bar_width > 0 {
            let segment = |from: usize, to: usize| -> String {
                (from..to)
                    .map(|i| if i < filled { '\u{2588}' } else { '\u{b7}' })
                    .collect()
            };
            let marker = marker.unwrap_or(0);
            spans.push(Span::styled(segment(0, marker), blue));
            spans.push(Span::styled(
                ">".to_string(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                segment(marker + 1, usize::from(bar_width)),
                blue,
            ));
        }
        spans.push(Span::styled(
            format!("] {}..{}", snap.history_start, snap.history_head),
            blue,
        ));

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Draw a small pop-up above the timeline cursor, showing the current cycle.
    fn draw_scrub_popup(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        if self.timeline_area.height == 0 {
            return;
        }

        // Center the pop-up on the bar's position marker, so it stays above
        // the marker even when the mouse is dragged past the bar's ends
        // (only the screen edges clamp it).
        let (bar_x, bar_width) =
            timeline_bar_geometry(self.timeline_area, snap.history_start, snap.history_head);
        let Some(offset) = timeline_marker_offset(
            bar_width,
            snap.history_start,
            snap.history_head,
            snap.cycle_count,
        ) else {
            return;
        };
        let center = bar_x + offset as u16;

        let text = format!(" cycle {} ", snap.cycle_count);
        let width = text.len() as u16;
        let x = center
            .saturating_sub(width / 2)
            .clamp(area.x, area.x + area.width.saturating_sub(width));
        let popup_area = Rect {
            x,
            y: self.timeline_area.y.saturating_sub(1),
            width,
            height: 1,
        };

        f.render_widget(Clear, popup_area);
        f.render_widget(
            Paragraph::new(text).style(Style::default().fg(Color::Black).bg(Color::Yellow)),
            popup_area,
        );
    }

    /// Draw the bottom bar with keyboard shortcuts.
    fn draw_bottom_bar(&self, f: &mut Frame, area: Rect) {
        let key_style = Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::REVERSED);
        let normal_style = Style::default();

        let mut spans: Vec<Span<'static>> = Vec::new();
        for (key, legend, _) in KEYMAP {
            if legend.is_empty() {
                continue;
            }
            let key_text = if key.chars().count() == 1 {
                format!(" {key} ")
            } else {
                (*key).to_string()
            };
            spans.push(Span::styled(key_text, key_style));
            spans.push(Span::styled(format!(" {legend}  "), normal_style));
        }

        let line = Line::from(spans);
        f.render_widget(Paragraph::new(line), area);
    }
}
