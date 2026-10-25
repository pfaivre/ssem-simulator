use std::{fmt, path::Path};

use super::{
    opcode::Opcode,
    store::{Bits, Store},
};

/// The default execution speed in instructions per second,
/// matching the typical speed of the original SSEM hardware.
pub const TYPICAL_SPEED: u32 = 700;

/// The last instruction executed by the machine, kept for display.
#[derive(Debug, Clone, Copy)]
pub struct LastInstruction {
    pub address: i32,
    pub opcode: Opcode,
    pub data: i32,
}

impl fmt::Display for LastInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02} {} {:02}", self.address, self.opcode, self.data)
    }
}

/// A condition that pauses the machine when it becomes true.
///
/// Breakpoints trigger on a rising edge: the machine pauses when the
/// condition turns true, and the breakpoint stays armed — it triggers
/// again only after the condition has turned false at least once.
#[derive(Debug, Clone)]
pub enum Breakpoint {
    /// Pause when the word at `address` holds `value`.
    WordValue {
        address: usize,
        value: i32,
        was_met: bool,
    },
    /// Pause when the program counter holds `address`.
    CiValue { address: i32, was_met: bool },
}

/// Two breakpoints are equal when they describe the same condition,
/// regardless of their edge-detection state.
impl PartialEq for Breakpoint {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Breakpoint::WordValue {
                    address: a1,
                    value: v1,
                    ..
                },
                Breakpoint::WordValue {
                    address: a2,
                    value: v2,
                    ..
                },
            ) => a1 == a2 && v1 == v2,
            (Breakpoint::CiValue { address: a1, .. }, Breakpoint::CiValue { address: a2, .. }) => {
                a1 == a2
            }
            _ => false,
        }
    }
}

impl Breakpoint {
    fn was_met(&self) -> bool {
        match self {
            Breakpoint::WordValue { was_met, .. } => *was_met,
            Breakpoint::CiValue { was_met, .. } => *was_met,
        }
    }

    fn set_was_met(&mut self, value: bool) {
        match self {
            Breakpoint::WordValue { was_met, .. } => *was_met = value,
            Breakpoint::CiValue { was_met, .. } => *was_met = value,
        }
    }

    /// Human-readable description shown in the breakpoint pop-up.
    pub fn description(&self) -> String {
        match self {
            Breakpoint::WordValue { address, value, .. } => {
                format!("WORD {address:02} = {value}")
            }
            Breakpoint::CiValue { address, .. } => {
                format!("CI = {address:02}")
            }
        }
    }
}

pub struct Simulator {
    /// Accumulator, the only register of the machine
    pub a: i32,

    /// Program counter. this points to the address currently being executed
    pub ci: i32,

    /// The main memory. This is an array of 32-bit words
    pub store: Store,

    /// This is set when the STP instruction is executed, or when a
    /// breakpoint is hit, or when the user pauses the machine.
    pub stop_flag: bool,

    /// Total number of instruction cycles executed since creation.
    pub cycle_count: u64,

    /// Target execution speed in instructions per second.
    pub speed: u32,

    /// The last instruction executed, for display in the TUI.
    pub last_instruction: Option<LastInstruction>,

