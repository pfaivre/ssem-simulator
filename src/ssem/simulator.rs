
use std::fmt;

use super::{store::Store, opcode::Opcode};

pub struct Simulator {
    a: i32,
    ci: i32,
    store: Store,
    stop_flag: bool,
}

impl Simulator {
    pub fn new() -> Simulator {
        Simulator {
            a: 0,
            ci: 0,
            store: Store::new(),
            stop_flag: false,
        }
    }

    pub fn from_file(filename: &str) -> Simulator {
        Simulator {
            a: 0,
            ci: 0,
            store: Store::from_asm_file(filename),
            stop_flag: false,
        }
    }

    /// Run the machine until STP is encountered or the given amount of cycles is reached.
    pub fn run(&mut self, max_cycles: i32) {
        // TODO: loop
        self.instruction_cycle();
    }

    /// Run the next instruction.
    pub fn instruction_cycle(&mut self) {
        // Fetch
        self.ci += 1;
        self.ci %= self.store.size; // CI loops back to the begining when it exceeds the store boundaries

        // Decode
        let (opcode, data) = match self.store.decode_instruction(self.ci) {
            Ok(r) => r,
            Err(message) => {
                eprintln!("Error while decoding instruction at address {}: {}", self.ci, message);
                self.stop_flag = true;
                return;
            }
        };

        // Execute
    }

    /// Modify the state of the machine according to the given instruction.
    fn _execute(&mut self, command: Opcode, data: i32) {
        todo!()
    }
}

impl fmt::Display for Simulator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, " {:032b} CI = {:6}", self.ci.reverse_bits(), self.ci).ok();
        writeln!(f, " {:032b} A  = {:6}", self.a.reverse_bits(), self.a).ok();
        writeln!(f, "").ok();
        writeln!(f, "{}", self.store).ok();
        Ok(())
    }
}
