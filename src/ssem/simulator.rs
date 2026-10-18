use std::{fmt, path::Path};

use super::{opcode::Opcode, store::Store};

pub struct Simulator {
    /// Accumulator, the only register of the machine
    pub a: i32,

    /// Program counter. this points to the address currently being executed
    pub ci: i32,

    /// The main memory. This is an array of 32-bit words
    pub store: Store,

    /// This is set when the STP instruction is executed
    stop_flag: bool,
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
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
            store,
            stop_flag: false,
        }
    }

    /// Run the machine until STP is encountered or the given amount of cycles is reached.
    ///
    /// Returns the number of cycles executed.
    pub fn run(&mut self, max_cycles: u32) -> u32 {
        let mut cycles = 0u32;
        self.stop_flag = false;

        // let ten_millis = std::time::Duration::from_millis(10);

        while cycles < max_cycles && !self.stop_flag {
            self.instruction_cycle();
            cycles += 1;
            // std::thread::sleep(ten_millis);
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

impl From<Vec<String>> for Simulator {
    fn from(value: Vec<String>) -> Self {
        Simulator {
            a: 0,
            ci: 0,
            store: Store::from(value),
            stop_flag: false,
        }
    }
}

impl fmt::Display for Simulator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // We reverse the bits for display because the SSEM stored numbers the opposite order than modern computers.
        // All the computing is done on the modern order for efficiency reasons.
        writeln!(f, " {:032b} CI = {:6}", self.ci.reverse_bits(), self.ci).ok();
        writeln!(f, " {:032b} A  = {:6}", self.a.reverse_bits(), self.a).ok();
        writeln!(f).ok();
        writeln!(f, "{}", self.store).ok();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Opcode, Simulator};

    /// Encode an instruction word the same way `Store::from_asm_file` does:
    /// opcode bits at position 13, operand in the low 5 bits.
    fn instr(opcode: Opcode, data: i32) -> i32 {
        (opcode as i32) << 13 | data
    }

    #[test]
    fn new_and_default() {
        for sim in [Simulator::new(), Simulator::default()] {
            assert_eq!(0, sim.a);
            assert_eq!(0, sim.ci);
            assert_eq!(32, sim.store.size);
            assert!(sim.store.words.iter().all(|&word| word == 0));
        }
    }

    #[test]
    fn from_vec_strings() {
        let mut words: Vec<String> = vec!["0".repeat(32); 32];
        words[1] = format!("{:032b}", 1_i32.reverse_bits());
        let sim = Simulator::from(words);
        assert_eq!(0, sim.a);
        assert_eq!(0, sim.ci);
        assert_eq!(0, sim.store.words[0]);
        assert_eq!(1, sim.store.words[1]);
    }

    #[test]
    fn from_file_asm() {
        let sim = Simulator::from_file(Path::new("samples/ssem/factorct.asm"));
        assert_eq!(0, sim.a);
        assert_eq!(0, sim.ci);
        // 01 LDN 24
        assert_eq!(instr(Opcode::LDN, 24), sim.store.words[1]);
    }

    #[test]
    fn from_file_snp() {
        let sim = Simulator::from_file(Path::new("samples/ssem/nightmare.snp"));
        assert_eq!(0, sim.a);
        assert_eq!(0, sim.ci);
        assert_eq!(
            "00000110101001000100000100000100",
            format!("{:032b}", sim.store.words[0].reverse_bits())
        );
    }

    #[test]
    #[should_panic(expected = "Unkown file format 'txt'")]
    fn from_file_unknown_extension() {
        let _ = Simulator::from_file(Path::new("samples/ssem/virpet.txt"));
    }

    #[test]
    #[should_panic(expected = "Unkown file format")]
    fn from_file_no_extension() {
        let _ = Simulator::from_file(Path::new("samples/ssem"));
    }

    #[test]
    fn run_stops_on_stp() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::STP, 0);
        assert_eq!(1, sim.run(10));
    }

    #[test]
    fn run_stops_at_max_cycles() {
        // An empty store endlessly executes `JMP 0`
        let mut sim = Simulator::new();
        assert_eq!(0, sim.run(0));
        assert_eq!(7, sim.run(7));
        assert_eq!(0, sim.ci);
    }

    #[test]
    fn run_can_be_restarted_after_stp() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::STP, 0);
        sim.store.words[2] = instr(Opcode::STP, 0);
        assert_eq!(1, sim.run(10));
        assert_eq!(1, sim.ci);
        // A second run resumes from where the machine stopped
        assert_eq!(1, sim.run(10));
        assert_eq!(2, sim.ci);
    }

    #[test]
    fn run_executes_simple_program() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::LDN, 6); // a = -store[6]
        sim.store.words[2] = instr(Opcode::SUB, 7); // a -= store[7]
        sim.store.words[3] = instr(Opcode::STO, 8); // store[8] = a
        sim.store.words[4] = instr(Opcode::STP, 0);
        sim.store.words[6] = 5;
        sim.store.words[7] = 3;
        assert_eq!(4, sim.run(100));
        assert_eq!(-8, sim.a);
        assert_eq!(-8, sim.store.words[8]);
    }

    #[test]
    fn instruction_cycle_increments_ci() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::STP, 0);
        sim.instruction_cycle();
        assert_eq!(1, sim.ci);
    }

    #[test]
    fn instruction_cycle_wraps_ci_around_store() {
        let mut sim = Simulator::new();
        sim.ci = 31;
        sim.store.words[0] = instr(Opcode::STP, 0);
        sim.instruction_cycle();
        assert_eq!(0, sim.ci);
    }

    #[test]
    fn jmp_sets_ci_from_store() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::JMP, 2);
        sim.store.words[2] = 20;
        sim.instruction_cycle();
        assert_eq!(20, sim.ci);
    }

    #[test]
    fn jrp_adds_store_value_to_ci() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::JRP, 2);
        sim.store.words[2] = 5;
        sim.instruction_cycle(); // CI goes 0 -> 1, then 5 is added
        assert_eq!(6, sim.ci);
    }

    #[test]
    fn ldn_negates_store_value() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::LDN, 2);
        sim.store.words[2] = 7;
        sim.instruction_cycle();
        assert_eq!(-7, sim.a);
    }

    #[test]
    fn ldn_wraps_on_i32_min() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::LDN, 2);
        sim.store.words[2] = i32::MIN;
        sim.instruction_cycle();
        assert_eq!(i32::MIN, sim.a);
    }

    #[test]
    fn sto_writes_accumulator_to_store() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::STO, 5);
        sim.a = 42;
        sim.instruction_cycle();
        assert_eq!(42, sim.store.words[5]);
    }

    #[test]
    fn sub_subtracts_store_value() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::SUB, 2);
        sim.store.words[2] = 3;
        sim.a = 10;
        sim.instruction_cycle();
        assert_eq!(7, sim.a);
    }

    #[test]
    fn sub2_behaves_like_sub() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::SUB2, 2);
        sim.store.words[2] = 3;
        sim.a = 10;
        sim.instruction_cycle();
        assert_eq!(7, sim.a);
    }

    #[test]
    fn sub_wraps_on_overflow() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::SUB, 2);
        sim.store.words[2] = 1;
        sim.a = i32::MIN;
        sim.instruction_cycle();
        assert_eq!(i32::MAX, sim.a);
    }

    #[test]
    fn cmp_skips_next_instruction_on_negative_accumulator() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::CMP, 0);
        sim.a = -1;
        sim.instruction_cycle(); // CI goes 0 -> 1, then skipped to 2
        assert_eq!(2, sim.ci);
    }

    #[test]
    fn cmp_does_not_skip_on_non_negative_accumulator() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::CMP, 0);
        sim.store.words[2] = instr(Opcode::STP, 0);

        sim.a = 0;
        sim.instruction_cycle();
        assert_eq!(1, sim.ci);

        sim.a = 1;
        sim.instruction_cycle();
        assert_eq!(2, sim.ci);
    }

    #[test]
    fn stp_sets_stop_flag() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::STP, 0);
        sim.instruction_cycle();
        assert_eq!(1, sim.ci);
        assert!(sim.stop_flag);
    }

    #[test]
    #[should_panic(expected = "Encountered an unexpected NUM command")]
    fn num_panics_when_executed() {
        let mut sim = Simulator::new();
        sim._execute(Opcode::NUM, 0);
    }

    #[test]
    fn display_shows_reversed_bits_and_registers() {
        let mut sim = Simulator::new();
        sim.ci = 1;
        sim.a = -1;
        let display = sim.to_string();
        let mut lines = display.lines();

        assert_eq!(
            Some(" 10000000000000000000000000000000 CI =      1"),
            lines.next()
        );
        assert_eq!(
            Some(" 11111111111111111111111111111111 A  =     -1"),
            lines.next()
        );
        assert_eq!(Some(""), lines.next());
        // The 32 store words, in reversed bit order
        for _ in 0..32 {
            assert_eq!(Some(" 00000000000000000000000000000000"), lines.next());
        }
        assert_eq!(Some(""), lines.next());
        assert_eq!(None, lines.next());
    }
}
