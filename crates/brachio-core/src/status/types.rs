use serde::{Deserialize, Serialize};

/// Why an agent is waiting for a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    Permission,
    Question,
    Auth,
}

/// What an agent session is doing, as shown on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum AgentState {
    Working,
    Blocked { reason: BlockReason },
    Idle,
    Error,
    Exited { code: Option<i32> },
    Unknown,
}

impl AgentState {
    /// States in which a person has to act, so a notification is warranted.
    #[must_use]
    pub fn needs_person(self) -> bool {
        matches!(self, Self::Blocked { .. } | Self::Error)
    }
}

/// Where an observation came from. Order is not priority; see `resolve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Process,
    Hook,
    Osc7501,
    Activity,
}

/// One status report from one source, stamped with a UTC millisecond clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observation {
    pub state: AgentState,
    pub at_ms: i64,
}

/// The latest observation per source for one session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Observations {
    pub process: Option<Observation>,
    pub hook: Option<Observation>,
    pub osc: Option<Observation>,
    /// Time of the last PTY output byte, if any.
    pub last_output_ms: Option<i64>,
}

impl Observations {
    /// Records an observation, keeping only the newest one per source.
    pub fn record(&mut self, source: Source, obs: Observation) {
        let slot = match source {
            Source::Process => &mut self.process,
            Source::Hook => &mut self.hook,
            Source::Osc7501 => &mut self.osc,
            Source::Activity => {
                self.last_output_ms = Some(obs.at_ms.max(self.last_output_ms.unwrap_or(i64::MIN)));
                return;
            }
        };
        if slot.is_none_or(|old| old.at_ms <= obs.at_ms) {
            *slot = Some(obs);
        }
    }
}
