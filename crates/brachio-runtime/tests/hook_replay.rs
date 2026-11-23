//! Replays recorded agent hook payloads through the real permission-hook parser and answer writer.
//!
//! Fixtures live in `brachio-core/tests/fixtures/agent-replay/hooks`; see the README there.

use std::fs;
use std::path::Path;

use brachio_runtime::approval::{hook_output, request_from_hook_input, Decision};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Case {
    events: Vec<Event>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Event {
    stdin: String,
    key: String,
    #[serde(default)]
    note: Option<String>,
    expect_request: Option<Expected>,
    #[serde(default)]
    decision: Option<Decision>,
    #[serde(default)]
    expect_output: Option<Value>,
}

#[derive(Deserialize)]
struct Expected {
    key: String,
    session_id: String,
    tool_name: String,
    tool_use_id: String,
}

fn run(path: &Path) {
    let name = path.display();
    let case: Case = serde_json::from_str(&fs::read_to_string(path).expect("read fixture"))
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    for (index, event) in case.events.iter().enumerate() {
        let context = format!(
            "{name} event {index} {}",
            event.note.as_deref().unwrap_or("")
        );
        let request = request_from_hook_input(&event.stdin, &event.key);
        let Some(expected) = &event.expect_request else {
            assert!(
                request.is_none(),
                "{context}: payload must not open an approval"
            );
            continue;
        };
        let request = request.unwrap_or_else(|| panic!("{context}: no approval opened"));
        assert_eq!(request.key, expected.key, "{context}");
        assert_eq!(request.session_id, expected.session_id, "{context}");
        assert_eq!(request.tool_name, expected.tool_name, "{context}");
        assert_eq!(request.tool_use_id, expected.tool_use_id, "{context}");
        assert!(
            request.review().is_some(),
            "{context}: not reviewable in the app"
        );

        let decision = event.decision.as_ref().expect("decision for an approval");
        let output = hook_output(decision)
            .map(|text| serde_json::from_str::<Value>(&text).expect("hook output is JSON"));
        assert_eq!(output, event.expect_output, "{context}");
    }
}

#[test]
fn every_recorded_hook_payload_yields_its_approval_and_answer() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../brachio-core/tests/fixtures/agent-replay/hooks");
    let mut cases: Vec<_> = fs::read_dir(&dir)
        .expect("fixture directory")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    cases.sort();
    assert!(
        cases.len() >= 6,
        "fixtures went missing in {}",
        dir.display()
    );
    for case in &cases {
        run(case);
    }
}
