use std::{
    fmt,
    path::{Path, PathBuf},
};

use super::{
    history::{History, MachineState},
    opcode::Opcode,
    store::{Bits, FileFormat, Store},
};

/// The default execution speed in instructions per second,
/// matching the typical speed of the original SSEM hardware.
pub const TYPICAL_SPEED: u32 = 700;

/// The last instruction executed by the machine, kept for display.
#[derive(Debug, Clone, Copy, PartialEq)]
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
                ts!("breakpoint.word", address = address : {:02}, value = value)
            }
            Breakpoint::CiValue { address, .. } => {
                ts!("breakpoint.ci", address = address : {:02})
            }
        }
    }
}

pub struct Simulator {
    /// Accumulator, the only register of the machine
    pub a: i32,

    /// Program counter. Incremented before each fetch, so the next
    /// instruction executes at `ci + 1` (wrapping at the store end);
    /// JMP, JRP and a taken CMP overwrite it with the jump target.
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

    /// How many times each store word has been fetched and executed as an
    /// instruction. Indexed by store address (0..32). Used by the TUI heatmap.
    pub exec_counts: [u64; 32],

    /// How many times each store word has been read as an operand
    /// (JMP, JRP, LDN, SUB, SUB2). Indexed by store address (0..32).
    /// Used by the TUI read/write heatmap.
    pub read_counts: [u64; 32],

    /// How many times each store word has been written by STO.
    /// Indexed by store address (0..32). Used by the TUI read/write heatmap.
    pub write_counts: [u64; 32],

    /// Execution history for time-travel, when enabled via `enable_history`.
    pub history: Option<History>,

    /// Latched by `rewind_to` when the machine is rewound behind the
    /// history head: the next executed cycle restarts the machine from a
    /// previous state, resets the heatmap counters once, and clears this
    /// flag. Scrubbing alone does not reset them.
    pub heatmap_reset_pending: bool,

    /// The file the store was loaded from, when built with
    /// `Simulator::from_file`. The save pop-up opens on its folder and
    /// proposes a default name derived from it.
    pub source_path: Option<PathBuf>,
}

impl Default for Simulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Simulator {
    pub fn new() -> Simulator {
        Self::with_store(Store::new(), None)
    }

    /// A simulator around `store` with every other field at its default — the
    /// single place the full field list lives, so `new`, `from_file`, and
    /// `From<Vec<String>>` delegate here and a new field is added in one spot.
    fn with_store(store: Store, source_path: Option<PathBuf>) -> Self {
        Simulator {
            a: 0,
            ci: 0,
            store,
            stop_flag: false,
            cycle_count: 0,
            speed: TYPICAL_SPEED,
            last_instruction: None,
            breakpoints: Vec::new(),
            exec_counts: [0; 32],
            read_counts: [0; 32],
            write_counts: [0; 32],
            history: None,
            heatmap_reset_pending: false,
            source_path,
        }
    }

    /// Capture the current machine state for the history ring.
    fn machine_state(&self) -> MachineState {
        MachineState {
            a: self.a,
            ci: self.ci,
            store: self.store.words,
            last_instruction: self.last_instruction,
        }
    }

