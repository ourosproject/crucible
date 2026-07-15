//! The five simplicity primitives (spec §4). Each analyzer is a pure
//! `fn(&syn::Block) -> u32`, independently tested against known-count fixtures.

mod branching;
pub use branching::branching;
