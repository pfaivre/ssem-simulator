use std::{fmt, path::Path};

use super::{opcode::Opcode, store::Store};

pub struct Simulator {
    /// Accumulator, the only register of the machine
    a: i32,

    /// Program counter. this points to the address currently being executed
    ci: i32,

    /// The main memory. This is an array of 32-bit words
    store: Store,

    /// This is set when the STP instruction is executed
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

    /// Initializes an SSEM simulator with memory state described in the given file.
    /// Supported file types are .snp and .asm
    pub fn from_file(filename: &Path) -> Simulator {
        let store = match filename.extension() {
            Some(ext) => {
                if ext == "asm" {
                    Store::from_asm_file(filename)
                } else if ext == "snp" {
                    Store::from_snp_file(filename)
                } else {
                    panic!("Unkown file format '{}'", ext.to_str().unwrap_or(""));
                }
            }
            None => panic!("Unkown file format"),
        };

        Simulator {
            a: 0,
            ci: 0,
            store: store,
            stop_flag: false,
        }
    }

    /// Run the machine until STP is encountered or the given amount of cycles is reached.
    ///
    /// Returns the number of cycles executed.
    pub fn run(&mut self, max_cycles: u32) -> u32 {
        let mut cycles = 0u32;
        self.stop_flag = false;

        while cycles < max_cycles && !self.stop_flag {
            self.instruction_cycle();
            cycles += 1;
        }

        cycles
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
                eprintln!(
                    "Error while decoding instruction at address {}: {}",
                    self.ci, message
                );
                self.stop_flag = true;
                return;
            }
        };

        // Execute
        self._execute(opcode, data);
    }

    /// Modify the state of the machine according to the given instruction.
    fn _execute(&mut self, command: Opcode, data: i32) {
        match command {
            Opcode::JMP => {
                self.ci = self.store[data];
            }
            Opcode::JRP => {
                self.ci += self.store[data];
            }
            Opcode::LDN => {
                // Was originally `self.a = -self.store[data];`
                // But in some cases we want to ignore overflowing. This has no measureable performance impact.
                // TODO: assert this is compliant with SSEM behavior
                self.a = self.store[data].wrapping_neg();
            }
            Opcode::STO => {
                // this indexing is safe as long as the data extracted earlier (a u5 for SSEM)
                // is smaller than the number of addresses on the store (32 for SSEM)
                self.store.words[data as usize] = self.a;
            }
            Opcode::SUB | Opcode::SUB2 => {
                // Was originally `self.a -= self.store[data];`
                // But in some cases we want to ignore overflowing. This has no measureable performance impact.
                // TODO: assert this is compliant with SSEM behavior
                self.a = self.a.wrapping_add(self.store[data].wrapping_neg());
            }
            Opcode::CMP => {
                if self.a < 0 {
                    self.ci += 1;
                }
            }
            Opcode::STP => {
                self.stop_flag = true;
            }
            Opcode::NUM => {
                panic!("Encountered an unexpected NUM command")
            }
        }

        // println!("{} {} {}", self.ci, command, data);
        // println!("{}", &self);
    }
}

impl fmt::Display for Simulator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // We reverse the bits for display because the SSEM stored numbers the opposite order than modern computers.
        // All the computing is done on the modern order for efficiency reasons.
        writeln!(f, " {:032b} CI = {:6}", self.ci.reverse_bits(), self.ci).ok();
        writeln!(f, " {:032b} A  = {:6}", self.a.reverse_bits(), self.a).ok();
        writeln!(f, "").ok();
        writeln!(f, "{}", self.store).ok();
        Ok(())
    }
}
