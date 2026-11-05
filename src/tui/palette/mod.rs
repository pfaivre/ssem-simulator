//! The color themes of the interface. Every color used by the TUI lives in
//! the [`Palette`] struct, and each built-in theme in its own file under
//! this directory (`dark.rs`, `light.rs`, …), so a theme can be tweaked —
//! or a new one added — without touching the rendering code.
//!
//! To add a theme: create a file here with a `pub const MY_THEME: Palette`,
//! add a `Theme` variant (its doc comment becomes the `--theme` help text),
//! and add one entry to [`Theme::ALL`] — that table is the single place
//! where labels, palettes, and cycle order are wired together.
//!
//! Themes are picked with the `--theme` command-line option, or cycled at
//! runtime with the T key (through [`Theme::ALL`]).
//!
//! The dark and light themes use exact RGB values (written as hex literals
//! via the [`rgb`] helper), so their appearance does not depend on the
//! terminal's color scheme; the terminal theme defers to the terminal's own
//! colors instead.

pub mod catppuccinmacchiato;
pub mod dark;
pub mod light;
pub mod monokai;
pub mod terminal;

use clap::ValueEnum;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::ssem::Opcode;

pub use catppuccinmacchiato::CATPPUCCIN_MACCHIATO;
pub use dark::DARK;
pub use light::LIGHT;
pub use monokai::MONOKAI;
pub use terminal::TERMINAL;

/// Build a color from a hex literal: `rgb(0x6C9EF8)`.
pub const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// All the colors used by the interface, by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The default text color of the whole interface (instead of the
    /// terminal's default foreground); text without an explicit color
    /// inherits it.
    pub fg: Color,
    /// The base background of the whole interface (instead of the terminal's
    /// default background). The zebra stripes of the main body are drawn on
    /// top of it.
    pub bg: Color,
    /// Chrome text: menu bar, timeline bar, status-bar labels.
    pub chrome: Color,
    /// The status word in the status bar, by machine state.
    pub status_running: Color,
    pub status_stopped: Color,
    pub status_rewind: Color,
    /// The status bar's text color (the labels around the status word);
    /// independent from the bar's background so the two can be tuned apart.
    pub status_fg: Color,
    /// The mouse-hover highlight (currently the menu bar buttons): the
    /// hovered button's background, and the text color on top of it.
    pub hover: Color,
    pub hover_fg: Color,
    /// The menu bar (top row): a solid background contrasting with `bg`,
    /// and the slightly different value of that color used by the buttons
    /// inside it, so they detach from the bar.
    pub menu_bg: Color,
    pub menu_block_bg: Color,
    /// The status bar (bottom row): a solid background contrasting with
    /// `bg`, and the slightly different value of that color used by the
    /// blocks inside it (STATUS, CYCLES, …), so they detach from the bar.
    /// Independent from the menu bar's colors.
    pub status_bg: Color,
    pub status_block_bg: Color,
    /// Key caps in the bottom bar and the help pop-up (drawn reversed).
    pub key_hint: Color,
    /// Reversed highlight of the selected row in the breakpoint list.
    pub selection: Color,

    /// CI, accumulator, and store-word bits (the machine's values).
    pub value: Color,
    /// The destination of the last data movement (the arrow's target line).
    pub highlight: Color,
    /// The `»` marker of the next line to execute.
    pub next_marker: Color,
    /// Dimmed text: `•` executed marker, source comments, cold heatmap bars,
    /// implicit fall-through edges.
    pub dim: Color,
    /// Background of the alternating ("zebra") lines of the main body.
    pub zebra_bg: Color,

    /// Mnemonic and pointer-arrow colors, by operation category. The two
    /// share the palette so a mnemonic's color tells the same story as the
    /// arrow its instruction draws. The hue families follow the meaning:
    /// jumps and their graph edges share one family, memory accesses
    /// (LDN/STO and the read heatmap) another, and red is reserved for
    /// halting (STP and breakpoints).
    pub op_jump: Color,
    pub op_memory: Color,
    pub op_arithmetic: Color,
    pub op_test: Color,
    pub op_halt: Color,
    pub op_num: Color,

    /// Heatmap bar ramps (low, mid, high). Each ramp stays in the hue family
    /// of the feature it measures: exec in the jump/control-flow family,
    /// reads in the memory family, writes in the data-destination
    /// (highlight) family.
    pub exec_ramp: [Color; 3],
    pub read_ramp: [Color; 3],
    pub write_ramp: [Color; 3],

    /// Control-flow graph edges, colored after the instruction family that
    /// creates them: a jump edge takes `op_jump` (a backward edge is the
    /// same color, bold), a CMP skip edge takes `op_test`, and a plain
    /// fall-through is dimmed.
    pub edge_fallthrough: Color,
    pub edge_jump: Color,
    pub edge_skip: Color,

    /// The position marker on the timeline bar.
    pub timeline_marker: Color,
    /// The scrub pop-up (cycle number above the timeline): foreground and
    /// background.
    pub scrub_popup_fg: Color,
    pub scrub_popup_bg: Color,

    /// Pop-up border colors.
    pub breakpoint_border: Color,
    pub editor_border: Color,
    pub save_border: Color,
    /// The live preview line of the editor pop-up.
    pub preview: Color,
}

