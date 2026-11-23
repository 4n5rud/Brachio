//! Replays recorded terminal output through the real OSC scanner, the OSC 7501 parser,
//! `resolve` and the debouncer, and checks the published status timeline.
//!
//! Fixtures live in `tests/fixtures/agent-replay/status`; see the README there.

use std::fs;
use std::path::Path;

use brachio_core::status::{
    resolve, AgentState, Debouncer, Observation, Observations, OscScanner, ProgramStatusMessage,
    ResolveConfig, Source, QUERY_REPLY,
};
use serde::Deserialize;

/// The debounce window the app uses (`terminal_status.rs`).
const STABLE_MS: i64 = 300;

#[derive(Deserialize)]
struct Case {
    steps: Vec<Step>,
}

#[derive(Deserialize)]
struct Exit {
    code: Option<i32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    at_ms: i64,
    #[serde(default)]
    output: Option<String>,
    #[serde(default)]
    output_hex: Option<String>,
    #[serde(default)]
    exit: Option<Exit>,
    #[serde(default)]
    expect: Option<AgentState>,
    #[serde(default)]
    expect_replies: Option<usize>,
    #[serde(default, deserialize_with = "present")]
    expect_message: Option<Option<String>>,
    #[serde(default)]
    note: Option<String>,
}

/// Tells `"expect_message": null` (no message) apart from a missing key.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

fn decode_hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex byte"))
        .collect()
}

/// Mirrors how `terminal_status.rs` folds PTY events into observations.
#[derive(Default)]
struct Replay {
    observations: Observations,
    message: Option<String>,
    scanner: OscScanner,
    debounce: Option<Debouncer>,
    replies: usize,
}

impl Replay {
    fn feed(&mut self, bytes: &[u8], now: i64) {
        let Self {
            observations,
            message,
            scanner,
            replies,
            ..
        } = self;
        if observations.osc.is_none() {
            observations.last_output_ms = Some(now);
        }
        scanner.feed(bytes, |body| match ProgramStatusMessage::parse(body) {
            Some(ProgramStatusMessage::Query) => {
                assert!(QUERY_REPLY.starts_with(b"\x1b]7501;?"));
                *replies += 1;
            }
            Some(ProgramStatusMessage::Report(status)) => {
                if status.state == AgentState::Unknown {
                    observations.osc = None;
                    *message = None;
                } else {
                    observations.record(
                        Source::Osc7501,
                        Observation {
                            state: status.state,
                            at_ms: now,
                        },
                    );
                    *message = status.message;
                }
            }
            None => {}
        });
    }

    fn tick(&mut self, now: i64) -> AgentState {
        let candidate = resolve(&self.observations, now, ResolveConfig::default());
        let debounce = self
            .debounce
            .get_or_insert_with(|| Debouncer::new(STABLE_MS));
        debounce.update(candidate, now);
        debounce.published()
    }

    /// The message belongs to the published state only while OSC speaks for it.
    fn published_message(&self, published: AgentState) -> Option<String> {
        self.observations
            .osc
            .filter(|osc| osc.state == published)
            .and(self.message.clone())
    }
}

fn run(path: &Path) {
    let name = path.display();
    let case: Case = serde_json::from_str(&fs::read_to_string(path).expect("read fixture"))
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    let mut replay = Replay::default();
    let mut last_at = i64::MIN;
    for (index, step) in case.steps.iter().enumerate() {
        let at = step.at_ms;
        assert!(at >= last_at, "{name} step {index}: time goes backwards");
        last_at = at;
        let context = format!(
            "{name} step {index} at {at}ms {}",
            step.note.as_deref().unwrap_or("")
        );
        if let Some(text) = &step.output {
            replay.feed(text.as_bytes(), at);
        }
        if let Some(hex) = &step.output_hex {
            replay.feed(&decode_hex(hex), at);
        }
        if let Some(exit) = &step.exit {
            replay.observations.record(
                Source::Process,
                Observation {
                    state: AgentState::Exited { code: exit.code },
                    at_ms: at,
                },
            );
        }
        let published = replay.tick(at);
        if let Some(expected) = step.expect {
            assert_eq!(published, expected, "{context}");
        }
        if let Some(expected) = step.expect_replies {
            assert_eq!(replay.replies, expected, "{context}");
        }
        if let Some(expected) = &step.expect_message {
            assert_eq!(&replay.published_message(published), expected, "{context}");
        }
    }
}

#[test]
fn every_recorded_stream_yields_its_status_timeline() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/agent-replay/status");
    let mut cases: Vec<_> = fs::read_dir(&dir)
        .expect("fixture directory")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    cases.sort();
    assert!(
        cases.len() >= 8,
        "fixtures went missing in {}",
        dir.display()
    );
    for case in &cases {
        run(case);
    }
}
