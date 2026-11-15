use std::path::{Path, PathBuf};

use super::{TempFile, instr};
use crate::ssem::Opcode;
use crate::ssem::simulator::{Breakpoint, LastInstruction, Simulator, TYPICAL_SPEED};

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
    let sim = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    assert_eq!(0, sim.a);
    assert_eq!(0, sim.ci);
    // 01 LDN 24
    assert_eq!(instr(Opcode::LDN, 24), sim.store.words[1]);
}

#[test]
fn from_file_snp() {
    let sim = Simulator::from_file(Path::new("samples/nightmare.snp")).unwrap();
    assert_eq!(0, sim.a);
    assert_eq!(0, sim.ci);
    assert_eq!(
        "00000110101001000100000100000100",
        format!("{:032b}", sim.store.words[0].reverse_bits())
    );
}

#[test]
fn from_file_unknown_extension() {
    let err = Simulator::from_file(Path::new("samples/virpet.txt"))
        .err()
        .unwrap();
    assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
    assert!(err.to_string().contains(".asm or .snp"), "err: {err}");
}

#[test]
fn from_file_no_extension() {
    let err = Simulator::from_file(Path::new("samples")).err().unwrap();
    assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
    assert!(err.to_string().contains(".asm or .snp"), "err: {err}");
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
fn instruction_cycle_does_not_overflow_on_max_ci() {
    let mut sim = Simulator::new();
    sim.ci = i32::MAX;
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
    sim.execute(Opcode::NUM, 0);
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
fn instruction_cycle_increments_exec_counts() {
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::STP, 0);
    sim.store.words[2] = instr(Opcode::STP, 0);

    assert!(sim.exec_counts.iter().all(|&c| c == 0));

    sim.instruction_cycle(); // executes line 1
    assert_eq!(1, sim.exec_counts[1]);
    assert_eq!(0, sim.exec_counts[2]);

    sim.instruction_cycle(); // executes line 2
    assert_eq!(1, sim.exec_counts[1]);
    assert_eq!(1, sim.exec_counts[2]);

    // Run a loop: line 1 jumps to itself
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::JMP, 3);
    sim.store.words[3] = 1; // JMP reads store[3] which holds 1
    sim.run(10);
    assert!(
        sim.exec_counts[1] > 1,
        "line 1 should be executed multiple times"
    );
}

#[test]
fn exec_counts_attributed_to_executed_line_not_target() {
    // CMP with negative A skips the next instruction.
    // The count must land on the CMP line, not the skipped line.
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::CMP, 0);
    sim.store.words[2] = instr(Opcode::STP, 0); // will be skipped
    sim.store.words[3] = instr(Opcode::STP, 0);
    sim.a = -1; // negative → CMP skips line 2

    sim.instruction_cycle(); // fetch→line 1 (CMP), skip→CI=2
    assert_eq!(1, sim.exec_counts[1], "CMP line should be counted");
    assert_eq!(0, sim.exec_counts[2], "skipped line should not be counted");
    assert_eq!(2, sim.ci, "CI points past the skipped line");

    // last_instruction should show the CMP, not the skip target
    let li = sim.last_instruction.unwrap();
    assert_eq!(1, li.address);
    assert_eq!(Opcode::CMP, li.opcode);

    sim.instruction_cycle(); // fetch→line 3 (STP)
    assert_eq!(1, sim.exec_counts[3]);
    assert_eq!(0, sim.exec_counts[2], "line 2 was never executed");
}

#[test]
fn instruction_cycle_counts_reads_and_writes() {
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::LDN, 6); // reads word 6
    sim.store.words[2] = instr(Opcode::SUB, 7); // reads word 7
    sim.store.words[3] = instr(Opcode::STO, 8); // writes word 8
    sim.store.words[4] = instr(Opcode::JMP, 9); // reads word 9, jumps to 4
    sim.store.words[5] = instr(Opcode::JRP, 9); // reads word 9, CI += 4
    sim.store.words[9] = 4;
    sim.store.words[10] = instr(Opcode::CMP, 12); // no store access
    sim.store.words[11] = instr(Opcode::STP, 13); // no store access, halts

    assert!(sim.read_counts.iter().all(|&c| c == 0));
    assert!(sim.write_counts.iter().all(|&c| c == 0));

    // Cycles execute lines 1, 2, 3, 4, 5, 10, 11
    sim.run(7);
    assert_eq!(1, sim.read_counts[6], "LDN reads its operand");
    assert_eq!(1, sim.read_counts[7], "SUB reads its operand");
    assert_eq!(1, sim.write_counts[8], "STO writes its operand");
    assert_eq!(2, sim.read_counts[9], "JMP and JRP both read word 9");
    assert_eq!(0, sim.read_counts[12], "CMP ignores its data field");
    assert_eq!(0, sim.write_counts[13], "STP ignores its data field");
    assert_eq!(0, sim.read_counts[1], "executed lines are not reads");
}

