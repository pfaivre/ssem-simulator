//! A simulator for the Small-Scale Experimental Machine

pub mod history;
mod opcode;
pub mod simulator;
mod store;

pub use opcode::Opcode;
pub use store::Store;

/// Whether `path` names a loadable program/snapshot file — the extension
/// check (`.asm`/`.snp`, case-insensitive) behind [`store::FileFormat`],
/// kept private to this module.
pub(crate) fn is_loadable_file(path: &std::path::Path) -> bool {
    store::FileFormat::from_path(path).is_ok()
}

#[cfg(test)]
mod tests;