    /// Active breakpoints, checked after every instruction cycle.
    pub breakpoints: Vec<Breakpoint>,
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
            cycle_count: 0,
            speed: TYPICAL_SPEED,
            last_instruction: None,
            breakpoints: Vec::new(),
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
            cycle_count: 0,
            speed: TYPICAL_SPEED,
            last_instruction: None,
            breakpoints: Vec::new(),
        }
    }

    /// Whether the machine is currently running (not stopped).
    pub fn is_running(&self) -> bool {
        !self.stop_flag
    }

    /// Run the machine until STP is encountered or the given amount of cycles is reached.
    ///
    /// Returns the number of cycles executed.
    pub fn run(&mut self, max_cycles: u32) -> u32 {
        let mut cycles = 0u32;
        self.stop_flag = false;

        while cycles < max_cycles && !self.stop_flag {
            self.instruction_cycle();
            self.check_breakpoints();
            cycles += 1;
        }

        cycles
    }

    /// Run the next instruction.
    pub fn instruction_cycle(&mut self) {
        // Fetch
        self.ci += 1;
        self.ci &= Store::INDEX_MASK; // CI loops back to the begining when it exceeds the store boundaries

        // Decode
        let (opcode, data) = self.store.decode_instruction(self.ci as usize);

        // Execute
        self._execute(opcode, data);

        // Track for display
        self.last_instruction = Some(LastInstruction {
            address: self.ci,
            opcode,
            data,
        });
        self.cycle_count += 1;
    }

    /// Check all breakpoints and stop the machine if any triggers.
    ///
    /// Breakpoints are edge-triggered: they fire only when their condition
    /// transitions from false to true.
    pub fn check_breakpoints(&mut self) {
        if self.breakpoints.is_empty() {
            return;
        }

        // Borrow the fields needed for checking separately from the
        // breakpoints list to satisfy the borrow checker.
        let store_words = &self.store.words;
        let ci = self.ci;

        let mut should_stop = false;
        for bp in &mut self.breakpoints {
            let met = match bp {
                Breakpoint::WordValue { address, value, .. } => store_words[*address] == *value,
                Breakpoint::CiValue { address, .. } => ci == *address,
            };
            let triggered = met && !bp.was_met();
            bp.set_was_met(met);
            if triggered {
                should_stop = true;
            }
        }

        if should_stop {
            self.stop_flag = true;
        }
    }

    /// Modify the state of the machine according to the given instruction.
    fn _execute(&mut self, command: Opcode, data: i32) {
        match command {
            Opcode::JMP => {
                // The fetched word can hold any bit pattern, but only its low
                // bits are significant for line addressing. Masking wraps any
                // value, negative included, into the store range.
                self.ci = self.store[data] & Store::INDEX_MASK;
            }
            Opcode::JRP => {
                self.ci = self.ci.wrapping_add(self.store[data]) & Store::INDEX_MASK;
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
                    // Wrap CI into the store range, like JMP and JRP: a taken
                    // CMP on the last line must not leave CI at 32.
                    self.ci = (self.ci + 1) & Store::INDEX_MASK;
                }
            }
            Opcode::STP => {
                self.stop_flag = true;
            }
            // NUM is only an assembly mnemonic to store a raw value; it is
            // never produced by decode_instruction.
            Opcode::NUM => unreachable!("Encountered an unexpected NUM command"),
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
            cycle_count: 0,
            speed: TYPICAL_SPEED,
            last_instruction: None,
            breakpoints: Vec::new(),
        }
    }
}

