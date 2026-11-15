use crate::ssem::Opcode;
use crate::ssem::history::{History, MAX_CAPACITY, MachineState};
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
    // The history footprint scales with this size; keep it in sync with
    // the real one.
    assert_eq!(148, std::mem::size_of::<MachineState>());
}

#[test]
fn new_allocates_nothing_and_grows_on_demand() {
    // A history at the maximum capacity must not pre-allocate: memory
    // follows the cycles actually recorded.
    let mut h = History::new(MAX_CAPACITY);
    assert_eq!(0, h.allocated());

    h.record(0, state(0));
    h.record(1, state(1));
    h.record(2, state(2));
    assert!(
        h.allocated() <= 8,
        "a handful of records stays tiny, got {}",
        h.allocated()
    );
    assert_eq!(Some(&state(2)), h.get(2));
    assert_eq!(None, h.get(3));
}

#[test]
fn first_record_at_an_arbitrary_cycle_grows_past_the_slot() {
    let mut h = History::new(8);
    h.record(5, state(5));
    assert_eq!(6, h.allocated(), "grown past the slot, once");
    assert_eq!(1, h.len());
    assert_eq!(5, h.start());
    assert_eq!(5, h.head());
    assert_eq!(Some(&state(5)), h.get(5));
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
