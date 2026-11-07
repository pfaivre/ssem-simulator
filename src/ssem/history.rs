//! Execution history for time-travel debugging.
//!
//! The machine state is tiny (accumulator, program counter, and 32 store
//! words), so the history keeps a full machine state per cycle in a ring buffer.
//! When the buffer is full, the oldest states are overwritten.

use super::simulator::LastInstruction;

/// Upper bound on history capacity. Each `MachineState` is 148 bytes
/// (two `i32`, `[i32; 32]`, and the last instruction), so this cap keeps
/// the pre-allocated ring buffer under ~14.8 GB.
pub const MAX_CAPACITY: usize = 100_000_000;

/// The machine's architectural state at one cycle.
///
/// The last executed instruction is stored alongside the core state, so
/// the display can mark the executed line even after a jump (when CI no
/// longer points at it). The heatmap counters (execution and read/write)
/// are not part of the machine state: they are kept while the history is
/// browsed, and reset only when the machine restarts from a rewound
/// position.
#[derive(Debug, Clone, Copy, PartialEq)]
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
            states: vec![
                MachineState {
                    a: 0,
                    ci: 0,
                    store: [0; 32],
                    last_instruction: None,
                };
                capacity
            ],
            capacity: capacity as u64,
            head: 0,
            count: 0,
        }
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
        Some(&self.states[(cycle % self.capacity) as usize])
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
        self.states[slot] = machine_state;
        if self.count == 0 || cycle > self.head {
            self.count = (self.count + 1).min(self.capacity);
        } else {
            self.count = cycle - self.start() + 1;
        }
        self.head = cycle;
    }

    /// Drop every retained state, keeping the allocated capacity, so
    /// the next `record` starts a fresh history at the cycle it is given.
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{History, MachineState};
    use crate::ssem::Opcode;
    use crate::ssem::simulator::LastInstruction;

    fn state(ci: i32) -> MachineState {
        MachineState {
            a: ci * 10,
            ci,
            store: [ci; 32],
            last_instruction: None,
        }
    }

    #[test]
    fn machine_state_size_is_stable() {
        // The ring buffer pre-allocates capacity × size_of::<MachineState>();
        // keep the documented memory budget in sync with the real size.
        assert_eq!(148, std::mem::size_of::<MachineState>());
    }

    #[test]
    fn machine_state_round_trips_last_instruction() {
        let li = LastInstruction {
            address: 3,
            opcode: Opcode::STO,
            data: 7,
        };
        let mut h = History::new(4);
        h.record(
            0,
            MachineState {
                a: 1,
                ci: 2,
                store: [0; 32],
                last_instruction: Some(li),
            },
        );
        assert_eq!(Some(li), h.get(0).unwrap().last_instruction);
    }

    #[test]
    fn record_and_get() {
        let mut h = History::new(8);
        assert!(h.is_empty());
        assert_eq!(None, h.get(0));

        h.record(0, state(0));
        h.record(1, state(1));
        h.record(2, state(2));

        assert_eq!(3, h.len());
        assert_eq!(0, h.start());
        assert_eq!(2, h.head());
        assert_eq!(Some(&state(0)), h.get(0));
        assert_eq!(Some(&state(2)), h.get(2));
        assert_eq!(None, h.get(3));
    }

    #[test]
    fn clear_drops_everything_and_keeps_capacity() {
        let mut h = History::new(4);
        for c in 0..=6 {
            h.record(c, state(c as i32));
        }
        assert_eq!(4, h.len());

        h.clear();
        assert!(h.is_empty());
        assert_eq!(4, h.capacity(), "capacity is kept");
        assert_eq!(None, h.get(3));
        assert_eq!(None, h.get(6));

        // The next record starts a fresh history at the given cycle
        h.record(42, state(42));
        assert_eq!(1, h.len());
        assert_eq!(42, h.start());
        assert_eq!(42, h.head());
        assert_eq!(Some(&state(42)), h.get(42));
    }

    #[test]
    fn ring_overwrites_oldest() {
        let mut h = History::new(4);
        for c in 0..=6 {
            h.record(c, state(c as i32));
        }
        assert_eq!(4, h.len());
        assert_eq!(3, h.start());
        assert_eq!(6, h.head());
        assert_eq!(None, h.get(2));
        assert_eq!(Some(&state(3)), h.get(3));
        assert_eq!(Some(&state(6)), h.get(6));
    }

    #[test]
    fn record_at_or_before_tip_drops_stale_future() {
        let mut h = History::new(8);
        for c in 0..=6 {
            h.record(c, state(c as i32));
        }

        // Re-executing from cycle 4 overwrites the stale future
        h.record(5, state(100));
        assert_eq!(5, h.head());
        assert_eq!(6, h.len()); // retained: 0..=5
        assert_eq!(Some(&state(100)), h.get(5));
        assert_eq!(None, h.get(6));
        assert_eq!(Some(&state(4)), h.get(4));
    }

    #[test]
    fn record_past_old_tip_after_rewind_evicts_oldest() {
        let mut h = History::new(8);
        for c in 0..=6 {
            h.record(c, state(c as i32));
        }
        h.record(5, state(100)); // rewound, retained: 0..=5

        // Appending past the old tip evicts the oldest once the ring is full
        h.record(6, state(60));
        h.record(7, state(70));
        h.record(8, state(80));
        assert_eq!(8, h.head());
        assert_eq!(8, h.len());
        assert_eq!(1, h.start());
        assert_eq!(None, h.get(0));
        assert_eq!(Some(&state(1)), h.get(1));
        assert_eq!(Some(&state(60)), h.get(6));
        assert_eq!(Some(&state(80)), h.get(8));
    }
}
