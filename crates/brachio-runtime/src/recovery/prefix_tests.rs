use std::path::PathBuf;

use super::*;

fn record(key: &str) -> SessionRecord {
    SessionRecord {
        key: key.to_owned(),
        cwd: PathBuf::from("work"),
        instance: "old-runtime".to_owned(),
        pid: 1,
        rows: 24,
        cols: 80,
        ansi: "bye".to_owned(),
        cursor: (0, 3),
        alternate_screen: false,
        output_bytes: 3,
        exited: true,
        exit_code: Some(0),
        saved_at_ms: 1,
    }
}

#[test]
fn forgetting_a_task_prefix_drops_only_its_saved_screens_from_memory_and_disk() {
    let directory = std::env::temp_dir().join(format!("brachio-prefix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let journal = Journal::open(&directory).expect("open journal");
    for key in [
        "t1:agent",
        "t1:shell",
        "t1:shell:ab",
        "t10:agent",
        "t2:agent",
    ] {
        journal.save(&record(key)).expect("save");
    }
    std::fs::write(directory.join("broken.json"), b"{nope").expect("corrupt file");
    let runtime = Runtime::new(Some(journal));

    assert!(runtime.forget_lost_with_prefix("").is_err());
    assert_eq!(runtime.forget_lost_with_prefix("t1:").expect("forget"), 3);
    assert_eq!(runtime.forget_lost_with_prefix("t1:").expect("again"), 0);

    let mut kept: Vec<String> = lock(&runtime.lost).expect("lost").keys().cloned().collect();
    kept.sort();
    assert_eq!(kept, ["t10:agent", "t2:agent"]);
    let on_disk = Journal::open(&directory)
        .expect("reopen")
        .load()
        .expect("load");
    let mut disk_keys: Vec<_> = on_disk.records.iter().map(|r| r.key.as_str()).collect();
    disk_keys.sort_unstable();
    assert_eq!(disk_keys, ["t10:agent", "t2:agent"]);
    let _ = std::fs::remove_dir_all(directory);
}
