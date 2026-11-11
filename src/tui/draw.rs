//! Rendering of the whole interface: menu bar, main body (CI, accumulator,
//! store words, heatmaps, flow arrows/graph), timeline (history scrub bar),
//! status bar, and the pop-ups (including the scrub pop-up).

use std::borrow::Cow;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use rust_i18n::t;

use super::app::{App, MenuAction, menu_items};
use super::graph::{GRAPH_EXTRA_WIDTH, GraphGutter};
use super::layout::{
    A_ROW, CHROME_HEIGHT, CI_ROW, CONTENT_HEIGHT, FlowMode, HEATMAP_BAR_WIDTH, HeatmapMode,
    MAX_COMMENT_WIDTH, MIN_COMMENT_WIDTH, MIN_HEIGHT, MIN_WIDTH, REGISTER_TEXT_WIDTH, RW_BAR_WIDTH,
    STORE_ROW_OFFSET, STORE_TEXT_WIDTH, comment_column_width, timeline_bar_geometry,
    timeline_marker_offset,
};
use super::palette::Palette;
use super::popups::{Popup, PopupDrawCtx, SettingsView};
use super::snapshot::Snapshot;
use crate::ssem::{Opcode, Store};

/// Extra width required when pointer arrows are shown
/// (separator + horizontal segment + arrowhead + gutter column).
pub(super) const ARROW_EXTRA_WIDTH: u16 = 4;

/// Width (in cells) the flow gutter occupies between the store text and the
/// comment column, by flow mode: the pointer arrow or the control-flow graph.
pub(super) fn flow_gutter_width(flow: FlowMode) -> u16 {
    match flow {
        FlowMode::Control => GRAPH_EXTRA_WIDTH,
        FlowMode::Data => ARROW_EXTRA_WIDTH,
        FlowMode::Off => 0,
    }
}

/// The heatmap actually drawn at the given terminal width: the requested
/// one, or Off when the width cannot fit it next to the store text.
pub(super) fn effective_heatmap(area_width: u16, heatmap: HeatmapMode) -> HeatmapMode {
    if area_width >= MIN_WIDTH + heatmap.extra_width() {
        heatmap
    } else {
        HeatmapMode::Off
    }
}

/// Screen column where the comment column starts (its " ;" prefix), after
/// the store text, the heatmap, and the flow gutter.
pub(super) fn comment_column_start(heatmap: HeatmapMode, flow: FlowMode) -> usize {
    STORE_TEXT_WIDTH + heatmap.extra_width() as usize + flow_gutter_width(flow) as usize
}

