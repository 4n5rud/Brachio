//! Agent states and the observations they are resolved from.

use serde::{Deserialize, Serialize};

/// Why an agent is waiting for a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockReason {
    /// A tool call waits for approval.
    Permission,
    /// The agent asked the person a question.
    Question,
    /// The CLI needs the person to sign in again.
    Auth,
    /// The agent is blocked but did not say why.
    Other,
}

/// What an agent session is doing, as shown on the board.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum AgentState {
    /// The agent is running a turn.
    Working,
    /// The agent cannot continue until a person acts.
    Blocked {
        /// What the agent is waiting for.
        reason: BlockReason,
    },
    /// The turn finished and the agent waits for the next prompt.
    Idle,
    /// The agent reported an error.
    Error,
    /// The process ended.
    Exited {
        /// Exit code, when the platform reported one.
        code: Option<i32>,
    },
    /// No signal says anything yet.
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
    /// The PTY child process itself.
    Process,
    /// A managed agent hook.
    Hook,
    /// The OSC 7501 program status sequence.
    Osc7501,
    /// Output activity on the PTY.
    Activity,
}

/// One status report from one source, stamped with a UTC millisecond clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Observation {
    /// The state the source reported.
    pub state: AgentState,
    /// When the source reported it.
    pub at_ms: i64,
}

/// The latest observation per source for one session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Observations {
    /// Latest report from the process.
    pub process: Option<Observation>,
    /// Latest report from a hook.
    pub hook: Option<Observation>,
    /// Latest OSC 7501 report.
    pub osc: Option<Observation>,
    /// Time of the last PTY output byte, if any.
    pub last_output_ms: Option<i64>,
}

impl Observations {
    /// Records an observation, keeping only the newest one per source.
    pub fn record(&mut self, source: Source, observation: Observation) {
        let slot = match source {
            Source::Process => &mut self.process,
            Source::Hook => &mut self.hook,
            Source::Osc7501 => &mut self.osc,
            Source::Activity => {
                self.last_output_ms = Some(
                    observation
                        .at_ms
                        .max(self.last_output_ms.unwrap_or(i64::MIN)),
                );
                return;
            }
        };
        if slot.is_none_or(|old| old.at_ms <= observation.at_ms) {
            *slot = Some(observation);
        }
    }
}
