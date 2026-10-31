//! A point-in-time copy of the simulator state, taken by the UI thread under
//! a brief lock so drawing never holds the lock.

use crate::ssem::simulator::{Breakpoint, LastInstruction, Simulator};

/// A snapshot of the simulator state, taken by the UI thread under a brief lock.
///
/// Drawing uses this snapshot instead of the live simulator so that the
/// (potentially slow) terminal I/O in `terminal.draw()` never holds the lock
/// and never blocks the machine thread.
pub(super) struct Snapshot {
    pub(super) ci: i32,
    pub(super) a: i32,
    pub(super) store: [i32; 32],
    pub(super) cycle_count: u64,
    pub(super) speed: u32,
    pub(super) stop_flag: bool,
    pub(super) last_instruction: Option<LastInstruction>,
    pub(super) breakpoints: Vec<Breakpoint>,
    pub(super) exec_counts: [u64; 32],
    pub(super) read_counts: [u64; 32],
    pub(super) write_counts: [u64; 32],
    pub(super) history_capacity: u64,
    pub(super) history_start: u64,
    pub(super) history_head: u64,
    pub(super) history_len: u64,
}

impl Snapshot {
    pub(super) fn from_sim(sim: &Simulator) -> Self {
        let (history_capacity, history_start, history_head, history_len) = match &sim.history {
            Some(h) => (h.capacity(), h.start(), h.head(), h.len()),
            None => (0, 0, 0, 0),
        };
        Snapshot {
            ci: sim.ci,
            a: sim.a,
            store: sim.store.words,
            cycle_count: sim.cycle_count,
            speed: sim.speed,
            stop_flag: sim.stop_flag,
            last_instruction: sim.last_instruction,
            breakpoints: sim.breakpoints.clone(),
            exec_counts: sim.exec_counts,
            read_counts: sim.read_counts,
            write_counts: sim.write_counts,
            history_capacity,
            history_start,
            history_head,
            history_len,
        }
    }
}
