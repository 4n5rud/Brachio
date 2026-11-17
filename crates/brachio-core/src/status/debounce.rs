use super::types::AgentState;

/// Publishes a state only after it has held for `stable_ms`.
///
/// Stops the board and notifications from flickering between working and idle
/// while an agent streams output between tool calls.
#[derive(Debug, Clone)]
pub struct Debouncer {
    stable_ms: i64,
    published: AgentState,
    pending: Option<(AgentState, i64)>,
}

impl Debouncer {
    #[must_use]
    pub fn new(stable_ms: i64) -> Self {
        Self {
            stable_ms,
            published: AgentState::Unknown,
            pending: None,
        }
    }

    #[must_use]
    pub fn published(&self) -> AgentState {
        self.published
    }

    /// Feeds the latest resolved state. Returns `Some(state)` when the
    /// published state changes.
    pub fn update(&mut self, candidate: AgentState, now_ms: i64) -> Option<AgentState> {
        if candidate == self.published {
            self.pending = None;
            return None;
        }
        // An exit is final; publish it at once.
        if matches!(candidate, AgentState::Exited { .. }) {
            return self.publish(candidate);
        }
        match self.pending {
            Some((state, since)) if state == candidate => (now_ms - since >= self.stable_ms)
                .then(|| self.publish(candidate))
                .flatten(),
            _ => {
                self.pending = Some((candidate, now_ms));
                None
            }
        }
    }

    fn publish(&mut self, state: AgentState) -> Option<AgentState> {
        self.published = state;
        self.pending = None;
        Some(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_only_after_state_holds() {
        let mut d = Debouncer::new(300);
        assert_eq!(d.update(AgentState::Working, 0), None);
        assert_eq!(d.update(AgentState::Working, 299), None);
        assert_eq!(
            d.update(AgentState::Working, 300),
            Some(AgentState::Working)
        );
        assert_eq!(d.published(), AgentState::Working);
    }

    #[test]
    fn flicker_shorter_than_window_is_ignored() {
        let mut d = Debouncer::new(300);
        d.update(AgentState::Working, 0);
        d.update(AgentState::Working, 300);
        assert_eq!(d.update(AgentState::Idle, 400), None);
        assert_eq!(d.update(AgentState::Working, 500), None);
        assert_eq!(d.update(AgentState::Idle, 600), None);
        assert_eq!(d.published(), AgentState::Working);
    }

    #[test]
    fn exit_is_published_immediately() {
        let mut d = Debouncer::new(300);
        let exited = AgentState::Exited { code: Some(0) };
        assert_eq!(d.update(exited, 0), Some(exited));
    }
}
