use std::env;

use crate::ssem::store::Store;

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

    let ci: i32 = 0;
    let a: i32 = 0;
    let store: Store;
    if has_filename {
        store = Store::from_asm_file(&filename);
    }
    else
    {
        store = Store::new();
    }

    println!(" {:032b} CI = {}", ci.reverse_bits(), ci);
    println!(" {:032b} A = {}", a.reverse_bits(), a);
    println!("");
    println!("{}", store.to_string());
}
