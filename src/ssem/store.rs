use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Index;
use std::str::FromStr;

use crate::ssem::opcode::Opcode;

const ASM_COMMENT_CHAR: char = ';';
const SSEM_STORE_WORDS: i32 = 32;
const SSEM_DATA_MASK: i32 = 0b00000000000000000000000000011111;
const SSEM_OPCODE_MASK: i32 = 0b00000000000000000000000000000111;
const SSEM_OPCODE_BIT_SHIFT: u8 = 13;

/// Main memory of a SSEM-like machine
///
/// Note: SSEM writes integers with its least significant to most significant bits from left to right.
/// This is the opposite of modern computers, here we simply reverse the order during display.
pub struct Store {
    pub words: Vec<i32>,
    pub size: i32,
}

impl Store {
    /// Instanciate a new store.
    ///
    /// Every word gets initialized to zero.
    pub fn new() -> Store {
        Store {
            words: vec![0_i32; usize::try_from(SSEM_STORE_WORDS).unwrap()],
            size: SSEM_STORE_WORDS,
        }
    }

    /// Initializes the store with the given assembly file
    ///
    /// An assembly file has the following form:
    /// ```
    /// 00 JMP 0  ;
    /// 01 LDN 24 ; -24 to C
    /// 02 STO 26 ; C to 26
    /// ...
    /// ```
    /// Each numbered line represents a word with its instruction. Erverything after ';' is ignored.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the file to read
    pub fn from_asm_file(filename: &str) -> Store {
        // Todo return a Result for a richer explaination on the possible issues
        let file = File::open(filename);
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                panic!("Error while reading '{}': {}", filename, e);
            },
        };
        let reader = BufReader::new(file);

        let mut store = Store {
            words: vec![0_i32; usize::try_from(SSEM_STORE_WORDS).unwrap()],
            size: SSEM_STORE_WORDS,
        };

        for (_, line) in reader.lines().enumerate() {

            let line = line.unwrap_or_else(|e| {
                panic!("Error while reading '{}': {}", filename, e);
            });

            // Ignoring comments
            let exploded_line: Vec<&str> = line.splitn(2, ASM_COMMENT_CHAR).collect();
            if exploded_line.len() > 0 {
                let instruction = exploded_line[0].clone().trim();
                if instruction.len() == 0 {
                    continue;
                }
                // Extracting tokens "<index> <opcode> <operand>"
                let i: Vec<&str> = instruction.split_ascii_whitespace().collect();
                if i.len() < 2 {
                    panic!("Unable to read the input file: invalid instruction '{}'", instruction);
                }

                let index: i32 = i[0].parse().expect("Unable to read the number");
                let opcode: &str = i[1];
                let operand: i32;
                if i.len() >= 3 {
                    operand = i[2].parse().expect("Unable to read the number");
                }
                else {
                    operand = 0;
                }

                if index >= store.size {
                    eprintln!("Unable to read the input file: invalid instruction '{}'", instruction);
                    panic!("Index '{}' is bigger than the machine size ({})", index, store.size);
                }

                let opcode = match Opcode::from_str(opcode) {
                    Ok(code) => code,
                    Err(_) => {
                        panic!("Error while reading '{}': opcode '{}' non valid.", filename, opcode);
                    }
                };

                match opcode {
                    Opcode::NUM => {
                        // TODO: make this safe
                        store.words[usize::try_from(index).unwrap()] = operand;
                    }
                    opcode => {
                        let mut w: i32 = 0;

                        // Print the opcode
                        w = w | ((opcode as i32) << SSEM_OPCODE_BIT_SHIFT);

                        // Print the operand
                        store.words[usize::try_from(index).unwrap()] = w | ((operand as i32));
                    }
                }
            }
        }

        store
    }

    /// Extract the opcode and data from the word at the given address
    pub fn decode_instruction(&self, address: i32) -> Result<(Opcode, i32), String> {
        let word = self[address];

        // Objective: extract opcode and data from word
        // word: 0b00000000000000000100000000011000
        //                         ===        =====
        //        Operation code ---'           |
        //                  Data ---------------'

        // Step 1: extract instruction data (5 bits)
        // word: 0b00000000000000000100000000011000
        // mask: 0b00000000000000000000000000011111
        //       ----------------------------------
        //    &: 0b00000000000000000000000000011000
        let data = SSEM_DATA_MASK & word;

        // Step 2: Shift bits to put the opcode on the edge
        // word: 0b00000000000000000100000000011000
        //       ----------------------------------
        // >>13: 0b00000000000000000000000000000010

        // Step 3: Extract opcode (3 bits)
        // word: 0b00000000000000000000000000000010
        // mask: 0b00000000000000000000000000000111
        //       ----------------------------------
        //    &: 0b00000000000000000000000000000010
        let opcode = SSEM_OPCODE_MASK & (word >> SSEM_OPCODE_BIT_SHIFT);

        Ok((Opcode::from(opcode), data))
    }
}

impl Index<i32> for Store {
    type Output = i32;

    fn index(&self, index: i32) -> &Self::Output {
        let address = match usize::try_from(index) {
            Ok(value) => value,
            Err(_) => panic!("Unable to convert integer {} to usize", index),
        };
        let word: &i32 = match self.words.get(address) {
            Some(value) => value,
            None => panic!("Out of bound read at address {}", address),
        };
        word
    }
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for word in self.words.iter() {
            writeln!(f, " {:032b}", word.reverse_bits()).ok();
        }
        Ok(())
    }
}
