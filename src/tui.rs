//! Interactive terminal interface for the SSEM simulator, built with ratatui.
//!
//! # Architecture: two threads
//!
//! The simulation and the interface run on separate threads, so that a high
//! target speed cannot starve the UI of CPU time:
//!
//! - **Machine thread**: runs the simulation at the target speed. It locks the
//!   simulator, executes a time-limited batch of cycles, releases the lock, and
//!   sleeps until the batch deadline. The short batch duration (~1 ms) keeps the
//!   lock available for the UI thread.
//! - **UI thread** (main): polls for keyboard input at ~60 fps, takes a state
//!   snapshot under a brief lock, and draws without holding the lock (so terminal
//!   I/O never blocks the machine thread).
//!
//! Shared state lives in an `Arc<Mutex<Simulator>>`; a separate
//! `Arc<AtomicBool>` signals the machine thread to exit.
//!
//! The interface is composed of four areas:
//! - A top bar showing status indicators (status, cycles, last instruction, speed, breakpoints)
//! - A main body showing the machine state (CI, accumulator, and store words)
//! - A timeline bar showing the retained execution history
//! - A bottom bar showing the keyboard shortcuts
//!
//! Two pop-ups are available: a help pop-up (H key) and a breakpoint editor (B key).
//! A third pop-up, the memory/register editor (E key), lets you modify CI, A, or store words.
//!
//! Mouse: double-click a line (CI, A, or a store word) to open the editor for it;
//! scroll wheel scrolls the store view.
//!
//! Time-travel: the timeline bar shows the retained execution history. Drag it
//! with the mouse (or use , . Home End, or the scroll wheel over it) to move
//! through the recorded cycles; the machine pauses and its state is restored to
//! the scrubbed cycle, and a small pop-up above the bar's position marker shows
//! the current cycle. Press P to resume from there — re-executing overwrites
//! the previously recorded future. Browsing the history resets the heatmap;
//! the last-instruction display follows the browsed cycle.
//!
//! Press M to cycle the heatmap shown next to each store word: off, the
//! execution heatmap (a colored bar shows how often each word has been
//! executed; green = cold, red = hot), or the read/write heatmap (two small
//! labeled bars per word: `r` for reads, `w` for writes).
//!
//! Press F to cycle the flow visualization shown in the right margin, like
//! the heatmap. The default mode, data, draws a pointer arrow following the
//! data flow of the last executed instruction (the line marked `>`) — STO points from the
//! accumulator to the destination word, LDN from the source word to the
//! accumulator, and the jumps from the word holding the target value to CI.
//! Magenta marks a load or store, cyan an arithmetic combination (SUB, JRP).
//! The destination is highlighted. After a jump, CI holds the target value
//! rather than the executed line, so the marker and the arrow follow the
//! executed instruction, not CI.
//!
//! The second mode, control, draws the static graph of the non-sequential
//! edges between the store lines — JMP/JRP jumps in yellow,
//! a taken CMP skip in white — with backward edges (loops) highlighted in
//! bold red and STP lines marked with a red `■`. Fall-through to the next
//! line is left implicit. Only lines reachable from the entry line (line 1)
//! get edges, so data words do not spawn spurious ones. Edges that would
//! need more lanes than available are labeled `⇢NN` on their source line.
//! The graph is computed from the current store contents, so it follows live
//! edits and history scrubbing. The third mode hides the visualization.
//!
//! Press C to toggle the source-comment column: the `;` comment found on each
//! word's line in the input file is shown next to the disassembly
//! (`01 LDN 24  ; A = -a`), dimmed, with its original indentation, in a
//! fixed-width column (long comments are truncated to 80 cells). The flow gutter (arrow or graph) is drawn between
//! the disassembly and the comment column, so the arrows keep their fixed
//! cells; when the terminal is narrow, the comments are cropped to the width
//! left after the store text, the heatmap, and the gutter instead of being
//! hidden. The column is on by default; with a comment-free input it renders
//! nothing. The headless final-state dump prints the comments too.
//!
//! Mnemonics are syntax-highlighted by category, reusing the arrows' palette
//! so the two stay coherent: magenta for a data movement (LDN, STO, JMP),
//! cyan for an arithmetic combination (SUB, SUB2, JRP), white for CMP (test)
//! and red for STP (halt). The chrome (top bar, shortcut bar, timeline) keeps
//! a single blue; the help pop-up documents the colors.
//!
//! When the machine is paused, the pointer column shows the execution
//! footprint of the current run: `>` marks the last executed line, `»` the
//! next line to execute (where P or F10 resumes), and `•` every other line
//! executed at least once. Like the heatmap, the footprint is reset when
//! browsing the history.
//!
//! # Module map
//!
//! - `theme`: mnemonic colors, heatmap/flow modes, keymap, bit glyphs
//! - `layout`: geometry constants and timeline-bar helpers
//! - `snapshot`: the lock-free state snapshot used for drawing
//! - `graph`: the static control-flow graph (flow mode "control")
//! - `app`: application state and input handling
//! - `draw`: rendering of the bars, main body, and pop-ups
//! - `runner`: the machine/UI threads and the entry point
//! - `popups`: the help, breakpoint, and editor pop-ups

mod app;
mod draw;
mod graph;
mod layout;
mod popups;
mod runner;
mod snapshot;
mod theme;

#[cfg(test)]
mod tests;

pub use runner::run;
