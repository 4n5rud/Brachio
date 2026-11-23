//! Verifies the client library the app uses against a real runtime process.

use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use brachio_runtime::client::{Client, ClientError};
use brachio_runtime::wire::{Action, ResponseResult};
use uuid::Uuid;

const RUNTIME: &str = env!("CARGO_BIN_EXE_brachio-runtime");

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn endpoint() -> String {
    format!("client-{}", &Uuid::new_v4().simple().to_string()[..12])
}

fn state_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("brachio-state-{name}"))
}

fn server(name: &str) -> Server {
    let state = state_dir(name);
    Server(
        Command::new(RUNTIME)
            .args(["--pipe", name, "--state-dir"])
            .arg(state)
            .arg("serve")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start runtime"),
    )
}

async fn ready(client: &Client) {
    for _ in 0..200 {
        if client.call(Action::Hello).await.is_ok() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    panic!("runtime did not become ready");
}

#[tokio::test]
async fn dropping_a_follower_leaves_the_session_running_and_followable_again() {
    let name = endpoint();
    let _host = server(&name);
    let client = Client::new(&name);
    ready(&client).await;
    client
        .call(Action::Ensure {
            key: "detach".to_owned(),
            cwd: std::env::temp_dir(),
        })
        .await
        .expect("start shell");
    let follow = || Action::Follow {
        key: "detach".to_owned(),
        after: 0,
    };
    let (_, following) = client.follow(follow()).await.expect("follow");
    drop(following);

    tokio::time::sleep(Duration::from_millis(300)).await;
    let reply = client
        .call(Action::Inspect {
            key: "detach".to_owned(),
        })
        .await
        .expect("inspect");
    let ResponseResult::Success { session, .. } = reply.result else {
        panic!("unexpected reply");
    };
    assert!(
        !session.exited,
        "a dropped follower must not end the session"
    );
    client.follow(follow()).await.expect("follow again");
    client
        .call(Action::Stop {
            key: "detach".to_owned(),
        })
        .await
        .expect("stop");
}

#[tokio::test]
async fn a_missing_runtime_is_reported_as_not_running() {
    let client = Client::new(&endpoint());
    let error = client.call(Action::Hello).await.expect_err("no runtime");
    assert!(matches!(error, ClientError::NotRunning), "{error:?}");
}

#[tokio::test]
async fn the_app_can_start_follow_and_write_to_a_shell() {
    let name = endpoint();
    let _host = server(&name);
    let client = Client::new(&name);
    ready(&client).await;

    let cwd = std::env::temp_dir();
    client
        .call(Action::Ensure {
            key: "task".to_owned(),
            cwd,
        })
        .await
        .expect("start shell");
    let refused = client
        .call(Action::Inspect {
            key: "missing".to_owned(),
        })
        .await
        .expect_err("unknown shell");
    assert!(matches!(refused, ClientError::Refused(_)), "{refused:?}");

    let mut offset = None;
    for _ in 0..200 {
        let (first, _) = match client
            .follow(Action::FollowFrame {
                key: "task".to_owned(),
            })
            .await
        {
            Ok(opened) => opened,
            Err(ClientError::Refused(_)) => {
                tokio::time::sleep(Duration::from_millis(40)).await;
                continue;
            }
            Err(error) => panic!("{error}"),
        };
        if let ResponseResult::Snapshot { screen, .. } = first.result {
            offset = screen.resume_offset;
            break;
        }
    }
    assert!(offset.is_some(), "no settled frame to follow from");

    let (_, mut following) = client
        .follow(Action::Follow {
            key: "task".to_owned(),
            after: offset.expect("offset"),
        })
        .await
        .expect("follow output");
    let ending = if cfg!(windows) { "\r" } else { "\n" };
    client
        .call(Action::Write {
            key: "task".to_owned(),
            text: format!("echo brachio-client-ok{ending}"),
        })
        .await
        .expect("write");

    let mut seen = Vec::new();
    while !String::from_utf8_lossy(&seen).contains("brachio-client-ok") {
        let reply = tokio::time::timeout(Duration::from_secs(8), following.next())
            .await
            .expect("output arrived in time")
            .expect("follow reply")
            .expect("connection open");
        if let ResponseResult::Success {
            output: Some(page), ..
        } = reply.result
        {
            seen.extend(page.bytes);
        }
    }
    client
        .call(Action::Stop {
            key: "task".to_owned(),
        })
        .await
        .expect("stop");
}

#[tokio::test]
async fn concurrent_recovery_is_read_only_and_reconnect_does_not_replay_input() {
    let name = endpoint();
    let state = state_dir(&name);
    let marker = state.join("replay-check.txt");
    let key = "task".to_owned();
    let client = Client::new(&name);
    let ensure = || Action::Ensure {
        key: key.clone(),
        cwd: state.clone(),
    };
    let ending = if cfg!(windows) { "\r" } else { "\n" };

    let mut first = server(&name);
    ready(&client).await;
    let before = client.call(ensure()).await.expect("start original shell");
    let ResponseResult::Success {
        session: original, ..
    } = before.result
    else {
        panic!("expected live session");
    };
    client
        .call(Action::Write {
            key: key.clone(),
            text: format!("echo counted>>replay-check.txt{ending}"),
        })
        .await
        .expect("write command with a visible side effect");
    let saved = state.join("task.json");
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let marker_written =
            fs::read_to_string(&marker).is_ok_and(|text| text.lines().eq(["counted"]));
        let journal_written =
            fs::read_to_string(&saved).is_ok_and(|record| record.contains("replay-check.txt"));
        if marker_written && journal_written {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "wait for the original command and screen"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    first.0.kill().expect("simulate runtime crash");
    first.0.wait().expect("reap old runtime");

    fs::write(state.join("task.json.tmp"), b"{\"key\":")
        .expect("leave a truncated unwritten checkpoint");
    fs::write(state.join("other.json"), b"{not a record").expect("leave a corrupt committed entry");

    let second = server(&name);
    ready(&client).await;
    let peer = Client::new(&name);
    let listed = client.call(Action::Lost).await.expect("list lost sessions");
    let ResponseResult::Lost { lost } = listed.result else {
        panic!("expected lost sessions");
    };
    assert_eq!(lost.len(), 1);
    assert_eq!(lost[0].key, key);
    assert!(!state.join("other.json").exists());
    let set_aside = fs::read_dir(&state)
        .expect("list state")
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("other.json.corrupt-")
        });
    assert!(
        set_aside,
        "the corrupt entry is renamed to other.json.corrupt-<ms>"
    );

    let recover = || Action::Recover { key: key.clone() };
    let (left, right) = tokio::join!(client.call(recover()), peer.call(recover()));
    for reply in [left, right] {
        let ResponseResult::Recovered { lost, screen } = reply.expect("recover screen").result
        else {
            panic!("expected saved screen");
        };
        assert_eq!(lost.former_pid, original.pid);
        assert_eq!(lost.former_instance, before.instance);
        assert!(screen.ansi.contains("replay-check.txt"));
    }
    let inspect = || Action::Inspect { key: key.clone() };
    assert!(matches!(
        client.call(inspect()).await,
        Err(ClientError::Refused(_))
    ));

    let (left, right) = tokio::join!(client.call(ensure()), peer.call(ensure()));
    let get_session = |reply: brachio_runtime::wire::Reply| {
        let ResponseResult::Success { session, .. } = reply.result else {
            panic!("expected live shell");
        };
        session
    };
    let live = get_session(left.expect("first reconnect"));
    let same = get_session(right.expect("second reconnect"));
    assert_eq!(live.pid, same.pid);
    assert_eq!(live.session_id, same.session_id);
    assert_ne!(live.pid, original.pid);
    assert!(matches!(
        client.call(recover()).await,
        Err(ClientError::Refused(_))
    ));

    // The new shell processes a fresh command before we check the old side effect.
    let ready_marker = state.join("reconnect-ready.txt");
    client
        .call(Action::Write {
            key: key.clone(),
            text: format!("echo ready>reconnect-ready.txt{ending}"),
        })
        .await
        .expect("write to reconnected shell");
    let deadline = Instant::now() + Duration::from_secs(8);
    while !fs::read_to_string(&ready_marker).is_ok_and(|text| text.lines().eq(["ready"])) {
        assert!(Instant::now() < deadline, "wait for the new shell");
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
    assert!(fs::read_to_string(marker)
        .expect("read side effect")
        .lines()
        .eq(["counted"]));
    client
        .call(Action::Stop { key })
        .await
        .expect("stop test shell");
    drop(second);
    let _ = fs::remove_dir_all(state);
}

#[tokio::test]
async fn a_slow_lifecycle_reply_over_the_socket_stays_pending_until_the_outcome() {
    use brachio_runtime::transport;
    use brachio_runtime::wire::{
        read_message, write_message, Request, Response, IO_TIMEOUT, MAX_MESSAGE, MAX_RESPONSE,
    };
    use interprocess::local_socket::tokio::prelude::*;
    use tokio::io::BufReader;

    let name = endpoint();
    let listener = transport::listen(&name).expect("listen");
    let client = tokio::spawn(async move {
        Client::new(&name)
            .call(Action::Ensure {
                key: "slow".into(),
                cwd: std::env::temp_dir(),
            })
            .await
    });
    answer_hello(&listener).await;
    let mut socket = BufReader::new(listener.accept().await.expect("accept"));
    let bytes = read_message(&mut socket, MAX_MESSAGE)
        .await
        .expect("read")
        .expect("request");
    let request: Request = serde_json::from_slice(&bytes).expect("decode");
    assert!(matches!(&request.action, Action::Ensure { key, .. } if key == "slow"));

    tokio::time::pause();
    tokio::task::yield_now().await;
    tokio::time::advance(IO_TIMEOUT + Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert!(!client.is_finished());

    write_message(
        socket.get_mut(),
        &Response {
            version: request.version,
            instance: "fixture",
            result: ResponseResult::Error {
                error: "definitive refusal".into(),
            },
        },
        MAX_RESPONSE,
    )
    .await
    .expect("respond");
    assert!(
        matches!(client.await.expect("join"), Err(ClientError::Refused(message)) if message == "definitive refusal")
    );
}

#[tokio::test]
async fn a_disconnect_after_dispatching_stop_leaves_its_outcome_unknown() {
    use brachio_runtime::transport;
    use brachio_runtime::wire::{read_message, MAX_MESSAGE};
    use interprocess::local_socket::tokio::prelude::*;
    use tokio::io::BufReader;

    let name = endpoint();
    let listener = transport::listen(&name).expect("listen");
    let client = tokio::spawn(async move {
        Client::new(&name)
            .call(Action::Stop { key: "stop".into() })
            .await
    });
    answer_hello(&listener).await;
    let mut socket = BufReader::new(listener.accept().await.expect("accept"));
    assert!(read_message(&mut socket, MAX_MESSAGE)
        .await
        .expect("read")
        .is_some());
    drop(socket);
    assert!(matches!(
        client.await.expect("join"),
        Err(ClientError::OutcomeUnknown(_))
    ));
}

async fn answer_hello(listener: &interprocess::local_socket::tokio::Listener) {
    use brachio_runtime::protocol;
    use brachio_runtime::wire::{
        read_message, write_message, Request, Response, RuntimeInfo, MAX_MESSAGE, MAX_RESPONSE,
    };
    use interprocess::local_socket::traits::tokio::Listener as _;
    use tokio::io::BufReader;

    let mut socket = BufReader::new(listener.accept().await.expect("accept hello"));
    let bytes = read_message(&mut socket, MAX_MESSAGE)
        .await
        .expect("read hello")
        .expect("hello request");
    let request: Request = serde_json::from_slice(&bytes).expect("decode hello");
    assert!(matches!(request.action, Action::Hello));
    write_message(
        socket.get_mut(),
        &Response {
            version: request.version,
            instance: "fixture",
            result: ResponseResult::Hello {
                runtime: RuntimeInfo {
                    pid: 0,
                    oldest_version: protocol::OLDEST,
                    current_version: protocol::CURRENT,
                    draining: false,
                },
            },
        },
        MAX_RESPONSE,
    )
    .await
    .expect("answer hello");
}
