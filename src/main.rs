use std::env;

use crate::ssem::simulator::Simulator;

pub mod ssem;

fn main() {
    println!();
    println!("//// ssem-simulator ////");
    println!("");
    println!("This is a very early build and does not work yet.");
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
        simulator = Simulator::from_file(&filename);
    }
    else {
        simulator = Simulator::new();
    }

    simulator.run(1_000_000);

    println!("{simulator}");
}
