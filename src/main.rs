use std::path::PathBuf;

use clap::Parser;

use crate::ssem::history::MAX_CAPACITY;
use crate::ssem::simulator::Simulator;
use crate::tui::Theme;

pub mod ssem;
pub mod tui;

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

fn main() {
    let args = Args::parse();

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
                    "Warning: --history-size {} is too large, clamped to {MAX_CAPACITY} (~14.8 GB max)",
                    args.history_size,
                );
            }
            simulator.enable_history(history_size);
        }
        // Launch the interactive TUI; it returns the simulator when the user quits
        simulator = match tui::run(simulator, args.max_cycles, args.theme) {
            Ok(sim) => sim,
            Err(e) => {
                eprintln!("Error: {e}");
                eprintln!(
                    "The interactive interface requires a terminal. Run this program in a TTY, or use --headless."
                );
                std::process::exit(1);
            }
        };
    }

    // Print the final state
    println!("Final state of the machine:");
    println!("{simulator}");

    // Save the final state when asked to (the format follows the extension)
    if let Some(path) = &args.save {
        match simulator.save(path) {
            Ok(()) => println!("Machine state saved to {}", path.display()),
            Err(e) => {
                eprintln!("Error: could not save to {}: {e}", path.display());
                std::process::exit(1);
            }
        }
    }
}
