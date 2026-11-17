use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Identifies "the same failure": test id plus the first line of its error.
#[must_use]
pub fn failure_signature(test_id: &str, error: &str) -> u64 {
    let mut h = DefaultHasher::new();
    test_id.hash(&mut h);
    error.lines().next().unwrap_or("").trim().hash(&mut h);
    h.finish()
}

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
    #[must_use]
    pub fn new(max_attempts: u32) -> Self {
        Self {
            max_attempts,
            attempts: 0,
            last_signature: None,
            repeats: 0,
        }
    }

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
        let a = failure_signature("login.spec:3", "Expected 200\n at a.ts:1");
        let b = failure_signature("login.spec:3", "Expected 200\n at b.ts:9");
        assert_eq!(a, b);
    }

    #[test]
    fn same_failure_twice_stops() {
        let mut g = FeedbackGate::new(3);
        assert_eq!(g.decide(1), FeedbackDecision::Send);
        assert_eq!(g.decide(1), FeedbackDecision::StopNoProgress);
    }

    #[test]
    fn different_failures_stop_at_limit() {
        let mut g = FeedbackGate::new(3);
        assert_eq!(g.decide(1), FeedbackDecision::Send);
        assert_eq!(g.decide(2), FeedbackDecision::Send);
        assert_eq!(g.decide(3), FeedbackDecision::Send);
        assert_eq!(g.decide(4), FeedbackDecision::StopLimit);
    }
}