#[test]
fn rewind_keeps_read_write_counts_until_reexecution() {
    let mut sim = history_test_program(); // LDN 6, SUB 7, STO 8, STP
    sim.enable_history(100);
    sim.run(3);
    assert_eq!(1, sim.read_counts[6]);
    assert_eq!(1, sim.read_counts[7]);
    assert_eq!(1, sim.write_counts[8]);

    // Scrubbing back keeps the heatmap values
    assert!(sim.rewind_to(0));
    assert_eq!(1, sim.read_counts[6]);
    assert_eq!(1, sim.read_counts[7]);
    assert_eq!(1, sim.write_counts[8]);

    // Restarting from the rewound state resets them: only the
    // re-executed cycle (LDN 6) is counted again
    sim.run(1);
    assert_eq!(1, sim.read_counts[6]);
    assert_eq!(0, sim.read_counts[7]);
    assert_eq!(0, sim.write_counts[8]);
}

#[test]
fn exec_counts_attributed_to_jmp_line_not_target() {
    // JMP sets CI from the store. The count must land on the JMP line.
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::JMP, 2);
    sim.store.words[2] = 5; // jump target
    sim.store.words[5] = instr(Opcode::STP, 0);

    sim.instruction_cycle(); // executes line 1 (JMP 2), CI → 5
    assert_eq!(1, sim.exec_counts[1], "JMP line should be counted");
    assert_eq!(
        0, sim.exec_counts[5],
        "jump target should not be counted yet"
    );
    assert_eq!(5, sim.ci);

    let li = sim.last_instruction.unwrap();
    assert_eq!(1, li.address, "last_instruction should show the JMP line");
    assert_eq!(Opcode::JMP, li.opcode);
}

#[test]
fn last_instruction_display() {
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
    assert_eq!(TYPICAL_SPEED, sim.speed);
    assert_eq!(700, sim.speed);
}

#[test]
fn breakpoint_description_word_value() {
    let bp = Breakpoint::WordValue {
        address: 5,
        value: -42,
        was_met: false,
    };
    assert_eq!("Word 05 = -42", bp.description());
}

#[test]
fn breakpoint_description_ci_value() {
    let bp = Breakpoint::CiValue {
        address: 10,
        was_met: false,
    };
    assert_eq!("CI = 10", bp.description());
}

#[test]
fn breakpoint_equality_ignores_was_met() {
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

/// A small program: a = -(store[6] + store[7]), stored to word 8.
fn history_test_program() -> Simulator {
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::LDN, 6);
    sim.store.words[2] = instr(Opcode::SUB, 7);
    sim.store.words[3] = instr(Opcode::STO, 8);
    sim.store.words[4] = instr(Opcode::STP, 0);
    sim.store.words[6] = 5;
    sim.store.words[7] = 3;
    sim
}

#[test]
fn enable_history_records_initial_state() {
    let mut sim = history_test_program();
    sim.enable_history(10);
    let h = sim.history.as_ref().unwrap();
    assert_eq!(1, h.len());
    assert_eq!(0, h.head());
    let s = h.get(0).unwrap();
    assert_eq!(0, s.a);
    assert_eq!(0, s.ci);
    assert_eq!(instr(Opcode::LDN, 6), s.store[1]);
    assert_eq!(None, s.last_instruction, "nothing executed at cycle 0");
    assert_eq!(None, h.get(1));
}

