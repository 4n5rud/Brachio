//! QA loop logic. Running Playwright and CDP streaming arrive in M3; the
//! rules that decide whether to send a failure back to an agent live here
//! now because they are pure.

mod feedback;

pub use feedback::{failure_signature, FeedbackDecision, FeedbackGate};
