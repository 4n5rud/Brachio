//! Brachio core: platform-independent domain logic.
//!
//! Side effects (processes, files, network) live in the adapter crates.
//! Everything here is pure and tested with injected clocks.

pub mod status;
pub mod task_graph;