/// Width of the comment column actually drawn at the given terminal width:
/// the widest comment — or the in-progress inline edit (`edit_width`), so
/// the text box stays visible while typing — capped at `MAX_COMMENT_WIDTH`
/// and at the width left after the store text, the heatmap, and the flow
/// gutter. When comments are shown, at least `MIN_COMMENT_WIDTH` is kept
/// even when no word has a comment yet, so one can be added by
/// double-clicking a store row. Zero when comments are hidden or no room
/// is left at all.
pub(super) fn drawn_comment_width(
    area_width: u16,
    heatmap: HeatmapMode,
    flow: FlowMode,
    comments: &[Option<String>; 32],
    edit_width: usize,
    show_comments: bool,
) -> usize {
    if !show_comments {
        return 0;
    }
    let available = (area_width as usize).saturating_sub(comment_column_start(heatmap, flow) + 2);
    comment_column_width(comments)
        .max(edit_width.min(MAX_COMMENT_WIDTH))
        .max(MIN_COMMENT_WIDTH)
        .min(available)
}

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
    fn heat_bar(&self, count: u64, max: u64, width: usize, ramp: [Color; 3]) -> (String, Color) {
        if count == 0 || max == 0 {
            return ("\u{b7}".repeat(width), self.palette().dim);
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
    fn heatmap_spans(
        &self,
        index: usize,
        snap: &Snapshot,
        heatmap: HeatmapMode,
    ) -> Vec<Span<'static>> {
        match heatmap {
            HeatmapMode::Off => Vec::new(),
            HeatmapMode::Exec => {
                let max = snap.exec_counts.iter().max().copied().unwrap_or(0);
                let (bar, color) = self.heat_bar(
                    snap.exec_counts[index],
                    max,
                    HEATMAP_BAR_WIDTH,
                    self.palette().exec_ramp,
                );
                vec![Span::styled(format!(" {bar} "), Style::default().fg(color))]
            }
            HeatmapMode::Rw => {
                let read_max = snap.read_counts.iter().max().copied().unwrap_or(0);
                let write_max = snap.write_counts.iter().max().copied().unwrap_or(0);
                let (rbar, rcolor) = self.heat_bar(
                    snap.read_counts[index],
                    read_max,
                    RW_BAR_WIDTH,
                    self.palette().read_ramp,
                );
                let (wbar, wcolor) = self.heat_bar(
                    snap.write_counts[index],
                    write_max,
                    RW_BAR_WIDTH,
                    self.palette().write_ramp,
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
    /// - JRP: like JMP, in the same jump color (the value is added to CI)
    ///
    /// The arrow takes the operation's category color, shared with the
    /// mnemonic and the graph edge: the memory color for LDN/STO, the jump
    /// color for JMP/JRP, the arithmetic color for SUB/SUB2. The arrow
    /// follows the executed instruction
    /// rather than CI: after a jump, CI holds the target value, so the word
    /// at CI is neither the last nor the next instruction to execute (CI is
    /// incremented before each fetch). Returns `None` when no instruction
    /// has executed yet, or when the instruction moves no store data (CMP
    /// and STP ignore their data field).
    fn current_arrow(palette: &Palette, snap: &Snapshot) -> Option<PointerArrow> {
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
            color: palette.opcode(li.opcode),
        })
    }

    /// The assignment made by the last executed instruction, as a margin
    /// label on the arrow's top line: "CI += -3", "A = 128", "[23] = 54".
    ///
    /// Loads, stores, and JMP are absolute (`=`, showing the new value);
    /// arithmetic combinations (SUB, JRP) are relative (`+=`, with the
    /// negated operand for SUB). CMP and STP move no data, so they get no
    /// label — like the arrow, which `current_arrow` withholds for them.
    fn assignment_label(snap: &Snapshot) -> Option<String> {
        let li = snap.last_instruction?;
        // `data` comes from `decode_word`, so it is masked to 0..=31 and
        // indexing the store is safe
        let operand = snap.store[li.data as usize];
        match li.opcode {
            Opcode::STO => Some(format!("[{:02}] = {}", li.data, snap.a)),
            Opcode::LDN => Some(format!("A = {}", snap.a)),
            Opcode::SUB | Opcode::SUB2 => Some(format!("A += {}", operand.wrapping_neg())),
            Opcode::JMP => Some(format!("CI = {}", snap.ci)),
            Opcode::JRP => Some(format!("CI += {operand}")),
            // CMP and STP move no data; NUM never reaches the display
            // (`Store::decode_word` always yields a real opcode)
            Opcode::CMP | Opcode::STP | Opcode::NUM => None,
        }
    }

    /// Draw the entire interface from a state snapshot.
    pub(super) fn draw(&mut self, f: &mut Frame, snap: &Snapshot) {
        let area = f.area();
        self.last_area = area;
        // The menu bar items are re-recorded on every draw (empty when the
        // terminal is too small, so clicks find nothing)
        self.menu_items.clear();

        // Paint the theme's base colors over the whole frame. Only fg/bg are
        // patched, so this touches no text; the widgets drawn below keep
        // them unless they set their own (the zebra stripes and the scrub
        // pop-up override the background).
        f.buffer_mut().set_style(
            area,
            Style::default().fg(self.palette().fg).bg(self.palette().bg),
        );

        // Clamp scroll to keep the content within the visible area
        let visible_height = area.height.saturating_sub(CHROME_HEIGHT) as usize;
        let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
        self.store_scroll = (self.store_scroll as usize).min(max_scroll) as u16;

        // Minimum terminal size: the store text needs MIN_WIDTH and the
        // chrome MIN_HEIGHT. On top of that, the heatmap (drawn just after
        // the pointer column) must fit entirely or it is dropped, and the
        // flow visualization at the right edge is cropped by the terminal
        // when the window is too narrow.
        let heatmap = effective_heatmap(area.width, self.heatmap);
        // The comment column is cropped to the width left after the store
        // text, the heatmap, and the flow gutter (which stays left of the
        // comments so the arrows remain visible), instead of being hidden
        // entirely. Each comment span is " ;" + the text, so it needs 2
        // cells on top of the comment width. While the inline comment
        // editor is open, its buffer counts toward the width so the text
        // box stays visible while typing, and the column is shown even
        // when no loaded word has a comment yet.
        let comment_width = drawn_comment_width(
            area.width,
            heatmap,
            self.flow,
            &snap.comments,
            self.comment_editor.edit_width(),
            self.show_comments || self.comment_editor.is_open(),
        );
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            let msg = t!(
                "draw.too_small",
                width = area.width,
                height = area.height,
                min_width = MIN_WIDTH,
                min_height = MIN_HEIGHT
            );
            let paragraph = Paragraph::new(msg);
            f.render_widget(paragraph, area);
            self.timeline_area = Rect::default();
            return;
        }

        // Layout: menu bar (1 line), main body (flexible), history scrub bar
        // (1 line), status bar (1 line)
        let chunks = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);

        self.timeline_area = chunks[2];
        self.draw_menu_bar(f, chunks[0], snap);
        self.draw_main_body(f, chunks[1], snap, heatmap, comment_width);
        self.draw_timeline(f, chunks[2], snap);
        self.draw_status_bar(f, chunks[3], snap, heatmap, comment_width);

        // Draw pop-ups on top, in registry order (z-order)
        {
            let ctx = PopupDrawCtx {
                area,
                palette: self.palette(),
                snap,
                settings_view: SettingsView {
                    displayed_speed: self.displayed_speed,
                    target_speed: snap.speed,
                    heatmap: self.heatmap,
                    flow: self.flow,
                    comments: self.show_comments,
                    bit_representation: self.bit_representation,
                    theme: self.theme,
                },
                // Anchor the picker just right of the store text (past the
                // heatmap bars), next to the mnemonic/data fields being edited
                picker_anchor_col: (STORE_TEXT_WIDTH + heatmap.extra_width() as usize) as u16,
            };
            for popup in self.popups_mut() {
                if popup.is_open() {
                    popup.draw(f, &ctx);
                }
            }
        }

        // While scrubbing the timeline, show the current cycle above the cursor
        if self.scrubbing {
            self.draw_scrub_popup(f, area, snap);
        }
    }

    /// Draw the menu bar (the top row): the clickable actions on the same
    /// solid bar as the status bar. Each button is a padded segment in the
    /// block color, without brackets; the button under the mouse — or whose
    /// pop-up is open — is highlighted (the hover colors). Each
    /// button's rect is recorded in `self.menu_items` for mouse hit-testing.
    fn draw_menu_bar(&mut self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        let palette = self.palette();
        let bar = Style::default().fg(palette.fg).bg(palette.menu_bg);
        let block = Style::default().fg(palette.fg).bg(palette.menu_block_bg);
        let hovered = Style::default().fg(palette.hover_fg).bg(palette.hover);
        // The bar itself: a solid background over the whole row
        f.buffer_mut().set_style(area, bar);

        let mut spans: Vec<Span<'static>> = Vec::new();
        let mut x = area.x;
        for (label, action) in menu_items() {
            let action = *action;
            // The run/stop item's label reflects the machine state
            let label: Cow<str> = if action == MenuAction::RunStop {
                Cow::Owned(if snap.stop_flag {
                    ts!("menu.run")
                } else {
                    ts!("menu.stop")
                })
            } else {
                Cow::Borrowed(label.as_str())
            };
            // No brackets: the button is the label padded with one cell
            // on each side in the block color
            let text = format!(" {label} ");
            let is_active = matches!(action, MenuAction::Settings if self.settings_popup.is_open())
                || matches!(action, MenuAction::Help if self.help_popup.is_open());
            let style = if self.menu_hover == Some(action) || is_active {
                hovered
            } else {
                block
            };
            let width = text.chars().count() as u16;
            spans.push(Span::styled(text, style));
            self.menu_items
                .push((Rect::new(x, area.y, width, 1), action));
            x += width;
            // (a separator span can be pushed here between buttons, if wanted:
            //  spans.push(Span::styled(" ", bar)); x += 1;)
        }

        f.render_widget(Paragraph::new(Line::from(spans)), area);
    }

    /// Draw the main body: CI, accumulator, and store words.
    fn draw_main_body(
        &mut self,
        f: &mut Frame,
        area: Rect,
        snap: &Snapshot,
        heatmap: HeatmapMode,
        comment_width: usize,
    ) {
        let mut lines: Vec<Line<'static>> = Vec::with_capacity(CONTENT_HEIGHT);

        // Spacer to align CI/A bits with store-word bits when a heatmap is on
        let heatmap_spacer = " ".repeat(heatmap.extra_width() as usize);

        // Row 0: blank
        lines.push(Line::from(""));

        // In graph mode the gutter draws the static control-flow edges
        // between the store lines instead of the data-flow arrow
        let graph =
            (self.flow == FlowMode::Control).then(|| GraphGutter::new(&snap.store, self.palette()));

        // Pointer arrow: gutter cells appended after a line's text, following
        // the data flow of the last executed instruction
        let arrow = if self.flow == FlowMode::Data {
            Self::current_arrow(self.palette(), snap)
        } else {
            None
        };
        let arrow_target = arrow.as_ref().map(|a| a.target);
        // The assignment label rides on the arrow's top line. The top row is
        // always the CI or A row: store lines sit below the registers, so one
        // of the arrow's endpoints is a register and it is the upper one.
        // The label is drawn in the arrow's color, past the gutter; the
        // comment column keeps its offset (register rows carry no comment).
        let arrow_top = arrow.as_ref().map(|a| a.source.min(a.target));
        let label_span = arrow
            .as_ref()
            .zip(Self::assignment_label(snap))
            .map(|(a, text)| Span::styled(format!(" {text}"), Style::default().fg(a.color)));

        // Gutter cells for the rows whose text spans the full store width
        // (store words, and the blank row the arrow's vertical may cross).
        // The register rows' cells (CI, accumulator) are built separately,
        // since their text is shorter and the arrow must connect back to it.
        let heatmap_extra = heatmap.extra_width() as usize;
        // Total width of the comment span (" ;" + the comment text), 0 when off
        let comment_extra = if comment_width > 0 {
            comment_width + 2
        } else {
            0
        };
        // Whether no word carries a comment yet: the column then shows a
        // dim " ;" gutter on every store row, as a visible double-click
        // target to add the first comment
        let no_comments = snap.comments.iter().all(Option::is_none);
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
            // (the gutter sits right after the store text and the heatmap)
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
                .fg(self.palette().highlight)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.palette().value)
                .add_modifier(Modifier::BOLD)
        };
        let mut ci_spans = vec![
            Span::styled(format!("{heatmap_spacer} {ci_bits}"), ci_style),
            Span::raw(format!(" CI: {:>11}", snap.ci)),
        ];
        if let Some(cell) = &register_gutter(CI_ROW) {
            ci_spans.push(cell.clone());
        }
        if arrow_top == Some(CI_ROW)
            && let Some(span) = &label_span
        {
            ci_spans.push(span.clone());
        }
        lines.push(Line::from(ci_spans));

        // Row 2: Accumulator
        let a_bits = self.format_bits(snap.a);
        let a_style = if arrow_target == Some(A_ROW) {
            Style::default()
                .fg(self.palette().highlight)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(self.palette().value)
                .add_modifier(Modifier::BOLD)
        };
        let mut a_spans = vec![
            Span::styled(format!("{heatmap_spacer} {a_bits}"), a_style),
            Span::raw(format!(" A : {:>11}", snap.a)),
        ];
        if let Some(cell) = &register_gutter(A_ROW) {
            a_spans.push(cell.clone());
        }
        if arrow_top == Some(A_ROW)
            && let Some(span) = &label_span
        {
            a_spans.push(span.clone());
        }
        lines.push(Line::from(a_spans));

        // Row 3: blank (carries the arrow's vertical segment when it crosses)
        let mut blank_spans: Vec<Span<'static>> = Vec::new();
        blank_spans.push(Span::raw(" ".repeat(STORE_TEXT_WIDTH + heatmap_extra)));
        if let Some(g) = &graph {
            blank_spans.extend(g.row_spans(3));
        } else if let Some(cell) = &gutter[3] {
            blank_spans.push(cell.clone());
        } else if self.flow == FlowMode::Data {
            // Keep the comment column aligned when no arrow is drawn
            blank_spans.push(Span::raw(" ".repeat(ARROW_EXTRA_WIDTH as usize)));
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
                        .fg(self.palette().next_marker)
                        .add_modifier(Modifier::BOLD),
                )
            } else if paused && snap.exec_counts[index] > 0 {
                ("\u{2022}", Style::default().fg(self.palette().dim))
            } else {
                (" ", Style::default())
            };
            let bits = self.format_bits(*word);
            let (opcode, data) = Store::decode_word(*word);

            let bits_style = if is_current {
                Style::default().add_modifier(Modifier::BOLD)
            } else if is_target {
                Style::default()
                    .fg(self.palette().highlight)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(self.palette().value)
                    .add_modifier(Modifier::BOLD)
            };

            // Heatmap bars (no span when the heatmap is off)
            let bars = self.heatmap_spans(index, snap, heatmap);

            let data_span = if opcode.uses_address() {
                Span::raw(format!(" {:02}", data))
            } else {
                Span::raw("   ")
            };

            let mut spans = vec![
                Span::styled(pointer.to_string(), pointer_style),
                Span::styled(bits, bits_style),
                Span::raw(format!(" {:02}: {:>11}  ", index, word)),
                Span::styled(
                    format!("{:<4}", opcode.to_string()),
                    Style::default()
                        .fg(self.palette().opcode(opcode))
                        .add_modifier(Modifier::BOLD),
                ),
                data_span,
            ];
            spans.splice(1..1, bars);
            // The flow gutter (pointer arrow or control-flow graph) sits
            // between the store text and the comment column, so the arrows
            // keep their fixed cells whatever width is left for the comments
            if let Some(g) = &graph {
                spans.extend(g.row_spans(row));
            } else if let Some(cell) = &gutter[row] {
                spans.push(cell.clone());
            } else if self.flow == FlowMode::Data {
                // Keep the comment column aligned when no arrow is drawn
                spans.push(Span::raw(" ".repeat(ARROW_EXTRA_WIDTH as usize)));
            }
            // The source comment of the word's line, in a fixed-width column
            // at the right edge, with its original indentation after the
            // ';'; dimmed like a comment in an editor. The row being edited
            // draws the inline text box instead: the buffer with a cursor,
            // highlighted like the selected row of the breakpoint list.
            if comment_width > 0 {
                if self.comment_editor.is_open_for(index) {
                    // Record the text box's rect for mouse hit-testing (a
                    // click outside it closes the editor). When the edited
                    // row is scrolled out of view no box is drawn, so the
                    // rect is emptied.
                    let box_row = row.saturating_sub(self.store_scroll as usize);
                    if row >= self.store_scroll as usize && box_row < area.height as usize {
                        self.comment_editor.set_last_rect(Rect::new(
                            area.x + comment_column_start(heatmap, self.flow) as u16,
                            area.y + box_row as u16,
                            comment_extra as u16,
                            1,
                        ));
                    } else {
                        self.comment_editor.set_last_rect(Rect::default());
                    }
                    // The cursor is drawn at the actual editing position:
                    // right after the last character of the buffer (typing
                    // appends, Backspace pops, so the insertion point is
                    // always the end). When the column is narrower than
                    // the buffer (narrow terminal or the width cap), the
                    // tail of the buffer is shown so the cursor stays at
                    // the end of the text instead of mid-buffer.
                    let raw = self.comment_editor.buffer();
                    let raw_len = raw.chars().count();
                    // One cell of the column is kept for the cursor
                    let box_width = comment_width.saturating_sub(1);
                    let visible: String = if raw_len > box_width {
                        raw.chars().skip(raw_len - box_width).collect()
                    } else {
                        raw.to_string()
                    };
                    let text = format!(" ;{visible}_");
                    // Pad the rest of the column so the highlight covers it fully
                    let pad = comment_extra.saturating_sub(2 + visible.chars().count() + 1);
                    spans.push(Span::styled(
                        format!("{text}{}", " ".repeat(pad)),
                        Style::default()
                            .fg(self.palette().dim)
                            .add_modifier(Modifier::REVERSED),
                    ));
                } else {
                    match &snap.comments[index] {
                        Some(text) => {
                            let truncated: String = text.chars().take(comment_width).collect();
                            spans.push(Span::styled(
                                format!(" ;{truncated:<comment_width$}"),
                                Style::default().fg(self.palette().dim),
                            ));
                        }
                        None if no_comments => {
                            // No comment anywhere yet: a dim " ;" gutter
                            // marks the column as the place to double-click
                            // for adding one (same total width as padding)
                            spans.push(Span::styled(
                                format!(" ;{}", " ".repeat(comment_width)),
                                Style::default().fg(self.palette().dim),
                            ));
                        }
                        None => spans.push(Span::raw(" ".repeat(comment_extra))),
                    }
                }
            } else if self.comment_editor.is_open_for(index) {
                // No room for the comment column: no text box drawn
                self.comment_editor.set_last_rect(Rect::default());
            }
            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(lines).scroll((self.store_scroll, 0));
        f.render_widget(paragraph, area);

        // Zebra stripes: every other line of the main body gets a subtle
        // background across the full width — but only the machine's own
        // lines (the CONTENT_HEIGHT content rows: spacers, CI, accumulator,
        // store words); below them the background stays plain. The parity
        // follows the content row (not the screen row), so the stripes stay
        // in place while the store view is scrolled. Applied on the buffer
        // because a line's style only covers the cells its spans draw.
        let buf = f.buffer_mut();
        for i in 0..area.height as usize {
            let content_row = self.store_scroll as usize + i;
            if content_row >= CONTENT_HEIGHT {
                break;
            }
            if content_row % 2 == 1 {
                buf.set_style(
                    Rect {
                        x: area.x,
                        y: area.y + i as u16,
                        width: area.width,
                        height: 1,
                    },
                    Style::default().bg(self.palette().zebra_bg),
                );
            }
        }

        // Scrollbar on the right edge when the window is too short for the
        // whole body: a solid dim track with the thumb in the chrome color
        // showing where `store_scroll` sits. No begin/end arrows, so the
        // track spans the whole body height. The state's content length is
        // `max_scroll + 1` (not CONTENT_HEIGHT) so the thumb is sized
        // proportionally to the visible rows and reaches the bottom of the
        // track exactly at the maximum scroll.
        let visible_height = area.height as usize;
        let max_scroll = CONTENT_HEIGHT.saturating_sub(visible_height);
        if max_scroll > 0 {
            let mut state = ScrollbarState::new(max_scroll + 1)
                .viewport_content_length(visible_height)
                .position(self.store_scroll as usize);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .track_symbol(Some(symbols::block::FULL))
                .begin_symbol(None)
                .end_symbol(None)
                .style(Style::default().fg(self.palette().dim))
                .thumb_style(Style::default().fg(self.palette().chrome));
            f.render_stateful_widget(scrollbar, area, &mut state);
        }
    }

    /// Draw the timeline bar: how much history is retained (filled bar) and
    /// where the machine currently is (marker). Dragging it scrubs the history.
    fn draw_timeline(&self, f: &mut Frame, area: Rect, snap: &Snapshot) {
        // The line is centered in the area, matching the offset computed
        // by `timeline_bar_geometry` (used for mouse hit-testing)
        if snap.history_capacity == 0 {
            let line = Line::from(Span::styled(
                t!("history.disabled"),
                Style::default().fg(self.palette().dim),
            ));
            f.render_widget(Paragraph::new(line).alignment(Alignment::Center), area);
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

        let palette = self.palette();
        let chrome = Style::default().fg(palette.chrome);
        let fill = Style::default().fg(palette.timeline_fill);
        let track = Style::default().fg(palette.timeline_track);
        let mut spans = vec![Span::styled(t!("history.label"), chrome)];
        if bar_width > 0 {
            // The whole bar is full blocks: the retained part in the tainted
            // fill color, the empty track in the dimmer track color
            let push_segment = |spans: &mut Vec<Span>, from: usize, to: usize| {
                let cut = filled.clamp(from, to);
                if from < cut {
                    spans.push(Span::styled("\u{2588}".repeat(cut - from), fill));
                }
                if cut < to {
                    spans.push(Span::styled("\u{2588}".repeat(to - cut), track));
                }
            };
            let marker = marker.unwrap_or(0);
            push_segment(&mut spans, 0, marker);
            spans.push(Span::styled(
                "\u{2588}".to_string(),
                Style::default()
                    .fg(palette.timeline_marker)
                    .add_modifier(Modifier::BOLD),
            ));
            push_segment(&mut spans, marker + 1, usize::from(bar_width));
        }
        spans.push(Span::styled(
            t!(
                "history.range",
                start = snap.history_start,
                end = snap.history_head
            ),
            chrome,
        ));

        f.render_widget(
            Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
            area,
        );
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

        let text = t!("history.scrub_cycle", n = snap.cycle_count);
        let width = text.chars().count() as u16;
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
            Paragraph::new(text).style(
                Style::default()
                    .fg(self.palette().scrub_popup_fg)
                    .bg(self.palette().scrub_popup_bg),
            ),
            popup_area,
        );
    }

    /// Draw the status bar (the bottom row): a solid background contrasting
    /// with the interface, with the blocks (STATUS, CYCLES, last instruction
    /// on the left; heatmap mode, flow mode, comments, breakpoints flush to
    /// the right edge) in a slightly different value of that color and
    /// without brackets, so they detach from the bar.
    fn draw_status_bar(
        &self,
        f: &mut Frame,
        area: Rect,
        snap: &Snapshot,
        heatmap: HeatmapMode,
        comment_width: usize,
    ) {
        let palette = self.palette();
        // The bar itself: a solid background over the whole row, the text
        // in the status bar's own foreground color
        let bar = Style::default().fg(palette.status_fg).bg(palette.status_bg);
        // A block: the same color family, one value off, padded with one
        // cell on each side
        let block = Style::default()
            .fg(palette.status_fg)
            .bg(palette.status_block_bg);
        f.buffer_mut().set_style(area, bar);

        let (status, status_color) = if self.scrubbing {
            (t!("status.rewind"), palette.status_rewind)
        } else if snap.stop_flag {
            (t!("status.stopped"), palette.status_stopped)
        } else {
            (t!("status.running"), palette.status_running)
        };
        let last_instr = snap
            .last_instruction
            .map(|li| li.to_string())
            .unwrap_or_else(|| "-- -- --".to_string());

        // The left group: status (the word keeps its state color), cycles,
        // last instruction, and the speed (smoothed display over the target)
        let left_blocks: Vec<Vec<Span<'static>>> = vec![
            vec![
                Span::styled(t!("status.label"), block),
                Span::styled(status, block.fg(status_color)),
            ],
            vec![Span::styled(
                t!("status.cycles", count = snap.cycle_count),
                block,
            )],
            vec![Span::styled(last_instr, block)],
            vec![Span::styled(
                t!(
                    "status.speed",
                    displayed = self.displayed_speed : {:>2},
                    target = snap.speed
                ),
                block,
            )],
        ];

        // The right group: the view modes and the breakpoints
        let mut right_blocks: Vec<Vec<Span<'static>>> = Vec::new();
        if let Some(label) = heatmap.label() {
            right_blocks.push(vec![Span::styled(
                t!("status.heatmap", label = label),
                block,
            )]);
        }
        if let Some(label) = self.flow.label() {
            right_blocks.push(vec![Span::styled(t!("status.flow", label = label), block)]);
        }
        if comment_width > 0 {
            right_blocks.push(vec![Span::styled(t!("status.comments"), block)]);
        }
        if !snap.breakpoints.is_empty() {
            right_blocks.push(vec![Span::styled(
                t!("status.breakpoints", count = snap.breakpoints.len()),
                block,
            )]);
        }

        let left = Self::status_line(left_blocks, bar, block);
        let right = Self::status_line(right_blocks, bar, block);
        let right_width = right.width() as u16;
        if right_width < area.width {
            let chunks = Layout::horizontal([Constraint::Min(0), Constraint::Length(right_width)])
                .split(area);
            f.render_widget(Paragraph::new(left), chunks[0]);
            f.render_widget(Paragraph::new(right), chunks[1]);
        } else {
            // No room for the right group: the left group alone
            f.render_widget(Paragraph::new(left), area);
        }
    }

    /// Assemble one status-bar line from its blocks: each block is padded
    /// with one cell in the block color on each side, and blocks are
    /// separated by one cell in the bar color, so the blocks read as
    /// detached segments (no brackets).
    fn status_line(blocks: Vec<Vec<Span<'static>>>, bar: Style, block: Style) -> Line<'static> {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (i, inner) in blocks.into_iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ", bar));
            }
            spans.push(Span::styled(" ", block));
            spans.extend(inner);
            spans.push(Span::styled(" ", block));
        }
        Line::from(spans)
    }
}
