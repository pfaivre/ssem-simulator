use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::ops::Index;
use std::path::Path;
use std::str::FromStr;

use super::opcode::Opcode;

const ASM_COMMENT_CHAR: char = ';';
const SSEM_STORE_WORDS: i32 = 32;
const SSEM_DATA_MASK: i32 = 0b00000000000000000000000000011111; // u5 equivalent
const SSEM_OPCODE_MASK: i32 = 0b00000000000000000000000000000111; // u3 equivalent
const SSEM_OPCODE_BIT_SHIFT: u8 = 13;

/// Main memory of a SSEM-like machine
///
/// Note: SSEM writes integers with its least significant to most significant bits from left to right.
/// This is the opposite of modern computers, here we simply reverse the order during display.
#[derive(Debug)]
pub struct Store {
    pub words: Vec<i32>,
    pub size: i32,
}

impl Store {
    /// Instanciate a new store.
    ///
    /// Every word gets initialized to zero.
    pub fn new() -> Store {
        let store = Store {
            words: vec![0_i32; usize::try_from(SSEM_STORE_WORDS).unwrap()],
            size: SSEM_STORE_WORDS,
        };
        store._check();
        store
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
    pub fn from_asm_file(filename: &Path) -> Store {
        let file = File::open(filename);
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                panic!(
                    "Error while reading '{}': {}",
                    filename.to_str().unwrap_or(""),
                    e
                );
            }
        };
        let reader = BufReader::new(file);

        let mut store = Store {
            words: vec![0_i32; usize::try_from(SSEM_STORE_WORDS).unwrap()],
            size: SSEM_STORE_WORDS,
        };

        let mut last_index: i32 = 0;

        for (_, line) in reader.lines().enumerate() {
            let line = line.unwrap_or_else(|e| {
                panic!(
                    "Error while reading '{}': {}",
                    filename.to_str().unwrap_or(""),
                    e
                );
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
                    panic!(
                        "Unable to read the input file: invalid instruction '{}'",
                        instruction
                    );
                }

                let index: i32 = i[0].parse().expect("Unable to read the number");
                let opcode: &str = i[1];
                let operand: i32;
                if i.len() >= 3 {
                    operand = i[2].parse().expect("Unable to read the number");
                } else {
                    operand = 0;
                }

                // Ensure lines are contiguous and without duplicates
                if index > 0 && index != last_index + 1 {
                    panic!(
                        "Error near line '{}': expected index '{}'",
                        line,
                        last_index + 1
                    )
                }
                last_index = index;

                // Ensure we don't write outside of the store
                if index >= store.size {
                    panic!(
                        "Error near line '{}': Index '{}' is bigger than the machine size ({})",
                        instruction, index, store.size,
                    );
                }

                let opcode = Opcode::from_str(opcode).unwrap_or_else(|_| {
                    panic!(
                        "Error near line '{}': opcode '{}' non valid.",
                        instruction, opcode,
                    );
                });

                match opcode {
                    Opcode::NUM => {
                        // TODO: make this safe
                        store.words[usize::try_from(index).unwrap()] = operand;
                    }
                    opcode => {
                        // Print the opcode
                        let w: i32 = (opcode as i32) << SSEM_OPCODE_BIT_SHIFT;

                        // Print the operand
                        store.words[usize::try_from(index).unwrap()] = w | (operand as i32);
                    }
                }
            }
        }

        store._check();

