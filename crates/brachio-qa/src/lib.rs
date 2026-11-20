//! Executes explicitly approved project checks and limits automated QA feedback.

mod feedback_gate;
mod runner;

pub use feedback_gate::{failure_signature, FeedbackDecision, FeedbackGate};
pub use runner::{run_approved, QaApproval, QaCommand, QaExit, QaNotRun, QaOutput, VerifiedQaRun};