#[test]
fn instruction_cycle_records_history() {
    let mut sim = history_test_program();
    sim.enable_history(10);
    sim.instruction_cycle(); // LDN 6
    sim.instruction_cycle(); // SUB 7

    let h = sim.history.as_ref().unwrap();
    assert_eq!(3, h.len()); // initial state + 2 cycles
    assert_eq!(2, h.head());
    assert_eq!(-5, h.get(1).unwrap().a);
    assert_eq!(1, h.get(1).unwrap().ci);
    assert_eq!(-8, h.get(2).unwrap().a);
    assert_eq!(2, h.get(2).unwrap().ci);
}

#[test]
fn history_disabled_by_default() {
    let mut sim = history_test_program();
    sim.run(4);
    assert!(sim.history.is_none());
}

#[test]
fn rewind_restores_state_and_keeps_heatmap() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // LDN, SUB, STO
    assert_eq!(-8, sim.a);
    assert_eq!(3, sim.cycle_count);
    assert_eq!(-8, sim.store.words[8]);
    assert!(sim.exec_counts.iter().any(|&c| c > 0));
    assert!(sim.last_instruction.is_some());

    assert!(sim.rewind_to(0));
    assert_eq!(0, sim.a);
    assert_eq!(0, sim.ci);
    assert_eq!(0, sim.cycle_count);
    assert_eq!(0, sim.store.words[8]);
    assert!(
        sim.exec_counts.iter().any(|&c| c > 0),
        "scrubbing the history keeps the heatmap"
    );
    assert!(
        sim.last_instruction.is_none(),
        "cycle 0 predates any executed instruction"
    );
    assert!(sim.stop_flag, "browsing the history pauses the machine");
    // The retained history is preserved for forward scrubbing
    let h = sim.history.as_ref().unwrap();
    assert_eq!(4, h.len()); // initial state + 3 cycles
    assert_eq!(3, h.head());
}

#[test]
fn restart_from_rewound_state_resets_heatmap() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // lines 1, 2, 3: LDN 6, SUB 7, STO 8
    assert_eq!(1, sim.exec_counts[1]);
    assert_eq!(1, sim.exec_counts[2]);
    assert_eq!(1, sim.exec_counts[3]);

    assert!(sim.rewind_to(1));
    assert_eq!(
        1, sim.exec_counts[1],
        "scrubbing keeps the heatmap of the discarded future"
    );

    // Re-executing from cycle 1 resets the counters, so only the
    // re-executed lines (2 and 3) are counted
    sim.run(2);
    assert_eq!(0, sim.exec_counts[1], "stale future counts are gone");
    assert_eq!(1, sim.exec_counts[2]);
    assert_eq!(1, sim.exec_counts[3]);
    assert_eq!(0, sim.read_counts[6]);
    assert_eq!(1, sim.read_counts[7]);
    assert_eq!(1, sim.write_counts[8]);
}

#[test]
fn continuing_from_the_tip_does_not_reset_heatmap() {
    // Scrubbing to the head and resuming is a plain continuation,
    // not a restart from a previous state: the heatmap accumulates.
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(2); // lines 1, 2
    assert!(sim.rewind_to(sim.history.as_ref().unwrap().head()));
    assert_eq!(2, sim.cycle_count);

    sim.run(1); // line 3
    assert_eq!(1, sim.exec_counts[1]);
    assert_eq!(1, sim.exec_counts[2]);
    assert_eq!(1, sim.exec_counts[3]);
}

#[test]
fn scrubbing_back_to_the_tip_cancels_the_pending_heatmap_reset() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // lines 1, 2, 3

    assert!(sim.rewind_to(1));
    assert!(sim.heatmap_reset_pending, "rewound behind the head");
    // Scrub forward to the tip without executing anything
    assert!(sim.rewind_to(3));
    assert!(!sim.heatmap_reset_pending, "back at the tip: continuation");

    sim.run(1); // line 4: STP
    assert_eq!(1, sim.exec_counts[1], "no reset happened");
    assert_eq!(1, sim.exec_counts[2]);
    assert_eq!(1, sim.exec_counts[3]);
    assert_eq!(1, sim.exec_counts[4]);
}

#[test]
fn heatmap_reset_happens_once_on_restart() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3);

    assert!(sim.rewind_to(0));
    sim.run(3); // re-executes lines 1, 2, 3
    assert!(
        !sim.heatmap_reset_pending,
        "latch cleared after the first cycle"
    );
    assert_eq!(1, sim.exec_counts[1]);
    assert_eq!(1, sim.exec_counts[2]);
    assert_eq!(1, sim.exec_counts[3]);
    assert_eq!(1, sim.read_counts[6]);
    assert_eq!(1, sim.read_counts[7]);
    assert_eq!(1, sim.write_counts[8]);
}

