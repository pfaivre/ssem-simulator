//! Execution history for time-travel debugging.
//!
//! The machine state is tiny (accumulator, program counter, and 32 store
//! words), so the history keeps a full snapshot per cycle in a ring buffer.
//! When the buffer is full, the oldest snapshots are overwritten.

use super::simulator::LastInstruction;

/// Upper bound on history capacity. Each `Snapshot` is 148 bytes
/// (two `i32`, `[i32; 32]`, and the last instruction), so this cap keeps
/// the pre-allocated ring buffer under ~14.8 GB.
pub const MAX_CAPACITY: usize = 100_000_000;

/// A snapshot of the machine's architectural state at one cycle.
///
/// The last executed instruction is stored alongside the core state, so
/// the display can mark the executed line even after a jump (when CI no
/// longer points at it). Only the heatmap counters are derived state and
/// are reset when the history is browsed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub a: i32,
    pub ci: i32,
    pub store: [i32; 32],
    pub last_instruction: Option<LastInstruction>,
}

/// A ring buffer of snapshots, indexed by absolute cycle number.
///
/// Snapshots are contiguous: the retained entries are the absolute cycles
/// `[start, head]`. `record` appends at the tip; when the buffer is full the
/// oldest entry is overwritten. Recording at a cycle at or before the tip
/// (re-executing from a rewound position) drops the stale future, so the
/// history always describes exactly what the machine has executed.
pub struct History {
    snapshots: Vec<Snapshot>,
    capacity: u64,
    /// Absolute cycle of the newest recorded snapshot.
    head: u64,
    /// Number of retained entries.
    count: u64,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "history capacity must be at least 1");
        History {
            snapshots: vec![
                Snapshot {
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

    /// Absolute cycle of the newest recorded snapshot.
    pub fn head(&self) -> u64 {
        self.head
    }

    /// Absolute cycle of the oldest retained snapshot.
    pub fn start(&self) -> u64 {
        self.head + 1 - self.count
    }

    /// The snapshot recorded at `cycle`, if it is still retained.
    pub fn get(&self, cycle: u64) -> Option<&Snapshot> {
        if self.count == 0 || cycle < self.start() || cycle > self.head {
            return None;
        }
        Some(&self.snapshots[(cycle % self.capacity) as usize])
    }

    /// Record the state at `cycle`: appends at the tip, or drops the stale
    /// future when recording at or before the tip (re-executing from a
    /// rewound position).
    pub fn record(&mut self, cycle: u64, snapshot: Snapshot) {
        if self.count > 0 {
            debug_assert!(
                cycle >= self.start() && cycle <= self.head + 1,
                "record at {cycle} outside [{}, {}]",
                self.start(),
                self.head + 1
            );
        }
        let slot = (cycle % self.capacity) as usize;
        self.snapshots[slot] = snapshot;
        if self.count == 0 || cycle > self.head {
            self.count = (self.count + 1).min(self.capacity);
        } else {
            self.count = cycle - self.start() + 1;
        }
        self.head = cycle;
    }
}

#[cfg(test)]
mod tests {
    use super::{History, Snapshot};
    use crate::ssem::Opcode;
    use crate::ssem::simulator::LastInstruction;

    fn snap(ci: i32) -> Snapshot {
        Snapshot {
            a: ci * 10,
            ci,
            store: [ci; 32],
            last_instruction: None,
        }
    }

    #[test]
    fn snapshot_size_is_stable() {
        // The ring buffer pre-allocates capacity × size_of::<Snapshot>();
        // keep the documented memory budget in sync with the real size.
        assert_eq!(148, std::mem::size_of::<Snapshot>());
    }

    #[test]
    fn snapshot_round_trips_last_instruction() {
        let li = LastInstruction {
            address: 3,
            opcode: Opcode::STO,
            data: 7,
        };
        let mut h = History::new(4);
        h.record(
            0,
            Snapshot {
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

        h.record(0, snap(0));
        h.record(1, snap(1));
        h.record(2, snap(2));

        assert_eq!(3, h.len());
        assert_eq!(0, h.start());
        assert_eq!(2, h.head());
        assert_eq!(Some(&snap(0)), h.get(0));
        assert_eq!(Some(&snap(2)), h.get(2));
        assert_eq!(None, h.get(3));
    }

    #[test]
    fn ring_overwrites_oldest() {
        let mut h = History::new(4);
        for c in 0..=6 {
            h.record(c, snap(c as i32));
        }
        assert_eq!(4, h.len());
        assert_eq!(3, h.start());
        assert_eq!(6, h.head());
        assert_eq!(None, h.get(2));
        assert_eq!(Some(&snap(3)), h.get(3));
        assert_eq!(Some(&snap(6)), h.get(6));
    }

    #[test]
    fn record_at_or_before_tip_drops_stale_future() {
        let mut h = History::new(8);
        for c in 0..=6 {
            h.record(c, snap(c as i32));
        }

        // Re-executing from cycle 4 overwrites the stale future
        h.record(5, snap(100));
        assert_eq!(5, h.head());
        assert_eq!(6, h.len()); // retained: 0..=5
        assert_eq!(Some(&snap(100)), h.get(5));
        assert_eq!(None, h.get(6));
        assert_eq!(Some(&snap(4)), h.get(4));
    }

    #[test]
    fn record_past_old_tip_after_rewind_evicts_oldest() {
        let mut h = History::new(8);
        for c in 0..=6 {
            h.record(c, snap(c as i32));
        }
        h.record(5, snap(100)); // rewound, retained: 0..=5

        // Appending past the old tip evicts the oldest once the ring is full
        h.record(6, snap(60));
        h.record(7, snap(70));
        h.record(8, snap(80));
        assert_eq!(8, h.head());
        assert_eq!(8, h.len());
        assert_eq!(1, h.start());
        assert_eq!(None, h.get(0));
        assert_eq!(Some(&snap(1)), h.get(1));
        assert_eq!(Some(&snap(60)), h.get(6));
        assert_eq!(Some(&snap(80)), h.get(8));
    }
}
