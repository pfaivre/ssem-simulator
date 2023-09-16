use std::{env, path::Path};

use crate::ssem::simulator::Simulator;

pub mod ssem;

fn main() {
    println!();
    println!("//// ssem-simulator ////");
    println!("");
    println!("This is a very early build.");
    println!("Visit the following repository for a fully functional simulator:");
    println!("    https://github.com/pfaivre/manchester-baby-sim");
    println!();

    let args: Vec<String> = env::args().collect();
    let mut filename = "".into();
    let mut has_filename = false;
    if args.len() > 1 {
        filename = args[1].clone();
        has_filename = true;
    }

    let mut simulator: Simulator;
    if has_filename {
        simulator = Simulator::from_file(&Path::new(&filename));
        use std::time::Instant;
        let start_time = Instant::now();

        let cycles = simulator.run(100_000_000);

        println!("Run completed!");
        println!("The final state of the machine is:");
        println!("{simulator}");
        println!("{} cycles executed in {:.2?}", cycles, start_time.elapsed());
    } else {
        print_help();
    }
}

fn print_help() {
    println!("Usage:");
    println!("  ssem-simulator [file]");
}