    /// Initializes an SSEM simulator with memory state described in the given file.
    /// Supported file types are .snp and .asm, chosen by the file extension via
    /// [`FileFormat::from_path`] + [`FileFormat::load`] (the same dispatcher
    /// the save path uses).
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` with a localized message when the extension is
    /// missing/unknown, the file cannot be read, or a line cannot be parsed —
    /// the same error type as the save path, instead of panicking.
    pub fn from_file(filename: &Path) -> std::io::Result<Simulator> {
        Ok(Self::with_store(
            Self::load_store(filename)?,
            Some(filename.to_path_buf()),
        ))
    }

    /// Load a program from `filename` into this simulator, replacing the
    /// current machine state: the store comes from the file, and the
    /// registers, cycle count, heatmap counters and breakpoints are reset
    /// (as by [`Simulator::clear`]). The history, when enabled, restarts at
    /// the loaded state. `source_path` is set, so a later save proposes the
    /// loaded file's folder.
    ///
    /// # Errors
    ///
    /// Same as [`Simulator::from_file`]; on error the machine is left
    /// untouched.
    pub fn load_file(&mut self, filename: &Path) -> std::io::Result<()> {
        let store = Self::load_store(filename)?;
        self.clear();
        self.store = store;
        self.source_path = Some(filename.to_path_buf());
        self.restart_history();
        Ok(())
    }

    /// Whether the machine is currently running (not stopped).
    pub fn is_running(&self) -> bool {
        !self.stop_flag
    }

    /// Load a store from `filename`, in the format given by the file
    /// extension (`.asm`/`.snp`) — the single load dispatch shared by
    /// `from_file` and `load_file`.
    fn load_store(filename: &Path) -> std::io::Result<Store> {
        FileFormat::from_path(filename)?.load(filename)
    }

    /// Write the current machine state to `path`, in the format given by the
    /// file extension (`.asm` assembly or `.snp` snapshot, case-insensitive).
    ///
    /// The store words are written with their source comments, if any. The
    /// registers cannot live in either format, so CI, the accumulator and
    /// the cycle count are recorded in a leading comment line — both loaders
    /// skip comment-only lines, so the file reloads as the same store.
    ///
    /// Returns an `InvalidInput` error when the extension is missing or
    /// unknown, and the underlying I/O error when the write fails.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let body = FileFormat::from_path(path)?.dump(&self.store);
        let header = format!(
            "; Machine state saved by ssem-simulator v{}\n; CI: {}  A: {}  cycles: {}\n",
            env!("CARGO_PKG_VERSION"),
            self.ci,
            self.a,
            self.cycle_count
        );
        std::fs::write(path, format!("{header}{body}"))
    }

    /// Enable execution history, retaining up to `capacity` past states.
    ///
    /// The current state is recorded as cycle 0; every subsequent instruction
    /// cycle appends a machine state. History is disabled (None) by default, so
    /// headless runs pay no memory or CPU cost.
    pub fn enable_history(&mut self, capacity: usize) {
        let mut history = History::new(capacity);
        history.record(self.cycle_count, self.machine_state());
        self.history = Some(history);
        // A fresh history starts at the tip, so nothing is pending
        self.heatmap_reset_pending = false;
    }

    /// Restore the machine to the state recorded at `cycle`.
    ///
    /// Restores the last-instruction display from the recorded machine state and pauses
    /// the machine. The heatmap counters (execution and read/write) are
    /// kept, so scrubbing the history does not wipe the heatmap; they are
    /// reset only when the machine restarts from the rewound position (see
    /// `instruction_cycle`). The retained history is preserved, so the
    /// position can be scrubbed forward again; re-executing from the
    /// rewound position overwrites the previously recorded future.
    /// Returns false when history is disabled or `cycle` is not retained.
    pub fn rewind_to(&mut self, cycle: u64) -> bool {
        let Some(history) = &self.history else {
            return false;
        };
        let Some(machine_state) = history.get(cycle).copied() else {
            return false;
        };
        // Latch whether this position is behind the head, so the next
        // instruction_cycle knows it restarts from a previous state
        // without re-deriving it on every cycle. Scrubbing back to the
        // tip clears the latch: resuming from there is a continuation.
        let pending = cycle < history.head();
        self.a = machine_state.a;
        self.ci = machine_state.ci;
        self.store.words = machine_state.store;
        self.cycle_count = cycle;
        self.last_instruction = machine_state.last_instruction;
        self.heatmap_reset_pending = pending;
        self.stop_flag = true;
        true
    }

    /// Drop the recorded history and restart it at the current state.
    ///
    /// Called when the user edits the machine state (a register or a store
    /// word): the recorded future no longer matches what the machine will
    /// execute, so it is discarded and the earliest browsable state becomes
    /// the one where the change was made. Does nothing when history is
    /// disabled.
    pub fn restart_history(&mut self) {
        let machine_state = self.machine_state();
        if let Some(history) = &mut self.history {
            history.clear();
            history.record(self.cycle_count, machine_state);
        }
    }

    /// Zero the heatmap counters (execution and read/write).
    fn reset_heatmap(&mut self) {
        self.exec_counts = [0; 32];
        self.read_counts = [0; 32];
        self.write_counts = [0; 32];
    }

    /// Reset the machine to the state it has when opened without a
    /// program
    pub fn clear(&mut self) {
        self.stop_flag = true;
        self.ci = 0;
        self.a = 0;
        self.cycle_count = 0;
        self.store.clear();
        self.last_instruction = None;
        self.breakpoints.clear();
        self.reset_heatmap();
        self.heatmap_reset_pending = false;
        let machine_state = self.machine_state();
        if let Some(history) = &mut self.history {
            history.clear();
            history.record(self.cycle_count, machine_state);
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
            self.check_breakpoints();
            cycles += 1;
        }

        cycles
    }

    /// Run the next instruction.
    pub fn instruction_cycle(&mut self) {
        // rewind_to latched that this cycle restarts the machine from a
        // previous state (the recorded future is about to be dropped):
        // the heatmap counters of that discarded future must not linger,
        // so reset them once, here. Scrubbing alone keeps them.
        if self.heatmap_reset_pending {
            self.reset_heatmap();
            self.heatmap_reset_pending = false;
        }

        // Fetch
        self.ci = self.ci.wrapping_add(1);
        self.ci &= Store::INDEX_MASK; // CI loops back to the begining when it exceeds the store boundaries

        // Decode
        let (opcode, data) = self.store.decode_instruction(self.ci as usize);

        // Capture the address before execution: JMP, JRP, and CMP modify CI,
        // so recording after execute would attribute the cycle to the wrong line.
        let address = self.ci;
        self.exec_counts[address as usize] += 1;

        // Execute
        self.execute(opcode, data);

        // Track for display
        self.last_instruction = Some(LastInstruction {
            address,
            opcode,
            data,
        });
        self.cycle_count += 1;

        // Record for time-travel
        let machine_state = self.machine_state();
        if let Some(history) = &mut self.history {
            history.record(self.cycle_count, machine_state);
        }
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
    pub(crate) fn execute(&mut self, command: Opcode, data: i32) {
        // Track store accesses for the read/write heatmap: every instruction
        // with an address operand reads it, except STO which writes it.
        // `data` comes from `decode_word`, so it is already masked to 0..=31
        // and indexing is safe, exactly like the STO arm below.
        if command.uses_address() {
            if command == Opcode::STO {
                self.write_counts[data as usize] += 1;
            } else {
                self.read_counts[data as usize] += 1;
            }
        }

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
        Self::with_store(Store::from(value), None)
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