        store
    }

    /// Initializes the store with the given snp file
    ///
    /// A snp file has the following form:
    /// ```
    /// 0000: 10000000000000000000000000000000
    /// 0001: 01010000000000100000000000000000 ; Some comment
    /// 0002: 00000000000000010000000000000000
    /// ...
    /// ```
    /// Each numbered line represents a raw word. Erverything after ';' is ignored.
    ///
    /// # Arguments
    ///
    /// * `filename` - Path to the file to read
    pub fn from_snp_file(filename: &Path) -> Store {
        // Todo return a Result for a richer explaination on the possible issues
        let file = File::open(filename);
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                panic!(
                    "Error while reading '{}': {}",
                    filename.to_str().unwrap_or(""),
                    e
                );
            }
        };
        let reader = BufReader::new(file);

        let mut store = Store {
            words: vec![0_i32; usize::try_from(SSEM_STORE_WORDS).unwrap()],
            size: SSEM_STORE_WORDS,
        };

        let mut last_index: i32 = 0;

        for (_, line) in reader.lines().enumerate() {
            let line = line.unwrap_or_else(|e| {
                panic!(
                    "Error while reading '{}': {}",
                    filename.to_str().unwrap_or(""),
                    e
                );
            });

            // Ignoring comments
            let exploded_line: Vec<&str> = line.splitn(2, ASM_COMMENT_CHAR).collect();
            if exploded_line.len() > 0 {
                let instruction = exploded_line[0].clone().trim();
                if instruction.len() == 0 {
                    continue;
                }
                // Extracting tokens "<index>: <binary_word>"
                let i: Vec<&str> = instruction.splitn(2, ':').collect();
                if i.len() != 2 {
                    panic!("Error near line '{}': invalid syntax", instruction);
                }

                let index: i32 = i[0].parse().expect("Unable to read the number");

                // Ensure lines are contiguous and without duplicates
                if index > 0 && index != last_index + 1 {
                    panic!(
                        "Error near line '{}': expected index '{}'",
                        instruction,
                        last_index + 1
                    )
                }
                last_index = index;

                // Ensure we don't write outside of the store
                if index >= store.size {
                    panic!(
                        "Error near line '{}': Index '{}' is bigger than the machine size ({})",
                        instruction, index, store.size,
                    );
                }

                // Reverse the bit order: SSEM is least significant bit first
                let word = i[1].trim().chars().rev().collect::<String>();
                if word.len() != 32 {
                    panic!(
                        "Error near line '{}': Invalid word size, expected 32, got {}",
                        line,
                        word.len()
                    );
                }
                // Use a 64-bit temporarily before casting down later below, to avoid parsing errors on some cases
                let word: i64 = match i64::from_str_radix(&word, 2) {
                    Ok(value) => value,
                    Err(e) => panic!(
                        "Error near line '{}': Unable to parse the word '{}': {}",
                        instruction, word, e
                    ),
                };

                // The cast to i32 is safe because we ensured before there is only 32 bits of data in the word
                store.words[usize::try_from(index).unwrap()] = word as i32;
            }
        }

        store._check();

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
        let opcode = (SSEM_OPCODE_MASK & (word >> SSEM_OPCODE_BIT_SHIFT)) as u8;

        Ok((Opcode::from(opcode), data))
    }

    fn _check(&self) {
        // Sanity checks
        if SSEM_DATA_MASK >= SSEM_STORE_WORDS {
            panic!(
                "Invalid configuration: addressing size ({}) is bigger than store size ({})",
                SSEM_DATA_MASK + 1,
                SSEM_STORE_WORDS
            )
        }
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

impl From<Vec<String>> for Store {
    fn from(value: Vec<String>) -> Self {
        if value.len() != SSEM_STORE_WORDS as usize {
            panic!(
                "Invalid store size. Expected {SSEM_STORE_WORDS}, got {}",
                value.len()
            );
        }

        let mut words: Vec<i32> = vec![0; SSEM_STORE_WORDS as usize];
        for (index, w) in value.iter().enumerate() {
            if w.len() != 32 {
                panic!("Invalid word size '{}'", w);
            }
            // Reverse bit order to match modern representation
            let w = w.chars().rev().collect::<String>();
            // Use a 64-bit temporarily before casting down later below, to avoid parsing errors on some cases
            let parsed_word: i64 = match i64::from_str_radix(&w, 2) {
                Ok(value) => value,
                Err(e) => panic!(
                    "Unable to parse the word '{}' for address {}: {}",
                    w, index, e
                ),
            };
            words[index] = parsed_word as i32;
        }

        let store = Store {
            words: words,
            size: SSEM_STORE_WORDS,
        };
        store._check();
        store
    }
}

impl PartialEq for Store {
    fn eq(&self, other: &Self) -> bool {
        self.words == other.words && self.size == other.size
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

#[cfg(test)]
mod tests {
    use crate::ssem::opcode::Opcode;

    use super::Store;

    #[test]
    fn decode_instruction() {
        let store = Store::from(vec![
            "10000111111110001111111111111111".into(), // 00
            "01000111111111001111111111111111".into(), // 01
            "11000111111110101111111111111111".into(), // 02
            "00100111111111101111111111111111".into(), // 03
            "10100111111110011111111111111111".into(), // 04
            "01100111111111011111111111111111".into(), // 05
            "11100111111110111111111111111111".into(), // 06
            "00010111111111111111111111111111".into(), // 07
            "00000000000000000000000000000000".into(), // 08
            "00000000000000000000000000000000".into(), // 09
            "00000000000000000000000000000000".into(), // 10
            "00000000000000000000000000000000".into(), // 11
            "00000000000000000000000000000000".into(), // 12
            "00000000000000000000000000000000".into(), // 13
            "00000000000000000000000000000000".into(), // 14
            "00000000000000000000000000000000".into(), // 15
            "00000000000000000000000000000000".into(), // 16
            "00000000000000000000000000000000".into(), // 17
            "00000000000000000000000000000000".into(), // 18
            "00000000000000000000000000000000".into(), // 19
            "00000000000000000000000000000000".into(), // 20
            "00000000000000000000000000000000".into(), // 21
            "00000000000000000000000000000000".into(), // 22
            "00000000000000000000000000000000".into(), // 23
            "00000000000000000000000000000000".into(), // 24
            "00000000000000000000000000000000".into(), // 25
            "00000000000000000000000000000000".into(), // 26
            "00000000000000000000000000000000".into(), // 27
            "00000000000000000000000000000000".into(), // 28
            "00000000000000000000000000000000".into(), // 29
            "00000000000000000000000000000000".into(), // 30
            "00000000000000000000000000000000".into(), // 31
        ]);

        let expected_outputs = [
            (0, Opcode::JMP, 1),
            (1, Opcode::JRP, 2),
            (2, Opcode::LDN, 3),
            (3, Opcode::STO, 4),
            (4, Opcode::SUB, 5),
            (5, Opcode::SUB2, 6),
            (6, Opcode::CMP, 7),
            (7, Opcode::STP, 8),
        ];

        for (address, exp_opcode, exp_data) in expected_outputs.iter() {
            let (opcode, data) = store.decode_instruction(*address).unwrap();
            assert_eq!(*exp_opcode, opcode);
            assert_eq!(*exp_data, data);
        }
    }

    #[test]
    #[should_panic]
    fn decode_instruction_error() {
        let store = Store::new();
        let _ = store.decode_instruction(32);
    }

    #[test]
    fn eq() {
        // default stores
        let s1 = Store::new();
        let s2 = Store::new();
        assert!(s1 == s2);

        // with data
        let s1 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ]);
        let s2 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ]);
        assert!(s1 == s2);

        // One word difference
        let s1 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000000".into(), // 31
        ]);
        let s2 = Store::from(vec![
            "00000000000000000000000000000000".into(), // 00
            "10000000000000000000000000000000".into(), // 01
            "01000000000000000000000000000000".into(), // 02
            "11000000000000000000000000000000".into(), // 03
            "00100000000000000000000000000000".into(), // 04
            "10100000000000000000000000000000".into(), // 05
            "01100000000000000000000000000000".into(), // 06
            "11100000000000000000000000000000".into(), // 07
            "00010000000000000000000000000000".into(), // 08
            "10010000000000000000000000000000".into(), // 09
            "01010000000000000000000000000000".into(), // 10
            "11010000000000000000000000000000".into(), // 11
            "00110000000000000000000000000000".into(), // 12
            "10110000000000000000000000000000".into(), // 13
            "01110000000000000000000000000000".into(), // 14
            "11110000000000000000000000000000".into(), // 15
            "00001000000000000000000000000000".into(), // 16
            "10001000000000000000000000000000".into(), // 17
            "01001000000000000000000000000000".into(), // 18
            "11001000000000000000000000000000".into(), // 19
            "00101000000000000000000000000000".into(), // 20
            "10101000000000000000000000000000".into(), // 21
            "01101000000000000000000000000000".into(), // 22
            "11101000000000000000000000000000".into(), // 23
            "00011000000000000000000000000000".into(), // 24
            "10011000000000000000000000000000".into(), // 25
            "01011000000000000000000000000000".into(), // 26
            "11011000000000000000000000000000".into(), // 27
            "00111000000000000000000000000000".into(), // 28
            "10111000000000000000000000000000".into(), // 29
            "01111000000000000000000000000000".into(), // 30
            "11111000000000000000000000000001".into(), // 31 difference
        ]);
        assert!(s1 != s2);
    }
}
