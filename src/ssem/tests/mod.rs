//! Tests for the simulator core, split by concern; the helpers shared by
//! all of them live in this file.

mod history;
mod integration;
mod opcode;
mod simulator;
mod store;

use crate::ssem::Opcode;

/// Encode an instruction word the same way `Store::from_asm_file` does:
/// opcode bits at position 13, operand in the low 5 bits.
fn instr(opcode: Opcode, data: i32) -> i32 {
    (opcode as i32) << 13 | data
}

/// The 32-bit string of a word in SSEM display order (least significant bit
/// first), the format `Store::from(Vec<String>)` expects.
fn word_bits(value: i32) -> String {
    format!("{:032b}", value.reverse_bits())
}

/// A 32-word store (as `Vec<String>`) of zero words with `entries`
/// overridden, each given as 32 bits in SSEM (least significant bit first)
/// order; later entries win.
fn store_from_bits(entries: &[(usize, &str)]) -> Vec<String> {
    let mut words = vec!["0".repeat(32); 32];
    for &(index, bits) in entries {
        words[index] = bits.to_string();
    }
    words
}

/// A temporary file removed when dropped, even if the test panics.
struct TempFile(std::path::PathBuf);

impl TempFile {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("ssem_tests_{name}"));
        let _ = std::fs::remove_file(&path); // Remove any leftover from a previous run
        TempFile(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
