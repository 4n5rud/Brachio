use super::types::{AgentState, Observation, Observations};

/// Tunables for `resolve`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolveConfig {
    /// A hook that reported working or blocked and then went quiet this long
    /// yields to a newer OSC 7501 report.
    pub hook_stale_ms: i64,
    /// Output within this window counts as working when nothing else speaks.
    pub activity_window_ms: i64,
}

impl Default for ResolveConfig {
    fn default() -> Self {
        Self {
            hook_stale_ms: 120_000,
            activity_window_ms: 3_000,
        }
    }
}

/// Merges the per-source observations into one state.
///
/// Rules, in order:
/// 1. A process exit wins.
/// 2. A hook wins, unless it is stale and OSC 7501 reported later.
///    `Idle` from a hook never goes stale: a finished turn is silent by design.
/// 3. OSC 7501 wins when there is no hook.
/// 4. Otherwise recent output means working; silence alone proves nothing.
#[must_use]
pub fn resolve(obs: &Observations, now_ms: i64, cfg: ResolveConfig) -> AgentState {
    if let Some(p) = obs.process {
        if matches!(p.state, AgentState::Exited { .. }) {
            return p.state;
        }
    }
    match (obs.hook, obs.osc) {
        (Some(hook), Some(osc)) if hook_is_stale(hook, now_ms, cfg) && osc.at_ms > hook.at_ms => {
            osc.state
        }
        (Some(hook), _) => hook.state,
        (None, Some(osc)) => osc.state,
        (None, None) => from_activity(obs.last_output_ms, now_ms, cfg),
    }
}

fn hook_is_stale(hook: Observation, now_ms: i64, cfg: ResolveConfig) -> bool {
    let can_go_stale = matches!(hook.state, AgentState::Working | AgentState::Blocked { .. });
    can_go_stale && now_ms - hook.at_ms >= cfg.hook_stale_ms
}

fn from_activity(last_output_ms: Option<i64>, now_ms: i64, cfg: ResolveConfig) -> AgentState {
    match last_output_ms {
        Some(t) if now_ms - t < cfg.activity_window_ms => AgentState::Working,
        _ => AgentState::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{BlockReason, Source};

    const CFG: ResolveConfig = ResolveConfig {
        hook_stale_ms: 120_000,
        activity_window_ms: 3_000,
    };

    fn at(state: AgentState, at_ms: i64) -> Observation {
        Observation { state, at_ms }
    }

    #[test]
    fn exit_beats_every_other_signal() {
        let mut o = Observations::default();
        o.record(Source::Hook, at(AgentState::Working, 10));
        o.record(Source::Process, at(AgentState::Exited { code: Some(1) }, 5));
        assert_eq!(resolve(&o, 20, CFG), AgentState::Exited { code: Some(1) });
    }

    #[test]
    fn fresh_hook_beats_newer_osc() {
        let mut o = Observations::default();
        o.record(Source::Hook, at(AgentState::Working, 1_000));
        o.record(Source::Osc7501, at(AgentState::Idle, 2_000));
        assert_eq!(resolve(&o, 3_000, CFG), AgentState::Working);
    }

    #[test]
    fn stale_working_hook_yields_to_newer_osc() {
        let mut o = Observations::default();
        o.record(Source::Hook, at(AgentState::Working, 0));
        o.record(Source::Osc7501, at(AgentState::Idle, 60_000));
        assert_eq!(resolve(&o, 120_000, CFG), AgentState::Idle);
    }

    #[test]
    fn stale_hook_keeps_winning_over_older_osc() {
        let mut o = Observations::default();
        o.record(Source::Osc7501, at(AgentState::Idle, 0));
        o.record(Source::Hook, at(AgentState::Working, 10));
        assert_eq!(resolve(&o, 500_000, CFG), AgentState::Working);
    }

    #[test]
    fn idle_hook_never_goes_stale() {
        let mut o = Observations::default();
        o.record(Source::Hook, at(AgentState::Idle, 0));
        let blocked = AgentState::Blocked {
            reason: BlockReason::Permission,
        };
        o.record(Source::Osc7501, at(blocked, 200_000));
        assert_eq!(resolve(&o, 400_000, CFG), AgentState::Idle);
    }

    #[test]
    fn osc_alone_is_used() {
        let mut o = Observations::default();
        let blocked = AgentState::Blocked {
            reason: BlockReason::Question,
        };
        o.record(Source::Osc7501, at(blocked, 5));
        assert_eq!(resolve(&o, 10, CFG), blocked);
    }

    #[test]
    fn recent_output_alone_means_working() {
        let mut o = Observations::default();
        o.record(Source::Activity, at(AgentState::Unknown, 9_000));
        assert_eq!(resolve(&o, 10_000, CFG), AgentState::Working);
    }

    #[test]
    fn silence_alone_is_unknown_not_idle() {
        let mut o = Observations::default();
        o.record(Source::Activity, at(AgentState::Unknown, 0));
        assert_eq!(resolve(&o, 3_000, CFG), AgentState::Unknown);
        assert_eq!(
            resolve(&Observations::default(), 0, CFG),
            AgentState::Unknown
        );
    }

    #[test]
    fn older_observation_does_not_replace_newer() {
        let mut o = Observations::default();
        o.record(Source::Hook, at(AgentState::Idle, 100));
        o.record(Source::Hook, at(AgentState::Working, 50));
        assert_eq!(resolve(&o, 200, CFG), AgentState::Idle);
    }
}
