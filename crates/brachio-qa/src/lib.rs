//! Executes explicitly approved project checks and limits automated QA feedback.

mod feedback_gate;
mod gated_run;
mod runner;

pub use feedback_gate::{failure_signature, FeedbackDecision, FeedbackGate};
pub use gated_run::{
    read_command, review_text, run_after_approval, GatedOutcome, QaOutcome, QaReport,
};
pub use runner::{run_approved, QaApproval, QaCommand, QaExit, QaNotRun, QaOutput, VerifiedQaRun};
