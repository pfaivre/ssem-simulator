use std::str::FromStr;

use crate::ssem::Opcode;

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

/// Instructions whose data field references a store line
#[test]
fn uses_address() {
    assert!(Opcode::JMP.uses_address());
    assert!(Opcode::JRP.uses_address());
    assert!(Opcode::LDN.uses_address());
    assert!(Opcode::STO.uses_address());
    assert!(Opcode::SUB.uses_address());
    assert!(Opcode::SUB2.uses_address());
    assert!(!Opcode::CMP.uses_address());
    assert!(!Opcode::STP.uses_address());
    assert!(!Opcode::NUM.uses_address());
}

/// All eight real opcodes, in bit order, round-tripping through u8
#[test]
fn all_in_bit_order() {
    assert_eq!(8, Opcode::ALL.len());
    for (i, opcode) in Opcode::ALL.iter().enumerate() {
        assert_eq!(*opcode, Opcode::from(i as u8));
        assert_eq!(i as u8, *opcode as u8);
    }
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
