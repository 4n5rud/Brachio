//! Normalizes terminal observations before they reach the task board.

use std::time::{Duration, Instant};

use brachio_core::status::{
    resolve, AgentState, Debouncer, Observation, Observations, ResolveConfig, Source,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::watch;

use crate::terminal::{send_message, ChannelMessage};

const STABLE_MS: i64 = 300;
const TICK_MS: u64 = 100;

#[derive(Clone, Default)]
struct Snapshot {
    observations: Observations,
    osc_message: Option<String>,
}

pub(crate) struct TerminalStatus {
    started: Instant,
    state: watch::Sender<Snapshot>,
}

impl TerminalStatus {
    pub(crate) fn new(channel: Channel<InvokeResponseBody>) -> Self {
        let started = Instant::now();
        let (state, updates) = watch::channel(Snapshot::default());
        tauri::async_runtime::spawn(publish(updates, started, channel));
        Self { started, state }
    }

    pub(crate) fn activity(&self) {
        let now = elapsed_ms(self.started);
        self.state.send_if_modified(|state| {
            if state.observations.osc.is_some()
                || state
                    .observations
                    .last_output_ms
                    .is_some_and(|last| now - last < 50)
            {
                return false;
            }
            state.observations.last_output_ms = Some(now);
            true
        });
    }

    pub(crate) fn osc(&self, status: AgentState, message: Option<String>) {
        let now = elapsed_ms(self.started);
        self.state.send_modify(|state| {
            if status == AgentState::Unknown {
                state.observations.osc = None;
                state.osc_message = None;
            } else {
                state.observations.record(
                    Source::Osc7501,
                    Observation {
                        state: status,
                        at_ms: now,
                    },
                );
                state.osc_message = message;
            }
        });
    }
}

fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

async fn publish(
    mut updates: watch::Receiver<Snapshot>,
    started: Instant,
    channel: Channel<InvokeResponseBody>,
) {
    let config = ResolveConfig::default();
    let mut debounce = Debouncer::new(STABLE_MS);
    let mut last_message = None;
    loop {
        let snapshot = updates.borrow_and_update().clone();
        let now = elapsed_ms(started);
        let candidate = resolve(&snapshot.observations, now, config);
        let changed = debounce.update(candidate, now).is_some();
        if candidate == debounce.published() {
            let message = snapshot
                .observations
                .osc
                .filter(|osc| osc.state == candidate)
                .and(snapshot.osc_message);
            if changed || message != last_message {
                last_message.clone_from(&message);
                send_message(
                    &channel,
                    &ChannelMessage::Status {
                        status: candidate,
                        message,
                    },
                );
            }
        }
        let activity_expires = snapshot.observations.osc.is_none()
            && snapshot
                .observations
                .last_output_ms
                .is_some_and(|last| now - last < config.activity_window_ms);
        if activity_expires || candidate != debounce.published() {
            tokio::select! {
                changed = updates.changed() => if changed.is_err() { return; },
                () = tokio::time::sleep(Duration::from_millis(TICK_MS)) => {}
            }
        } else if updates.changed().await.is_err() {
            return;
        }
    }
}
