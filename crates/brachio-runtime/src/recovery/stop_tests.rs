use brachio_runtime::protocol;
use brachio_runtime::wire::{Action, Request};

use super::*;

fn call(runtime: &Runtime, action: Action) {
    runtime
        .execute(Request {
            version: protocol::CURRENT,
            action,
        })
        .expect("request");
}

#[test]
fn a_stopped_terminal_is_not_written_back_to_the_journal_by_a_later_save() {
    let directory = std::env::temp_dir().join(format!("brachio-stop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    let runtime = Runtime::new(Some(Journal::open(&directory).expect("open journal")));
    let (program, args) = if cfg!(windows) {
        ("ping.exe", vec!["-n", "30", "127.0.0.1"])
    } else {
        ("sleep", vec!["30"])
    };
    call(
        &runtime,
        Action::Launch {
            key: "task/stopped".to_owned(),
            cwd: std::env::temp_dir(),
            program: program.to_owned(),
            args: args.into_iter().map(str::to_owned).collect(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        },
    );
    runtime.persist();
    let journal = runtime.journal.as_ref().expect("journal");
    assert_eq!(journal.load().expect("load").records.len(), 1);

    call(
        &runtime,
        Action::Stop {
            key: "task/stopped".to_owned(),
        },
    );
    runtime.persist();
    assert!(journal.load().expect("load").records.is_empty());
    let _ = std::fs::remove_dir_all(directory);
}

#[test]
fn guarded_stop_rejects_a_different_runtime_or_session() {
    let runtime = Runtime::new(None);
    let key = "guarded-stop".to_owned();
    let launch = || Action::Ensure {
        key: key.clone(),
        cwd: std::env::temp_dir(),
    };
    let reply = runtime
        .execute(Request {
            version: protocol::CURRENT,
            action: launch(),
        })
        .expect("launch");
    let ResponseResult::Success { session, .. } = reply else {
        panic!("expected session")
    };
    for (instance, session_id) in [
        (
            "old-runtime".to_owned(),
            session.session_id.to_string().parse().expect("id"),
        ),
        (runtime.instance.clone(), u64::MAX),
    ] {
        let result = runtime.execute(Request {
            version: protocol::CURRENT,
            action: Action::StopGuarded {
                key: key.clone(),
                instance,
                session_id,
            },
        });
        assert!(result.is_err());
        assert!(runtime
            .execute(Request {
                version: protocol::CURRENT,
                action: Action::Inspect { key: key.clone() },
            })
            .is_ok());
    }
    call(
        &runtime,
        Action::StopGuarded {
            key,
            instance: runtime.instance.clone(),
            session_id: session.session_id.to_string().parse().expect("id"),
        },
    );
}
