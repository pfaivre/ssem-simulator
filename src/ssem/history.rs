//! Execution history for time-travel debugging.
//!
//! The machine state is tiny (accumulator, program counter, and 32 store
//! words), so the history keeps a full machine state per cycle in a ring buffer.
//! When the buffer is full, the oldest states are overwritten.

use super::simulator::LastInstruction;

/// Upper bound on history capacity. Each `MachineState` is 148 bytes
/// (two `i32`, `[i32; 32]`, and the last instruction), so this cap keeps
/// the ring buffer under ~1.5 GB — about 4 hours of history at the
/// typical 700 instructions per second. The buffer is allocated lazily
/// (see `History::record`), so a large requested capacity costs nothing
/// until the machine actually runs that many cycles.
pub const MAX_CAPACITY: usize = 10_000_000;

/// The machine's architectural state at one cycle.
///
/// The last executed instruction is stored alongside the core state, so
/// the display can mark the executed line even after a jump (when CI no
/// longer points at it). The heatmap counters (execution and read/write)
/// are not part of the machine state: they are kept while the history is
/// browsed, and reset only when the machine restarts from a rewound
/// position.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MachineState {
    pub a: i32,
    pub ci: i32,
    pub store: [i32; 32],
    pub last_instruction: Option<LastInstruction>,
}

/// A ring buffer of machine states, indexed by absolute cycle number.
///
/// Entries are contiguous: the retained entries are the absolute cycles
/// `[start, head]`. `record` appends at the tip; when the buffer is full the
/// oldest entry is overwritten. Recording at a cycle at or before the tip
/// (re-executing from a rewound position) drops the stale future, so the
/// history always describes exactly what the machine has executed.
///
/// The backing vector grows lazily as cycles are recorded, up to `capacity`
/// entries, so memory follows the cycles actually executed rather than the
/// requested capacity.
pub struct History {
    states: Vec<MachineState>,
    capacity: u64,
    /// Absolute cycle of the newest recorded state.
    head: u64,
    /// Number of retained entries.
    count: u64,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "history capacity must be at least 1");
        History {
            states: Vec::new(),
            capacity: capacity as u64,
            head: 0,
            count: 0,
        }
    }

    /// Number of state slots allocated so far. Test-only: the lazy
    /// allocation must stay proportional to the recorded cycles.
    #[cfg(test)]
    pub(crate) fn allocated(&self) -> usize {
        self.states.len()
    }

    pub fn capacity(&self) -> u64 {
        self.capacity
    }

    pub fn len(&self) -> u64 {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Absolute cycle of the newest recorded state.
    pub fn head(&self) -> u64 {
        self.head
    }

    /// Absolute cycle of the oldest retained state.
    pub fn start(&self) -> u64 {
        self.head + 1 - self.count
    }

    /// The machine state recorded at `cycle`, if it is still retained.
    pub fn get(&self, cycle: u64) -> Option<&MachineState> {
        if self.count == 0 || cycle < self.start() || cycle > self.head {
            return None;
        }
        self.states.get((cycle % self.capacity) as usize)
    }

    /// Record the state at `cycle`: appends at the tip, or drops the stale
    /// future when recording at or before the tip (re-executing from a
    /// rewound position).
    pub fn record(&mut self, cycle: u64, machine_state: MachineState) {
        if self.count > 0 {
            debug_assert!(
                cycle >= self.start() && cycle <= self.head + 1,
                "record at {cycle} outside [{}, {}]",
                self.start(),
                self.head + 1
            );
        }
        let slot = (cycle % self.capacity) as usize;
        if slot >= self.states.len() {
            // Grow the backing vector on demand, geometrically but capped
            // at the ring capacity so the allocation never exceeds it (a
            // bare push would let the Vec overshoot up to 2x). Growth
            // happens only at these ~log2(capacity) boundaries, so it
            // amortizes to O(1) per cycle.
            let new_len = (self.states.len() * 2)
                .max(slot + 1)
                .min(self.capacity as usize);
            self.states.resize(new_len, MachineState::default());
        }
        self.states[slot] = machine_state;
        if self.count == 0 || cycle > self.head {
            self.count = (self.count + 1).min(self.capacity);
        } else {
            self.count = cycle - self.start() + 1;
        }
        self.head = cycle;
    }

    /// Drop every retained state, keeping the allocated buffer, so
    /// the next `record` starts a fresh history at the cycle it is given.
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }
}
