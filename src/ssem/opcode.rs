use std::fmt;
use std::str::FromStr;

/// Represents an operation code for the SSEM
///
/// Its values gives the opcode bits, except for NUM
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Opcode {
    /// Indirect jump
    JMP = 0b000,

    /// Relative jump
    JRP = 0b001,

    /// Load negative of value from given address to accumulator
    LDN = 0b010,

    /// Store accumulator in given address
    STO = 0b011,

    /// Substract value in given address from accumulator
    SUB = 0b100,

    /// Should not be used. Same effect as SUB
    SUB2 = 0b101,

    /// Skip next instruction if accumulator is negative
    CMP = 0b110,

    /// Halt the program
    STP = 0b111,

    /// Not an instruction. Mnemonic used to set a raw number to the store
    NUM,
}

impl Opcode {
    /// All real opcodes, in bit order (0b000 to 0b111).
    ///
    /// NUM is a pseudo-opcode (a raw data word carries no opcode bits), so
    /// it is not listed: `Store::decode_word` never yields it.
    pub const ALL: [Opcode; 8] = [
        Opcode::JMP,
        Opcode::JRP,
        Opcode::LDN,
        Opcode::STO,
        Opcode::SUB,
        Opcode::SUB2,
        Opcode::CMP,
        Opcode::STP,
    ];

    /// Whether the instruction's data field is a store address.
    ///
    /// CMP and STP ignore their data field; NUM is not an instruction.
    pub fn uses_address(&self) -> bool {
        matches!(
            self,
            Opcode::JMP | Opcode::JRP | Opcode::LDN | Opcode::STO | Opcode::SUB | Opcode::SUB2
        )
    }
}

impl From<u8> for Opcode {
    // decode_instruction masks the opcode bits down to 3 before calling this,
    // so the match is a simple jump table on the hot path.
    // Out-of-range input still panics, like the previous table lookup did.
    fn from(input: u8) -> Opcode {
        match input {
            0b000 => Opcode::JMP,
            0b001 => Opcode::JRP,
            0b010 => Opcode::LDN,
            0b011 => Opcode::STO,
            0b100 => Opcode::SUB,
            0b101 => Opcode::SUB2,
            0b110 => Opcode::CMP,
            0b111 => Opcode::STP,
            _ => panic!("Unknown opcode {}", input),
        }
    }
}

impl FromStr for Opcode {
    type Err = ();

    fn from_str(input: &str) -> Result<Opcode, Self::Err> {
        match input {
            "JMP" => Ok(Opcode::JMP),
            "JRP" => Ok(Opcode::JRP),
            "LDN" => Ok(Opcode::LDN),
            "STO" => Ok(Opcode::STO),
            "SUB" => Ok(Opcode::SUB),
            "SUB2" => Ok(Opcode::SUB2),
            "CMP" => Ok(Opcode::CMP),
            "STP" => Ok(Opcode::STP),
            "NUM" => Ok(Opcode::NUM),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Opcode {
    /// Prints the mnemonic of the opcode ("LDN", "JMP", "STO", etc.)
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