#[test]
fn clear_wipes_store_heatmap_and_history() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // LDN 6, SUB 7, STO 8
    std::sync::Arc::make_mut(&mut sim.store.comments)[6] = Some("operand".into());
    sim.breakpoints.push(Breakpoint::CiValue {
        address: 2,
        was_met: false,
    });
    assert!(sim.exec_counts.iter().any(|&c| c > 0));
    assert!(sim.read_counts.iter().any(|&c| c > 0));
    assert!(sim.write_counts.iter().any(|&c| c > 0));
    assert!(sim.history.as_ref().unwrap().len() > 1);
    assert_eq!(3, sim.cycle_count);

    sim.clear();

    // The machine looks like when opened without a program
    assert_eq!(0, sim.cycle_count, "cycle count reset");
    assert_eq!(0, sim.a);
    assert_eq!(0, sim.ci);
    assert!(sim.last_instruction.is_none());
    assert!(sim.breakpoints.is_empty(), "breakpoints dropped");
    assert!(sim.store.words.iter().all(|&w| w == 0), "store wiped");
    assert!(
        sim.store.comments.iter().all(Option::is_none),
        "comments wiped"
    );
    assert!(
        sim.exec_counts.iter().all(|&c| c == 0),
        "exec heatmap reset"
    );
    assert!(
        sim.read_counts.iter().all(|&c| c == 0),
        "read heatmap reset"
    );
    assert!(
        sim.write_counts.iter().all(|&c| c == 0),
        "write heatmap reset"
    );
    assert!(!sim.heatmap_reset_pending);

    // The history restarts at cycle 0, holding only the cleared
    // state; the pre-clear cycles are gone
    let h = sim.history.as_ref().unwrap();
    assert_eq!(1, h.len());
    assert_eq!(0, h.start());
    assert_eq!(0, h.head());
    assert!(
        h.get(0).unwrap().store.iter().all(|&w| w == 0),
        "history holds the cleared state at cycle 0"
    );
    assert_eq!(None, h.get(3), "the pre-clear history is gone");

    // The machine keeps recording from cycle 0
    sim.instruction_cycle();
    assert_eq!(1, sim.cycle_count);
    let h = sim.history.as_ref().unwrap();
    assert_eq!(2, h.len());
    assert_eq!(sim.cycle_count, h.start() + 1);
    assert_eq!(sim.cycle_count, h.head());
}

#[test]
fn clear_without_history_keeps_history_disabled() {
    let mut sim = history_test_program();
    sim.run(2);
    assert!(sim.history.is_none());

    sim.clear();

    assert!(sim.history.is_none(), "history stays disabled");
    assert_eq!(0, sim.cycle_count, "cycle count reset");
    assert!(sim.store.words.iter().all(|&w| w == 0));
    assert!(sim.exec_counts.iter().all(|&c| c == 0));
    assert!(sim.read_counts.iter().all(|&c| c == 0));
    assert!(sim.write_counts.iter().all(|&c| c == 0));
}

#[test]
fn load_file_replaces_the_state_and_keeps_the_history() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3);
    sim.breakpoints.push(Breakpoint::CiValue {
        address: 2,
        was_met: false,
    });
    assert_eq!(3, sim.cycle_count);

    sim.load_file(Path::new("samples/factorct.asm")).unwrap();

    let reference = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    assert_eq!(reference.store.words, sim.store.words, "store from file");
    assert_eq!(
        reference.store.comments, sim.store.comments,
        "comments from file"
    );
    assert_eq!(Some(PathBuf::from("samples/factorct.asm")), sim.source_path);
    assert_eq!(0, sim.cycle_count, "cycle count reset");
    assert_eq!(0, sim.a);
    assert_eq!(0, sim.ci);
    assert!(sim.stop_flag, "the loaded machine starts paused");
    assert!(sim.breakpoints.is_empty(), "breakpoints dropped");
    assert!(sim.exec_counts.iter().all(|&c| c == 0));

    // The history keeps its capacity and restarts at the loaded state
    let h = sim.history.as_ref().expect("history kept");
    assert_eq!(100, h.capacity());
    assert_eq!(1, h.len());
    assert_eq!(0, h.start());
    assert_eq!(0, h.head());
    assert_eq!(sim.store.words, h.get(0).unwrap().store);
}

