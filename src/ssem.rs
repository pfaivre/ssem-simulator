//! A simulator for the Small-Scale Experimental Machine

pub mod history;
mod opcode;
pub mod simulator;
mod store;

pub use opcode::Opcode;
pub use store::Store;

#[cfg(test)]
mod tests;
