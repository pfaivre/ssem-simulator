//! The color themes of the interface. Every color used by the TUI lives in
//! the [`Palette`] struct, and each built-in theme in its own file under
//! this directory (`base16ocean.rs`, `notebook.rs`, …), so a theme can be tweaked —
//! or a new one added — without touching the rendering code.
//!
//! To add a theme: create a file here with `pub const PALETTE: Palette` and
//! `pub const LABEL: &str`, then add one line to the [`themes!`] invocation
//! below (`MyTheme = mytheme,`) — it declares the module, the [`Theme`]
//! variant (its doc comment becomes the `--theme` help text), and the
//! [`Theme::ALL`] entry (label, palette, cycle order) in one place.
//!
//! Themes are picked with the `--theme` command-line option, or cycled at
//! runtime with the T key (through [`Theme::ALL`]).
//!
//! The fixed-color themes use exact RGB values (written as hex literals
//! via the [`rgb`] helper), so their appearance does not depend on the
//! terminal's color scheme; the terminal theme defers to the terminal's own
//! colors instead.

use clap::ValueEnum;
use ratatui::style::Color;
use serde::{Deserialize, Serialize};

use crate::ssem::Opcode;

/// Declare the built-in themes in one list. Each entry generates the theme's
/// module, its [`Theme`] variant (the doc comment becomes the `--theme` help
/// text), and its entry in [`Theme::ALL`] (from the module's own `LABEL` and
/// `PALETTE` consts).
macro_rules! themes {
    (
        $(
            $(#[$meta:meta])*
            $variant:ident = $module:ident
        ),* $(,)?
    ) => {
        $(
            pub mod $module;
        )*

        /// The built-in color themes.
        ///
        /// Serialized in kebab-case, so the config file accepts the same
        /// spellings as the `--theme` command-line option.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum Theme {
            $(
                $(#[$meta])*
                $variant,
            )*
        }

        impl Theme {
            /// Every built-in theme with its name and palette, in the order
            /// the T key cycles through them: the dark themes first and the
            /// light ones last, so cycling never flashes between a dark and
            /// a light theme; `terminal`, which follows the terminal's own
            /// colors, goes last.
            pub const ALL: &[(Theme, &'static str, Palette)] = &[
                $( (Theme::$variant, $module::LABEL, $module::PALETTE) ),*
            ];
        }
    };
}

themes! {
    /// The Base16 Ocean theme: dark, blue-tinted (the default).
    Base16Ocean = base16ocean,
    /// A Monokai-inspired theme: dark background, bright accents.
    Monokai = monokai,
    /// A Catppuccin Macchiato theme: dark, with pastel accents.
    CatppuccinMacchiato = catppuccinmacchiato,
    /// The Dracula theme: dark, with purple and pink accents.
    Dracula = dracula,
    /// The Gruvbox theme: dark, with retro warm colors.
    Gruvbox = gruvbox,
    /// The Rose Pine theme: dark, with soft pastels.
    RosePine = rosepine,
    /// The Notebook theme: light, like a paper notebook.
    Notebook = notebook,
    /// A Catppuccin Latte theme: light, with pastel accents.
    CatppuccinLatte = catppuccinlatte,
    /// The Gruvbox Light theme: light, with retro warm colors.
    GruvboxLight = gruvboxlight,
    /// The Rose Pine Dawn theme: light, with soft pastels.
    RosePineDawn = rosepinedawn,
    /// The terminal's own colors: default fg/bg, standard ANSI accents.
    Terminal = terminal,
}

/// Build a color from a hex literal: `rgb(0x6C9EF8)`.
pub const fn rgb(hex: u32) -> Color {
    Color::Rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

/// Blend `from` toward `to` by `num / den` (`blend(a, b, 1, 2)` is the
/// midpoint). Only RGB colors have channels to interpolate; any other pair
/// collapses to whichever endpoint the blend is at least halfway toward.
pub const fn blend(from: Color, to: Color, num: u32, den: u32) -> Color {
    const fn channel(a: u8, b: u8, num: u32, den: u32) -> u8 {
        let weighted = a as i32 * (den - num) as i32 + b as i32 * num as i32;
        ((2 * weighted + den as i32) / (2 * den as i32)) as u8
    }
    let num = if num > den { den } else { num };
    match (from, to) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => Color::Rgb(
            channel(r1, r2, num, den),
            channel(g1, g2, num, den),
            channel(b1, b2, num, den),
        ),
        _ => {
            if 2 * num >= den {
                to
            } else {
                from
            }
        }
    }
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
    /// The mouse-hover highlight (the menu bar buttons and the hovered
    /// settings control): the hovered button's background, and the text
    /// color on top of it.
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
    /// Background of the alternating ("zebra") lines of the main body: `bg`
    /// blended a faint step (1/16) toward `chrome` — slightly lighter on dark
    /// themes, slightly darker on light ones.
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

    /// The retained-history fill of the timeline bar, drawn as full blocks
    /// in this color: `bg` blended halfway to `chrome`, the perceived fill
    /// of the half-shade glyph (U+2592) the bar drew before.
    pub timeline_fill: Color,
    /// The empty track of the timeline bar, drawn as full blocks in this
    /// color: `bg` blended a quarter of the way to `chrome`, the perceived
    /// fill of the light-shade glyph (U+2591) the bar drew before.
    pub timeline_track: Color,
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

impl Theme {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_interpolates_rgb_channels() {
        assert_eq!(blend(rgb(0x000000), rgb(0xFFFFFF), 0, 2), rgb(0x000000));
        assert_eq!(blend(rgb(0x000000), rgb(0xFFFFFF), 2, 2), rgb(0xFFFFFF));
        assert_eq!(blend(rgb(0x000000), rgb(0xFFFFFF), 1, 2), rgb(0x808080));
        assert_eq!(blend(rgb(0x000000), rgb(0xFFFFFF), 1, 4), rgb(0x404040));
        assert_eq!(blend(rgb(0x2B303B), rgb(0x8CA1B4), 1, 2), rgb(0x5C6978));
    }

    #[test]
    fn blend_of_named_colors_collapses_to_the_nearer_endpoint() {
        assert_eq!(blend(Color::Red, Color::Blue, 1, 4), Color::Red);
        assert_eq!(blend(Color::Red, Color::Blue, 1, 2), Color::Blue);
        assert_eq!(blend(Color::Red, Color::Blue, 3, 4), Color::Blue);
    }

    #[test]
    fn zebra_bg_is_a_faint_blend_of_bg_and_chrome() {
        for (theme, _, palette) in Theme::ALL {
            assert_eq!(
                palette.zebra_bg,
                blend(palette.bg, palette.chrome, 1, 16),
                "{theme:?}"
            );
        }
    }

    #[test]
    fn timeline_colors_are_blends_of_bg_and_chrome() {
        for (theme, _, palette) in Theme::ALL {
            assert_eq!(
                palette.timeline_fill,
                blend(palette.bg, palette.chrome, 1, 2),
                "{theme:?}"
            );
            // Named colors (the terminal theme) have no channels to blend
            // a quarter way, so only RGB themes get the exact track blend
            if let (Color::Rgb(..), Color::Rgb(..)) = (palette.bg, palette.chrome) {
                assert_eq!(
                    palette.timeline_track,
                    blend(palette.bg, palette.chrome, 1, 4),
                    "{theme:?}"
                );
            }
        }
    }
}
