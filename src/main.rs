use std::path::PathBuf;

use clap::{CommandFactory, FromArgMatches, Parser};
use rust_i18n::t;

use crate::i18n::Language;
use crate::ssem::history::MAX_CAPACITY;
use crate::ssem::simulator::Simulator;
use crate::tui::Theme;

/// Like [`rust_i18n::t!`], but returns an owned `String` (for fields and
/// return types that expect `String` rather than `Cow<str>`).
#[macro_export]
macro_rules! ts {
    ($($arg:tt)*) => {
        rust_i18n::t!($($arg)*).into_owned()
    };
}

pub mod i18n;
pub mod ssem;
pub mod tui;

rust_i18n::i18n!("locales", fallback = "en");

#[derive(Parser)]
#[command(author, about, long_about = None)]
struct Args {
    /// Stop after this amount of cycles
    #[arg(short, long, value_name = "NUM", default_value_t = 100_000_000)]
    max_cycles: u32,

    /// Run without the interactive TUI; print the final state and exit
    #[arg(long)]
    headless: bool,

    /// Number of past machine states kept for time-travel in the interactive
    /// interface (0 to disable)
    #[arg(long, value_name = "NUM", default_value_t = 100_000)]
    history_size: usize,

    /// Color theme of the interactive interface, overriding the one saved in
    /// the config file for this session (the T key cycles themes at runtime)
    #[arg(long, value_enum)]
    theme: Option<Theme>,

    /// Language of the interface, overriding the one saved in the config file
    /// for this session
    #[arg(long, value_enum)]
    lang: Option<Language>,

    /// Write the machine state to this file once the run ends (.asm or .snp,
    /// with the source comments); in the interactive interface, the S key
    /// saves the current state at any time
    #[arg(long, value_name = "FILE")]
    save: Option<PathBuf>,

    /// Input file to initialize the store. Can be .asm or .snp format.
    /// When omitted, the machine starts with an empty (all-zero) store
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

/// The clap command with localized help texts. The derive attributes read
/// compile-time doc comments, so the help strings are overridden here at
/// runtime, after the locale has been set.
fn localized_command() -> clap::Command {
    Args::command()
        .about(t!("cli.about"))
        .mut_arg("max_cycles", |a| a.help(t!("cli.help.max_cycles")))
        .mut_arg("headless", |a| a.help(t!("cli.help.headless")))
        .mut_arg("history_size", |a| a.help(t!("cli.help.history_size")))
        .mut_arg("theme", |a| a.help(t!("cli.help.theme")))
        .mut_arg("lang", |a| a.help(t!("cli.help.lang")))
        .mut_arg("save", |a| a.help(t!("cli.help.save")))
        .mut_arg("file", |a| a.help(t!("cli.help.file")))
}

fn main() {
    // Resolve the interface language before anything prints, so that even
    // --help is localized. --lang is pre-scanned from the raw arguments
    // because clap's own help needs the locale before parsing.
    let lang = i18n::resolve(i18n::prescan_lang());
    rust_i18n::set_locale(lang.code());

    let matches = localized_command().get_matches();
    let args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    let mut simulator = match &args.file {
        Some(file) => Simulator::from_file(file),
        None => Simulator::new(),
    };

    if args.headless {
        // Headless mode: run to completion (or max-cycles) and print the result
        simulator.run(args.max_cycles);
    } else {
        // The interactive interface records the execution history for time-travel
        if args.history_size > 0 {
            let history_size = args.history_size.min(MAX_CAPACITY);
            if history_size < args.history_size {
                eprintln!(
                    "{}",
                    t!(
                        "main.history_clamped",
                        given = args.history_size,
                        max = MAX_CAPACITY
                    )
                );
            }
            simulator.enable_history(history_size);
        }
        // Launch the interactive TUI; it returns the simulator when the user quits
        simulator = match tui::run(simulator, args.max_cycles, args.theme) {
            Ok(sim) => sim,
            Err(e) => {
                eprintln!("{}", t!("main.error", error = e.to_string()));
                eprintln!("{}", t!("main.tty_hint"));
                std::process::exit(1);
            }
        };
    }

    // Print the final state
    println!("{}", t!("main.final_state"));
    println!("{simulator}");

    // Save the final state when asked to (the format follows the extension)
    if let Some(path) = &args.save {
        match simulator.save(path) {
            Ok(()) => println!("{}", t!("main.saved", path = path.display().to_string())),
            Err(e) => {
                eprintln!(
                    "{}",
                    t!(
                        "main.save_error",
                        path = path.display().to_string(),
                        error = e.to_string()
                    )
                );
                std::process::exit(1);
            }
        }
    }
}