#[test]
fn load_file_error_leaves_the_machine_untouched() {
    let mut sim = history_test_program();
    sim.run(2);
    sim.store.words[3] = 42;
    let before = sim.store.words;

    assert!(
        sim.load_file(Path::new("samples/does_not_exist.asm"))
            .is_err()
    );
    assert!(sim.load_file(Path::new("samples/virpet.txt")).is_err());

    assert_eq!(before, sim.store.words, "store unchanged");
    assert_eq!(2, sim.cycle_count, "cycle count unchanged");
    assert_eq!(None, sim.source_path, "source path unchanged");
}

#[test]
fn rewind_restores_last_instruction() {
    // After a JMP, CI points at the jump's target value, not at the
    // executed line. Rewinding to that cycle must restore the last
    // instruction (the JMP) so the display can mark the executed line.
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::JMP, 2);
    sim.store.words[2] = 5; // the value JMP loads into CI
    sim.store.words[6] = instr(Opcode::STP, 0);
    sim.enable_history(100);
    sim.run(2); // cycle 1: JMP at line 1, CI → 5; cycle 2: STP at line 6
    assert_eq!(6, sim.ci);
    assert_eq!(6, sim.last_instruction.unwrap().address);

    assert!(sim.rewind_to(1));
    assert_eq!(5, sim.ci);
    let li = sim.last_instruction.unwrap();
    assert_eq!(1, li.address);
    assert_eq!(Opcode::JMP, li.opcode);
    assert_eq!(2, li.data);
}

#[test]
fn history_records_last_instruction() {
    let mut sim = Simulator::new();
    sim.store.words[1] = instr(Opcode::JMP, 2);
    sim.store.words[2] = 5;
    sim.enable_history(10);
    sim.instruction_cycle(); // executes line 1 (JMP 2), CI → 5

    let h = sim.history.as_ref().unwrap();
    let s = h.get(1).unwrap();
    assert_eq!(5, s.ci);
    let li = s.last_instruction.unwrap();
    assert_eq!(1, li.address, "the recorded instruction is the JMP line");
    assert_eq!(Opcode::JMP, li.opcode);
    assert_eq!(2, li.data);
}

#[test]
fn rewind_preserves_history_for_forward_scrubbing() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(4);

    assert!(sim.rewind_to(1));
    assert_eq!(1, sim.cycle_count);
    // The retained history is still browsable in both directions
    assert!(sim.rewind_to(3));
    assert_eq!(3, sim.cycle_count);
    assert!(sim.rewind_to(1));
    assert_eq!(1, sim.cycle_count);
    let h = sim.history.as_ref().unwrap();
    assert_eq!(4, h.head());
    assert_eq!(5, h.len());
}

#[test]
fn rewind_out_of_range_returns_false() {
    let mut sim = history_test_program();
    sim.enable_history(10);
    sim.instruction_cycle();
    assert!(!sim.rewind_to(99));
    assert_eq!(1, sim.ci, "state must be unchanged");
}

#[test]
fn rewind_without_history_returns_false() {
    let mut sim = Simulator::new();
    assert!(!sim.rewind_to(0));
}

#[test]
fn rewind_then_rerun_reproduces_recorded_states() {
    // The machine is deterministic: re-executing from a rewound cycle
    // reproduces exactly the states that were recorded before.
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(4);
    let final_a = sim.a;
    let final_store = sim.store.words;

    assert!(sim.rewind_to(1));
    assert_eq!(1, sim.cycle_count);
    sim.run(3);
    assert_eq!(final_a, sim.a);
    assert_eq!(final_store, sim.store.words);
    assert_eq!(4, sim.cycle_count);
}

