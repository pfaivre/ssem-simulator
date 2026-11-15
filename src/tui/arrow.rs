//! The data-flow pointer arrow drawn next to the store view in the "data"
//! flow mode: the arrow for the last executed instruction, the gutter cells
//! that draw it, and its margin label.

use ratatui::style::{Color, Style};
use ratatui::text::Span;

use super::layout::{
    A_ROW, CI_ROW, CONTENT_HEIGHT, REGISTER_TEXT_WIDTH, STORE_ROW_OFFSET, STORE_TEXT_WIDTH,
};
use super::palette::Palette;
use super::snapshot::Snapshot;
use crate::ssem::Opcode;

/// A pointer arrow: where it starts and ends (in content rows of the main
/// body) and its color.
pub(super) struct PointerArrow {
    pub(super) source: usize,
    pub(super) target: usize,
    pub(super) color: Color,
}

impl PointerArrow {
    /// The arrow for the last executed instruction, following its data flow:
    ///
    /// - STO: from the accumulator to the destination word
    /// - LDN: from the source word to the accumulator
    /// - SUB/SUB2: like LDN, in a different color
    /// - JMP: from the word holding the target value to CI (a copy)
    /// - JRP: like JMP, in the same jump color (the value is added to CI)
    /// - CMP: from the accumulator to CI (its sign decides whether CI skips)
    ///
    /// The arrow takes the operation's category color, shared with the
    /// mnemonic and the graph edge: the memory color for LDN/STO, the jump
    /// color for JMP/JRP, the arithmetic color for SUB/SUB2, the test color
    /// for CMP. The arrow follows the executed instruction
    /// rather than CI: after a jump, CI holds the target value, so the word
    /// at CI is neither the last nor the next instruction to execute (CI is
    /// incremented before each fetch). Returns `None` when no instruction
    /// has executed yet, or for STP, which moves no data.
    pub(super) fn new(palette: &Palette, snap: &Snapshot) -> Option<Self> {
        let li = snap.last_instruction?;
        let word_row = STORE_ROW_OFFSET + li.data as usize;
        let (source, target) = match li.opcode {
            Opcode::STO => (A_ROW, word_row),
            Opcode::LDN | Opcode::SUB | Opcode::SUB2 => (word_row, A_ROW),
            Opcode::JMP | Opcode::JRP => (word_row, CI_ROW),
            Opcode::CMP => (A_ROW, CI_ROW),
            // STP ignores its data field and moves no data
            _ => return None,
        };
        Some(PointerArrow {
            source,
            target,
            color: palette.opcode(li.opcode),
        })
    }

    /// Gutter cells for the rows whose text spans the full store width
    /// (store words, and the blank row the arrow's vertical may cross).
    /// The register rows' cells (CI, accumulator) are built separately by
    /// [`PointerArrow::register_gutter`], since their text is shorter and
    /// the arrow must connect back to it.
    pub(super) fn store_gutter(&self) -> Vec<Option<Span<'static>>> {
        let style = Style::default().fg(self.color);
        let (top, bottom) = (self.source.min(self.target), self.source.max(self.target));
        // The corner leaving the source line turns toward the target;
        // the corner at the target line turns toward the arrowhead
        let (src_corner, dst_corner) = if self.target > self.source {
            ('\u{256e}', '\u{256f}') // ╮ ╯
        } else {
            ('\u{256f}', '\u{256e}') // ╯ ╮
        };
        let mut gutter: Vec<Option<Span<'static>>> = vec![None; CONTENT_HEIGHT];
        if !matches!(self.source, CI_ROW | A_ROW) {
            gutter[self.source] = Some(Span::styled(format!(" ──{src_corner}"), style));
        }
        for cell in gutter.iter_mut().take(bottom).skip(top + 1) {
            *cell = Some(Span::styled("   \u{2502}".to_string(), style));
        }
        if !matches!(self.target, CI_ROW | A_ROW) {
            gutter[self.target] = Some(Span::styled(
                format!(" \u{25c0}\u{2500}{dst_corner}"),
                style,
            ));
        }
        gutter
    }

    /// A register row's (CI, accumulator) gutter cell: a run of dashes
    /// connects the arrow back to the register's text. When the arrow's
    /// vertical crosses a register row that is not an endpoint (an arrow
    /// to CI crosses the accumulator), that row carries the vertical.
    pub(super) fn register_gutter(&self, row: usize) -> Option<Span<'static>> {
        let style = Style::default().fg(self.color);
        // Cells between the register's text and the store gutter
        // (the gutter sits right after the store text and the heatmap)
        let pad = STORE_TEXT_WIDTH - REGISTER_TEXT_WIDTH;
        if self.source == row {
            // The arrow leaves the register (STO, CMP)
            let corner = if self.target > self.source {
                '\u{256e}'
            } else {
                '\u{256f}'
            };
            Some(Span::styled(
                format!(" {}{corner}", "\u{2500}".repeat(pad + 2)),
                style,
            ))
        } else if self.target == row {
            // The arrow arrives at the register (LDN/SUB to A, JMP/JRP to CI)
            let corner = if self.target > self.source {
                '\u{256f}'
            } else {
                '\u{256e}'
            };
            Some(Span::styled(
                format!(" \u{25c0}{}{corner}", "\u{2500}".repeat(pad + 1)),
                style,
            ))
        } else if self.source.min(self.target) < row && row < self.source.max(self.target) {
            // The arrow's vertical crosses this register row
            Some(Span::styled(
                format!("{}   \u{2502}", " ".repeat(pad)),
                style,
            ))
        } else {
            None
        }
    }
}

/// The assignment made by the last executed instruction, as a margin
/// label on the arrow's top line: "CI += -3", "A = 128", "[23] = 54".
///
/// Loads, stores, and JMP are absolute (`=`, showing the new value);
/// arithmetic combinations (SUB, JRP) are relative (`+=`, with the
/// negated operand for SUB). CMP moves no store data, but its outcome is
/// labeled from A's sign — the predicate the machine applies, which skips
/// the next line when A is negative. STP moves no data and gets no label.
pub(super) fn assignment_label(snap: &Snapshot) -> Option<String> {
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
        Opcode::CMP => Some(if snap.a < 0 {
            "A < 0 → CI += 1".to_string()
        } else {
            "A >= 0".to_string()
        }),
        // STP moves no data; NUM never reaches the display
        // (`Store::decode_word` always yields a real opcode)
        Opcode::STP | Opcode::NUM => None,
    }
}
