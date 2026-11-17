//! Agent status model and the rule that merges status signals.
//!
//! Signal precedence: process > hook > OSC 7501 > output activity.

mod debounce;
mod resolve;
mod types;

pub use debounce::Debouncer;
pub use resolve::{resolve, ResolveConfig};
pub use types::{AgentState, BlockReason, Observation, Observations, Source};