#[test]
fn rewind_then_edit_truncates_future() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // head = 3
    let old_cycle2_a = sim.history.as_ref().unwrap().get(2).unwrap().a;

    assert!(sim.rewind_to(1));
    sim.store.words[7] = 100; // change what the future will compute
    sim.instruction_cycle(); // executes line 2 (SUB 7) with the edited word

    let h = sim.history.as_ref().unwrap();
    assert_eq!(2, h.head());
    assert_eq!(None, h.get(3), "stale future must be truncated");
    assert_ne!(old_cycle2_a, h.get(2).unwrap().a);
    assert_eq!(-105, h.get(2).unwrap().a); // -5 - 100
}

#[test]
fn restart_history_makes_the_edited_state_the_earliest() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // head = 3

    // Rewind, then edit a word: the history restarts at the edited state
    assert!(sim.rewind_to(1));
    sim.store.words[7] = 100;
    sim.restart_history();

    let h = sim.history.as_ref().unwrap();
    assert_eq!(1, h.len());
    assert_eq!(1, h.start(), "the edited state is the earliest");
    assert_eq!(1, h.head());
    assert_eq!(100, h.get(1).unwrap().store[7]);
    assert_eq!(None, h.get(0), "the pre-edit history is gone");
    assert_eq!(None, h.get(3));

    // The machine keeps recording from there
    sim.instruction_cycle(); // executes line 2 (SUB 7) with the edited word
    let h = sim.history.as_ref().unwrap();
    assert_eq!(2, h.len());
    assert_eq!(1, h.start());
    assert_eq!(2, h.head());
    assert_eq!(-105, h.get(2).unwrap().a); // -5 - 100
}

#[test]
fn restart_history_at_the_tip_drops_only_the_past() {
    let mut sim = history_test_program();
    sim.enable_history(100);
    sim.run(3); // head = 3, no rewind

    sim.store.words[7] = 100;
    sim.restart_history();

    let h = sim.history.as_ref().unwrap();
    assert_eq!(1, h.len());
    assert_eq!(3, h.start());
    assert_eq!(3, h.head());
    assert_eq!(100, h.get(3).unwrap().store[7]);
    assert_eq!(None, h.get(2), "earlier cycles are no longer browsable");
}

#[test]
fn restart_history_without_history_is_noop() {
    let mut sim = history_test_program();
    sim.run(2);
    assert!(sim.history.is_none());

    sim.restart_history();

    assert!(sim.history.is_none(), "history stays disabled");
    assert_eq!(2, sim.cycle_count, "the machine state is untouched");
}

#[test]
fn save_writes_a_header_comment_and_the_store() {
    let mut sim = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    sim.run(10);
    let file = TempFile::new("save.snp");
    sim.save(file.path()).unwrap();

    let content = std::fs::read_to_string(file.path()).unwrap();
    let mut lines = content.lines();
    assert!(
        lines
            .next()
            .unwrap()
            .starts_with("; Machine state saved by ssem-simulator v")
    );
    let header = lines.next().unwrap();
    assert!(
        header.contains(&format!("CI: {}", sim.ci)),
        "header: {header}"
    );
    assert!(
        header.contains(&format!("A: {}", sim.a)),
        "header: {header}"
    );
    assert!(
        header.contains(&format!("cycles: {}", sim.cycle_count)),
        "header: {header}"
    );
    assert!(lines.next().unwrap().starts_with("0000: "));

    // The saved file reloads as the same store, comments included
    let reloaded = Simulator::from_file(file.path()).unwrap();
    assert_eq!(sim.store, reloaded.store);
    assert_eq!(sim.store.comments, reloaded.store.comments);
}

#[test]
fn save_asm_round_trips_and_keeps_comments() {
    let mut sim = Simulator::from_file(Path::new("samples/factorct.asm")).unwrap();
    sim.run(10);
    let file = TempFile::new("save.asm");
    sim.save(file.path()).unwrap();

    let content = std::fs::read_to_string(file.path()).unwrap();
    assert!(content.contains("01 LDN 24; A = -a"), "asm line: {content}");
    let reloaded = Simulator::from_file(file.path()).unwrap();
    assert_eq!(sim.store, reloaded.store);
    assert_eq!(sim.store.comments, reloaded.store.comments);
}

#[test]
fn save_rejects_unknown_extension() {
    let sim = Simulator::new();
    let err = sim.save(Path::new("state.txt")).unwrap_err();
    assert_eq!(std::io::ErrorKind::InvalidInput, err.kind());
    assert!(err.to_string().contains(".asm or .snp"));
}