impl Palette {
    /// The syntax-highlight color of an instruction mnemonic, by category:
    /// a jump (JMP, JRP), a memory access (LDN, STO), an arithmetic
    /// combination (SUB, SUB2), a test (CMP), a halt (STP). NUM never
    /// reaches the display (`Store::decode_word` always yields a real
    /// opcode); it is mapped anyway to keep the match exhaustive.
    pub fn opcode(&self, opcode: Opcode) -> Color {
        match opcode {
            Opcode::JMP | Opcode::JRP => self.op_jump,
            Opcode::LDN | Opcode::STO => self.op_memory,
            Opcode::SUB | Opcode::SUB2 => self.op_arithmetic,
            Opcode::CMP => self.op_test,
            Opcode::STP => self.op_halt,
            Opcode::NUM => self.op_num,
        }
    }
}

/// The built-in color themes.
///
/// Serialized in kebab-case, so the config file accepts the same spellings
/// as the `--theme` command-line option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    /// For terminals with a dark background (the default).
    Dark,
    /// For terminals with a light background.
    Light,
    /// A Monokai-inspired theme: dark background, bright accents.
    Monokai,
    /// A Catppuccin Macchiato theme: dark, with pastel accents.
    CatpuccinMacchiato,
    /// The terminal's own colors: default fg/bg, standard ANSI accents.
    Terminal,
}

impl Theme {
    /// Every built-in theme with its name and palette, in the order the T
    /// key cycles through them. `palette`, `label`, and `cycle` are all
    /// derived from this table, so it is the only place to extend.
    pub const ALL: &[(Theme, &'static str, Palette)] = &[
        (Theme::Dark, "Dark", DARK),
        (Theme::Light, "Light", LIGHT),
        (Theme::Monokai, "Monokai", MONOKAI),
        (
            Theme::CatpuccinMacchiato,
            "Catppuccin Macchiato",
            CATPPUCCIN_MACCHIATO,
        ),
        (Theme::Terminal, "terminal", TERMINAL),
    ];

    /// This theme's entry in [`Theme::ALL`].
    fn entry(self) -> &'static (Theme, &'static str, Palette) {
        Self::ALL
            .iter()
            .find(|(t, _, _)| *t == self)
            .expect("every Theme variant must have an entry in Theme::ALL")
    }

    /// This theme's palette: a reference into the static [`Theme::ALL`]
    /// table, so reading colors through it does not copy the `Palette`.
    pub fn palette(self) -> &'static Palette {
        &self.entry().2
    }

    /// The theme's name, shown in the top bar.
    pub fn label(self) -> &'static str {
        self.entry().1
    }

    /// The next theme in [`Theme::ALL`], wrapping around.
    pub fn cycle(self) -> Self {
        let i = Self::ALL
            .iter()
            .position(|(t, _, _)| *t == self)
            .unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()].0
    }
}
