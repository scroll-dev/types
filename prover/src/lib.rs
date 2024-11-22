// re-export the whole prover in zkevm-circuit

mod scroll_sp1;
#[path = "utils.rs"]
mod prover_utils;

pub use prover::*;
pub use scroll_sp1::Sp1Prover;
pub use prover_utils::load_elf;
