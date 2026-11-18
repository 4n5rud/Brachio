//! Agent status model and the rule that merges status signals.
//!
//! Signal precedence: process > hook > OSC 7501 > output activity.

mod agent_state;
mod debouncer;
mod osc_scanner;
mod program_status;
mod resolve;

pub use agent_state::{AgentState, BlockReason, Observation, Observations, Source};
pub use debouncer::Debouncer;
pub use osc_scanner::OscScanner;
pub use program_status::{ProgramStatus, ProgramStatusMessage, QUERY_REPLY};
pub use resolve::{resolve, ResolveConfig};
