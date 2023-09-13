use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::str::FromStr;

use crate::ssem::opcode::Opcode;

const ASM_COMMENT_CHAR: char = ';';
const SSEM_STORE_WORDS: i32 = 32;

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
        let file = File::open(filename);
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error while reading '{}': {}", filename, e);
                std::process::exit(1);
            },
        };
        let reader = BufReader::new(file);

        let mut store = Store { 
            words: vec![0_i32; usize::try_from(SSEM_STORE_WORDS).unwrap()],
            size: SSEM_STORE_WORDS,
        };

        for (_, line) in reader.lines().enumerate() {

            let line = line.unwrap_or_else(|e| {
                eprintln!("Error while reading '{}': {}", filename, e);
                std::process::exit(1);
            });

            // Ignoring comments
            if let Some((instruction, _)) = line.split_once(ASM_COMMENT_CHAR) {
                if instruction.len() > 0 {
                    // Extracting tokens "<index> <opcode> <operand>"
                    let index: i32;
                    let opcode: &str;
                    let operand: i32;
                    let i: Vec<&str> = instruction.trim().split(' ').collect();
                    if i.len() < 2 {
                        eprintln!("Unable to read the input file: invalid instruction '{}'", instruction);
                        std::process::exit(1);
                    }
                    index = i[0].parse().expect("Unable to read the number");
                    opcode = i[1];
                    if i.len() >= 3 {
                        operand = i[2].parse().expect("Unable to read the number");
                    }
                    else {
                        operand = 0;
                    }

                    if index >= store.size {
                        eprintln!("Unable to read the input file: invalid instruction '{}'", instruction);
                        eprintln!("Index '{}' is bigger than the machine size ({})", index, store.size);
                        std::process::exit(1);
                    }

                    let opcode = match Opcode::from_str(opcode) {
                        Ok(code) => code,
                        Err(_) => {
                            eprintln!("Error while reading '{}': opcode '{}' non valid.", filename, opcode);
                            std::process::exit(1);
                        }
                    };

                    match opcode {
                        Opcode::NUM => {
                            // TODO: make this safe
                            store.words[usize::try_from(index).unwrap()] = operand;
                        }
                        opcode => {
                            let mut w = store.words[usize::try_from(index).unwrap()]; // TODO: make this safe

                            // Print the opcode
                            w = w | ((opcode as i32) << 13);

                            // Print the operand
                            store.words[usize::try_from(index).unwrap()] = w | ((operand as i32));
                        }
                    }
                }
            }
        }

        store
    }

    pub fn decode_instruction(&self, index: i32) -> Result<(Opcode, i32), String> {
        let index = match usize::try_from(index) {
            Ok(value) => value,
            Err(_) => return Err("Unable to get index from number".into()),
        };
        let word: i32 = match self.words.get(index) {
            Some(value) => *value,
            None => return Err("Out of bound".into()), 
        };

        // TODO: actually decode

        Ok((Opcode::LDN, 0))
    }
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for word in self.words.iter() {
            write!(f, " {:032b}\n", word.reverse_bits()).ok();
        }
        Ok(())
    }
}
