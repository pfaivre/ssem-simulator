use std::fmt;
use std::str::FromStr;

/// Represents an operation code for the SSEM
/// 
/// Its values gives the opcode bits, except for NUM
pub(crate) enum Opcode {
    JMP  = 0b000,
    JRP  = 0b001,
    LDN  = 0b010,
    STO  = 0b011,
    SUB  = 0b100,
    SUB2 = 0b101,
    CMP  = 0b110,
    STP  = 0b111,
    NUM,
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
       match self {
            Opcode::JMP => write!(f, "JMP"),
            Opcode::JRP => write!(f, "JRP"),
            Opcode::LDN => write!(f, "LDN"),
            Opcode::STO => write!(f, "STO"),
            Opcode::SUB => write!(f, "SUB"),
            Opcode::SUB2 => write!(f, "SUB2"),
            Opcode::CMP => write!(f, "CMP"),
            Opcode::STP => write!(f, "STP"),
            Opcode::NUM => write!(f, "NUM"),
       }
    }
}
