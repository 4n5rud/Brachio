//! Decides whether a QA failure goes back to the agent.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Identifies "the same failure": test id plus the first line of its error.
#[must_use]
pub fn failure_signature(test_id: &str, error: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    test_id.hash(&mut hasher);
    error.lines().next().unwrap_or("").trim().hash(&mut hasher);
    hasher.finish()
}

/// What to do with a new QA failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackDecision {
    /// Send the failure bundle to the agent.
    Send,
    /// Attempts used up for this task; ask a person.
    StopLimit,
    /// The same failure came back twice in a row; ask a person.
    StopNoProgress,
}

/// Limits automatic fix attempts per task so a QA loop cannot burn tokens.
#[derive(Debug, Clone)]
pub struct FeedbackGate {
    max_attempts: u32,
    attempts: u32,
    last_signature: Option<u64>,
    repeats: u32,
}

impl FeedbackGate {
    /// A gate that allows `max_attempts` automatic fixes.
    #[must_use]
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            attempts: 0,
            last_signature: None,
            repeats: 0,
        }
    }

    /// Records a failure and decides what to do with it.
    pub fn decide(&mut self, signature: u64) -> FeedbackDecision {
        self.repeats = if self.last_signature == Some(signature) {
            self.repeats + 1
        } else {
            1
        };
        self.last_signature = Some(signature);
        if self.repeats >= 2 && self.attempts > 0 {
            return FeedbackDecision::StopNoProgress;
        }
        if self.attempts >= self.max_attempts {
            return FeedbackDecision::StopLimit;
        }
        self.attempts += 1;
        FeedbackDecision::Send
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_ignores_stack_after_first_line() {
        let first = failure_signature("login.spec:3", "Expected 200\n at a.ts:1");
        let second = failure_signature("login.spec:3", "Expected 200\n at b.ts:9");
        assert_eq!(first, second);
    }

    #[test]
    fn same_failure_twice_stops() {
        let mut gate = FeedbackGate::new(3);
        assert_eq!(gate.decide(1), FeedbackDecision::Send);
        assert_eq!(gate.decide(1), FeedbackDecision::StopNoProgress);
    }

    #[test]
    fn different_failures_stop_at_limit() {
        let mut gate = FeedbackGate::new(3);
        assert_eq!(gate.decide(1), FeedbackDecision::Send);
        assert_eq!(gate.decide(2), FeedbackDecision::Send);
        assert_eq!(gate.decide(3), FeedbackDecision::Send);
        assert_eq!(gate.decide(4), FeedbackDecision::StopLimit);
    }
}
