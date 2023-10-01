use rustc_hash::FxHashMap;
use std::fmt;
use std::str::FromStr;

/// Represents an operation code for the SSEM
///
/// Its values gives the opcode bits, except for NUM
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

lazy_static! {
    pub static ref SSEM_OPCODE_TABLE: FxHashMap<u8, Opcode> = {
        let mut m = FxHashMap::default();
        m.insert(0b000, Opcode::JMP);
        m.insert(0b001, Opcode::JRP);
        m.insert(0b010, Opcode::LDN);
        m.insert(0b011, Opcode::STO);
        m.insert(0b100, Opcode::SUB);
        m.insert(0b101, Opcode::SUB2);
        m.insert(0b110, Opcode::CMP);
        m.insert(0b111, Opcode::STP);
        m
    };
}

impl From<u8> for Opcode {
    // Using a static FxHashMap instead of a match has been observed to provide about 10% performance gains
    fn from(input: u8) -> Opcode {
        SSEM_OPCODE_TABLE[&input]
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

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Opcode;

    /// Ensure consistency during conversion
    #[test]
    fn from_u8() {
        assert_eq!(Opcode::from(Opcode::JMP as u8), Opcode::JMP);
        assert_eq!(Opcode::from(Opcode::JRP as u8), Opcode::JRP);
        assert_eq!(Opcode::from(Opcode::LDN as u8), Opcode::LDN);
        assert_eq!(Opcode::from(Opcode::STO as u8), Opcode::STO);
        assert_eq!(Opcode::from(Opcode::SUB as u8), Opcode::SUB);
        assert_eq!(Opcode::from(Opcode::SUB2 as u8), Opcode::SUB2);
        assert_eq!(Opcode::from(Opcode::CMP as u8), Opcode::CMP);
        assert_eq!(Opcode::from(Opcode::STP as u8), Opcode::STP);
    }

    #[test]
    #[should_panic]
    fn from_u8_error() {
        let _ = Opcode::from(0b1000);
    }

    /// Ensure consistency during string conversion
    #[test]
    fn from_str() {
        assert_eq!(Opcode::from_str("JMP").unwrap().to_string(), "JMP");
        assert_eq!(Opcode::from_str("JRP").unwrap().to_string(), "JRP");
        assert_eq!(Opcode::from_str("LDN").unwrap().to_string(), "LDN");
        assert_eq!(Opcode::from_str("STO").unwrap().to_string(), "STO");
        assert_eq!(Opcode::from_str("SUB").unwrap().to_string(), "SUB");
        assert_eq!(Opcode::from_str("SUB2").unwrap().to_string(), "SUB2");
        assert_eq!(Opcode::from_str("CMP").unwrap().to_string(), "CMP");
        assert_eq!(Opcode::from_str("STP").unwrap().to_string(), "STP");
    }
}
