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

    /// Input file to initialize the store. Can be .asm or .snp format
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

fn main() {
    let args = Args::parse();

    let simulator = Simulator::from_file(Path::new(&args.file));

    // Launch the interactive TUI; it returns the simulator when the user quits
    let simulator = match tui::run(simulator, args.max_cycles) {
        Ok(sim) => sim,
        Err(e) => {
            eprintln!("Error: {e}");
            eprintln!("The interactive interface requires a terminal. Run this program in a TTY.");
            std::process::exit(1);
        }
    };

    // Print the final state after quitting the TUI
    println!("Final state of the machine:");
    println!("{simulator}");
}
