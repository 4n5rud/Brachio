//! Runs the real `brachio skills` and checks the shipped stubs point at it.

use std::path::Path;
use std::process::Command;

const BRACHIO: &str = env!("CARGO_BIN_EXE_brachio");

fn brachio(arguments: &[&str]) -> std::process::Output {
    Command::new(BRACHIO)
        .args(arguments)
        .output()
        .expect("run brachio")
}

#[test]
fn list_prints_each_skill_name_on_its_own_line() {
    let output = brachio(&["skills", "list"]);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n"),
        "brachio-send\nbrachio-lead\n"
    );
}

#[test]
fn get_prints_the_guide_without_front_matter() {
    let send = brachio(&["skills", "get", "brachio-send"]);
    assert!(send.status.success());
    let text = String::from_utf8_lossy(&send.stdout);
    assert!(text.contains("brachio peers") && text.contains("brachio check --wait"));
    assert!(!text.starts_with("---"));

    let lead = brachio(&["skills", "get", "brachio-lead"]);
    assert!(String::from_utf8_lossy(&lead.stdout).contains("brachio task create"));
}

#[test]
fn an_unknown_skill_fails_and_names_the_available_ones() {
    let output = brachio(&["skills", "get", "nope"]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("brachio-send") && error.contains("brachio-lead"));
}

#[test]
fn each_shipped_stub_points_at_its_own_guide() {
    let skills = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/skills");
    for name in ["brachio-send", "brachio-lead"] {
        let stub = std::fs::read_to_string(skills.join(name).join("SKILL.md")).expect("stub");
        assert!(
            stub.contains(&format!("brachio skills get {name}")),
            "{name}"
        );
        assert!(stub.lines().count() <= 15, "{name} stays a stub");
    }
}