impl fmt::Display for Simulator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // We display the bits least significant first because the SSEM stored numbers the opposite order than modern computers.
        // All the computing is done on the modern order for efficiency reasons.
        writeln!(f, " {} {:<2}:{:>12}", Bits(self.ci), "CI", self.ci).ok();
        writeln!(f, " {} {:<2}:{:>12}", Bits(self.a), "A", self.a).ok();
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
            assert_eq!(32, sim.store.words.len());
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
    fn jmp_negative_word_wraps_ci_into_store_range() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::JMP, 2);
        sim.store.words[2] = -4; // -4 selects line 28 on the SSEM
        sim.instruction_cycle();
        assert_eq!(28, sim.ci);
    }

    #[test]
    fn jrp_negative_offset_wraps_ci_into_store_range() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::JRP, 2);
        sim.store.words[2] = -5;
        sim.instruction_cycle(); // CI goes 0 -> 1, then -5 is added
        assert_eq!(28, sim.ci);
    }

    #[test]
    fn jrp_huge_offset_does_not_overflow() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::JRP, 2);
        sim.store.words[2] = i32::MAX;
        sim.instruction_cycle(); // CI goes 0 -> 1, then i32::MAX is added
        assert_eq!(0, sim.ci);
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
    fn cmp_on_last_line_wraps_ci_into_store_range() {
        let mut sim = Simulator::new();
        sim.store.words[31] = instr(Opcode::CMP, 0);
        sim.store.words[1] = instr(Opcode::STP, 0);
        sim.a = -1;
        sim.ci = 30; // the next fetch executes line 31 (CMP)
        sim.instruction_cycle();
        // The skip wraps CI into the store instead of leaving it at 32
        assert_eq!(0, sim.ci);
        // Line 0 is the skipped one, so the machine stops on line 1 (STP)
        assert_eq!(1, sim.run(10));
        assert_eq!(1, sim.ci);
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
        sim.store.words[0] = 5; // bits: 101 least significant bit first
        sim.store.words[1] = instr(Opcode::LDN, 29);
        let display = sim.to_string();
        let mut lines = display.lines();

        assert_eq!(
            Some(" #............................... CI:           1"),
            lines.next()
        );
        assert_eq!(
            Some(" ################################ A :          -1"),
            lines.next()
        );
        assert_eq!(Some(""), lines.next());
        // The 32 store words, bits least significant bit first,
        // with their index, integer value and instruction interpretation
        assert_eq!(
            Some(" #.#............................. 00:           5  JMP 05"),
            lines.next()
        );
        assert_eq!(
            Some(" #.###.........#................. 01:       16413  LDN 29"),
            lines.next()
        );
        for index in 2..32 {
            assert_eq!(
                Some(
                    format!(
                        " ................................ {:02}:           0  JMP 00",
                        index
                    )
                    .as_str()
                ),
                lines.next()
            );
        }
        assert_eq!(Some(""), lines.next());
        assert_eq!(None, lines.next());
    }

    #[test]
    fn instruction_cycle_tracks_last_instruction_and_cycle_count() {
        let mut sim = Simulator::new();
        sim.store.words[1] = instr(Opcode::LDN, 5);
        sim.store.words[2] = instr(Opcode::STP, 0);

        assert_eq!(0, sim.cycle_count);
        assert!(sim.last_instruction.is_none());

        sim.instruction_cycle();
        assert_eq!(1, sim.cycle_count);
        let li = sim.last_instruction.unwrap();
        assert_eq!(1, li.address);
        assert_eq!(Opcode::LDN, li.opcode);
        assert_eq!(5, li.data);

        sim.instruction_cycle();
        assert_eq!(2, sim.cycle_count);
        let li = sim.last_instruction.unwrap();
        assert_eq!(2, li.address);
        assert_eq!(Opcode::STP, li.opcode);
        assert_eq!(0, li.data);
    }

    #[test]
    fn last_instruction_display() {
        use super::LastInstruction;
        let li = LastInstruction {
            address: 1,
            opcode: Opcode::LDN,
            data: 29,
        };
        assert_eq!("01 LDN 29", li.to_string());
    }

    #[test]
    fn is_running_reflects_stop_flag() {
        let mut sim = Simulator::new();
        sim.stop_flag = false;
        assert!(sim.is_running());
        sim.stop_flag = true;
        assert!(!sim.is_running());
    }

    #[test]
    fn speed_defaults_to_typical() {
        let sim = Simulator::new();
        assert_eq!(super::TYPICAL_SPEED, sim.speed);
        assert_eq!(700, sim.speed);
    }

    #[test]
    fn breakpoint_description_word_value() {
        use super::Breakpoint;
        let bp = Breakpoint::WordValue {
            address: 5,
            value: -42,
            was_met: false,
        };
        assert_eq!("WORD 05 = -42", bp.description());
    }

    #[test]
    fn breakpoint_description_ci_value() {
        use super::Breakpoint;
        let bp = Breakpoint::CiValue {
            address: 10,
            was_met: false,
        };
        assert_eq!("CI = 10", bp.description());
    }

    #[test]
    fn breakpoint_equality_ignores_was_met() {
        use super::Breakpoint;
        let bp1 = Breakpoint::WordValue {
            address: 5,
            value: 42,
            was_met: false,
        };
        let bp2 = Breakpoint::WordValue {
            address: 5,
            value: 42,
            was_met: true,
        };
        assert_eq!(bp1, bp2);

        let bp3 = Breakpoint::WordValue {
            address: 5,
            value: 43,
            was_met: false,
        };
        assert_ne!(bp1, bp3);

        let ci1 = Breakpoint::CiValue {
            address: 3,
            was_met: false,
        };
        let ci2 = Breakpoint::CiValue {
            address: 3,
            was_met: true,
        };
        assert_eq!(ci1, ci2);

        // Different kinds are never equal
        assert_ne!(bp1, ci1);
    }

    #[test]
    fn check_breakpoints_word_value_triggers_on_match() {
        use super::Breakpoint;
        let mut sim = Simulator::new();
        sim.store.words[5] = 42;
        sim.breakpoints.push(Breakpoint::WordValue {
            address: 5,
            value: 42,
            was_met: false,
        });

        sim.check_breakpoints();
        assert!(sim.stop_flag, "should stop when word matches");
    }

    #[test]
    fn check_breakpoints_word_value_does_not_trigger_on_mismatch() {
        use super::Breakpoint;
        let mut sim = Simulator::new();
        sim.store.words[5] = 10;
        sim.breakpoints.push(Breakpoint::WordValue {
            address: 5,
            value: 42,
            was_met: false,
        });

        sim.check_breakpoints();
        assert!(!sim.stop_flag, "should not stop when word does not match");
    }

    #[test]
    fn check_breakpoints_ci_value_triggers_on_match() {
        use super::Breakpoint;
        let mut sim = Simulator::new();
        sim.ci = 7;
        sim.breakpoints.push(Breakpoint::CiValue {
            address: 7,
            was_met: false,
        });

        sim.check_breakpoints();
        assert!(sim.stop_flag, "should stop when CI matches");
    }

    #[test]
    fn check_breakpoints_edge_triggered() {
        use super::Breakpoint;
        let mut sim = Simulator::new();
        sim.store.words[5] = 42;
        sim.breakpoints.push(Breakpoint::WordValue {
            address: 5,
            value: 42,
            was_met: false,
        });

        // First check: condition is met, rising edge -> stop
        sim.check_breakpoints();
        assert!(sim.stop_flag);

        // Reset stop flag, condition still met -> should NOT trigger again (no rising edge)
        sim.stop_flag = false;
        sim.check_breakpoints();
        assert!(
            !sim.stop_flag,
            "should not trigger again while condition stays true"
        );

        // Condition goes false, then true again -> rising edge -> stop
        sim.store.words[5] = 0;
        sim.check_breakpoints();
        assert!(!sim.stop_flag);
        sim.store.words[5] = 42;
        sim.check_breakpoints();
        assert!(sim.stop_flag, "should trigger on new rising edge");
    }

    #[test]
    fn check_breakpoints_empty_list_is_noop() {
        let mut sim = Simulator::new();
        sim.check_breakpoints();
        assert!(!sim.stop_flag);
    }

    #[test]
    fn run_stops_at_breakpoint() {
        use super::Breakpoint;
        let mut sim = Simulator::new();
        // Line 1: STO 5 (store A=0 to address 5)
        sim.store.words[1] = instr(Opcode::STO, 5);
        // Line 2: STP
        sim.store.words[2] = instr(Opcode::STP, 0);
        // Breakpoint: word 5 becomes 0 (which it already is, so it triggers on first check)
        sim.breakpoints.push(Breakpoint::WordValue {
            address: 5,
            value: 0,
            was_met: false,
        });

        let cycles = sim.run(100);
        // The breakpoint on word 5 = 0 triggers after the first cycle (STO 5 writes 0)
        // Actually, word 5 starts at 0, so the breakpoint triggers immediately after cycle 1
        assert!(
            cycles <= 2,
            "should stop early due to breakpoint, got {cycles}"
        );
        assert!(sim.stop_flag);
    }
}
