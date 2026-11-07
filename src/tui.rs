//! Interactive terminal interface for the SSEM simulator, built with ratatui.
//!
//! # Architecture
//!
//! Two threads share the simulator through an `Arc<Mutex<Simulator>>` (plus an
//! `Arc<AtomicBool>` stop flag):
//!
//! - **Machine thread** (`runner`): executes cycles in short time-limited
//!   batches (~1 ms), releasing the lock between batches so it stays available
//!   to the UI thread, and sleeps until each batch deadline to hit the target
//!   speed.
//! - **UI thread** (main): polls input at ~60 fps, copies the simulator into a
//!   `Snapshot` under a brief lock, then draws from that snapshot without
//!   holding the lock, so terminal I/O never blocks the machine thread.
//!
//! The screen is four stacked areas — menu bar, main body (CI, accumulator,
//! store words, with an optional heatmap/flow gutter and comment column),
//! history scrub bar, status bar — with pop-ups (help, breakpoints, word
//! editor, save, settings, inline comment editor, value picker) drawn on top.
//!
//! # Working in this module
//!
//! - `App` (`app.rs`) is the only mutable UI state; it lives on the UI thread
//!   and owns input handling, pop-up state and history scrubbing. It touches
//!   the simulator only in short lock sections (edits, save) or via a
//!   `Snapshot`.
//! - `draw.rs` is pure rendering: it takes `&App`, `&Snapshot` and a `Frame`.
//! - All colors come from `palette/` (one file per theme); geometry constants
//!   and view modes (heatmap, flow, comments, bit glyphs, theme) live in
//!   `layout.rs` and persist to `config.toml` in the platform config dir
//!   (`config.rs`), written on exit.
//!
//! # Module map
//!
//! - `runner`: thread setup, machine batch loop, UI event loop, entry point `run`
//! - `app`: `App` state, keyboard/mouse handling, history scrubbing
//! - `snapshot`: point-in-time copy of the simulator used for drawing
//! - `draw`: rendering of the bars, main body, heatmaps, flow arrows, pop-ups
//! - `popups`: all pop-up widgets, the inline comment editor and value picker
//! - `layout`: geometry constants, timeline-bar helpers, view modes and widths
//! - `config`: persistent view modes (`config.toml` in the platform config dir)
//! - `graph`: static control-flow graph for the "control" flow mode
//! - `palette`: color themes — every UI color, one file per theme
//!
//! Where to add things: a new view mode goes in `layout.rs` (+ `config.rs` to
//! persist it, + `draw.rs` to render it, + `app.rs` for its key); a new pop-up
//! goes in `popups/` (one file per widget, re-exported in `popups/mod.rs`),
//! gets a `Popup` impl, a field on `App`, and one entry in `App::popups_mut`
//! — that array is the single list every key/mouse/close/draw site iterates;
//! a new theme is a new file in `palette/` plus a `Theme` variant; new
//! machine-thread behavior goes in `runner.rs`.

mod app;
mod config;
mod draw;
mod graph;
mod layout;
pub mod palette;
mod popups;
mod runner;
mod snapshot;

#[cfg(test)]
mod tests;

pub use palette::Theme;
pub use runner::run;
