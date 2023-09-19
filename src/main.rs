use std::path::{Path, PathBuf};

use clap::{command, Parser};

use crate::ssem::simulator::Simulator;

pub mod ssem;

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
    println!();
    println!("//// ssem-simulator ////");
    println!("");
    println!("This is a very early build.");
    println!("Visit the following repository for a fully functional simulator:");
    println!("    https://github.com/pfaivre/manchester-baby-sim");
    println!();

    let args = Args::parse();

    let mut simulator = Simulator::from_file(&Path::new(&args.file));
    use std::time::Instant;
    let start_time = Instant::now();

    let cycles = simulator.run(args.max_cycles);

    println!("Run completed!");
    println!("The final state of the machine is:");
    println!("{simulator}");
    println!(
        "{} cycles executed in {:.2?} ({:.0?} cps)",
        cycles,
        start_time.elapsed(),
        f64::from(cycles) / start_time.elapsed().as_secs_f64(),
    );
}
