use std::path::{Path, PathBuf};

use clap::Parser;

use crate::ssem::simulator::Simulator;

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

    /// Input file to initialize the store. Can be .asm or .snp format
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

fn main() {
    let args = Args::parse();

    let mut simulator = Simulator::from_file(Path::new(&args.file));

    if args.headless {
        // Headless mode: run to completion (or max-cycles) and print the result
        simulator.run(args.max_cycles);
    } else {
        // Launch the interactive TUI; it returns the simulator when the user quits
        simulator = match tui::run(simulator, args.max_cycles) {
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
}
