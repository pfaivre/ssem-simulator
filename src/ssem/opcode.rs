use std::fmt;
use std::str::FromStr;

/// Represents an operation code for the SSEM
///
/// Its values gives the opcode bits, except for NUM
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

// TODO: Find a way to avoid all those explicit conversions
impl From<i32> for Opcode {
    fn from(input: i32) -> Opcode {
        match input {
            0b000 => Opcode::JMP,
            0b001 => Opcode::JRP,
            0b010 => Opcode::LDN,
            0b011 => Opcode::STO,
            0b100 => Opcode::SUB,
            0b101 => Opcode::SUB2,
            0b110 => Opcode::CMP,
            0b111 => Opcode::STP,
            _ => panic!("Unexpected opcode value"),
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
            "SUB" | "SUB2" => Ok(Opcode::SUB),
            "CMP" => Ok(Opcode::CMP),
            "STP" => Ok(Opcode::STP),
            "NUM" => Ok(Opcode::NUM),
            _ => Err(()),
        }
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Opcode::JMP => write!(f, "JMP"),
            Opcode::JRP => write!(f, "JRP"),
            Opcode::LDN => write!(f, "LDN"),
            Opcode::STO => write!(f, "STO"),
            Opcode::SUB | Opcode::SUB2 => write!(f, "SUB"),
            Opcode::CMP => write!(f, "CMP"),
            Opcode::STP => write!(f, "STP"),
            Opcode::NUM => write!(f, "NUM"),
        }
    }
}
