//! The five simplicity primitives (spec §4). Each analyzer is a pure
//! `fn(&syn::Block) -> u32`, independently tested against known-count fixtures.

mod branching;
mod density;
mod depth;
mod size;
mod state;

pub use branching::branching;
pub use density::density;
pub use depth::depth;
pub use size::size;
pub use state::state;
