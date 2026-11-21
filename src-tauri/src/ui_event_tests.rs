//! What the window is told, and what its commands answer, with the real handlers and state on
//! Tauri's mock runtime. The window itself is absent: a listener plays its part.

use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use brachio_store::{open_in_memory, NewRepo, RepoRecord};
use serde_json::{json, Value};
use tauri::test::{
    get_ipc_response, mock_builder, mock_context, noop_assets, MockRuntime, INVOKE_KEY,
};
use tauri::webview::InvokeRequest;
use tauri::{App, Listener, Manager, WebviewWindow, WebviewWindowBuilder};

use crate::child_task_request::{AppChildTasks, ChildRequest, ChildTasks};
use crate::hook_socket::{Approvals, NoChildTasks};
use crate::qa_run::QaResults;
use crate::workspace::{Database, TaskWorktreeLocks};

type Events = Arc<Mutex<Vec<Value>>>;

fn capture(app: &App<MockRuntime>, name: &str) -> Events {
    let seen = Events::default();
    let sink = Arc::clone(&seen);
    app.handle().listen(name, move |event| {
        let payload = serde_json::from_str(event.payload()).expect("event payload is JSON");
        sink.lock().expect("lock").push(payload);
    });
    seen
}

fn wait_for(events: &Events, count: usize) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let seen = events.lock().expect("lock").clone();
        if seen.len() >= count {
            return seen;
        }
        assert!(
            Instant::now() < deadline,
            "expected {count} events, saw {seen:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn git(folder: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(folder)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?}");
}

fn git_repo(folder: &Path) {
    std::fs::create_dir_all(folder).expect("make repo");
    git(folder, &["init", "-q", "-b", "main"]);
    git(folder, &["commit", "-q", "--allow-empty", "-m", "start"]);
}

fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    let url = if cfg!(any(windows, target_os = "android")) {
        "http://tauri.localhost"
    } else {
        "tauri://localhost"
    };
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: url.parse().expect("url"),
        body: tauri::ipc::InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(window, request).map(|body| body.deserialize::<Value>().expect("JSON reply"))
}

mod child_task_events {
    use super::*;

    struct Fixture {
        _root: tempfile::TempDir,
        app: App<MockRuntime>,
    }

    fn fixture(scheduled: bool) -> Fixture {
        let root = tempfile::tempdir().expect("temp dir");
        let repo_path = root.path().join("repo");
        git_repo(&repo_path);
        let connection = open_in_memory().expect("open database");
        RepoRecord::insert(
            &connection,
            &NewRepo {
                id: "r",
                name: "repo",
                path: &repo_path,
                default_branch: "main",
                added_at_ms: 0,
                is_git: true,
            },
        )
        .expect("insert repo");
        connection
            .execute_batch(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('lead', 'r', 'Lead', '', 'main', '/w0', 'active', 0);",
            )
            .expect("insert lead");
        if scheduled {
            brachio_store::set_scheduler_enabled(&connection, "lead", true).expect("schedule");
        }
        let app = mock_builder()
            .build(mock_context(noop_assets()))
            .expect("build app");
        app.manage(Database(Mutex::new(connection)));
        app.manage(TaskWorktreeLocks::default());
        Fixture { _root: root, app }
    }

    fn ask_for_child(app: &App<MockRuntime>) {
        let request = ChildRequest {
            key: "lead:agent".to_owned(),
            title: "Write docs".to_owned(),
            prompt: "do it".to_owned(),
            agent: None,
            base: None,
            after: Vec::new(),
        };
        tauri::async_runtime::block_on(
            AppChildTasks(app.handle().clone()).create("lead".to_owned(), request),
        )
        .expect("create child");
    }

    #[test]
    fn a_child_an_agent_asks_for_is_announced_once_for_the_window_to_open() {
        let Fixture { _root, app } = fixture(false);
        let created = capture(&app, "task://created");
        let started = capture(&app, "task://started");
        ask_for_child(&app);

        let seen = wait_for(&created, 1);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["title"], "Write docs");
        assert_eq!(seen[0]["parentTaskId"], "lead");
        assert!(started.lock().expect("lock").is_empty());
    }

    #[test]
    fn a_child_of_a_scheduling_lead_is_announced_as_started_not_created() {
        let Fixture { _root, app } = fixture(true);
        let created = capture(&app, "task://created");
        let started = capture(&app, "task://started");
        let changed = capture(&app, "orchestration://changed");
        ask_for_child(&app);

        let seen = wait_for(&started, 1);
        assert_eq!(seen[0]["title"], "Write docs");
        assert_eq!(seen[0]["parentTaskId"], "lead");
        wait_for(&changed, 1);
        assert!(created.lock().expect("lock").is_empty());
    }
}

mod approval_and_qa_events {
    use super::*;

    struct Fixture {
        _root: tempfile::TempDir,
        _app: App<MockRuntime>,
        window: WebviewWindow<MockRuntime>,
        asked: Events,
        closed: Events,
    }

    /// A task whose `brachio.toml` runs this test binary, which exits 0 when asked to list tests.
    fn fixture() -> Fixture {
        let root = tempfile::tempdir().expect("temp dir");
        let project = root.path().join("project");
        let worktree = root.path().join("worktree");
        std::fs::create_dir_all(&project).expect("make project");
        git_repo(&worktree);
        let program = std::env::current_exe().expect("test binary");
        std::fs::write(
            project.join("brachio.toml"),
            format!(
                "[qa]\nprogram = {:?}\nargs = [\"--list\"]\n",
                program.to_str().expect("utf-8")
            ),
        )
        .expect("write brachio.toml");
        let connection = open_in_memory().expect("open database");
        RepoRecord::insert(
            &connection,
            &NewRepo {
                id: "r",
                name: "project",
                path: &project,
                default_branch: "main",
                added_at_ms: 0,
                is_git: true,
            },
        )
        .expect("insert repo");
        connection
            .execute(
                "INSERT INTO tasks (id, repo_id, title, prompt, branch, worktree_path, state, created_at_ms)
                 VALUES ('t', 'r', 'T', '', 'main', ?1, 'active', 0)",
                [worktree.to_str().expect("utf-8")],
            )
            .expect("insert task");
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                crate::qa_run::qa_run,
                crate::qa_run::qa_last_result,
                crate::hook_socket::approval_answer,
            ])
            .build(mock_context(noop_assets()))
            .expect("build app");
        let handle = app.handle().clone();
        let approvals = Approvals::with_delivery(
            |_| true,
            move |notice| crate::forward_notice(&handle, notice),
            |_| brachio_runtime::mailbox::MailResponse::Error {
                message: "Messages are not available.".to_owned(),
            },
            |_| {},
            NoChildTasks,
        );
        app.manage(Database(Mutex::new(connection)));
        app.manage(approvals);
        app.manage(Arc::new(QaResults::default()));
        let window = WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .expect("window");
        let asked = capture(&app, "approval-asked");
        let closed = capture(&app, "approval-closed");
        Fixture {
            _root: root,
            _app: app,
            window,
            asked,
            closed,
        }
    }

    type QaReply = std::thread::JoinHandle<Result<Value, Value>>;

    /// Runs `qa_run` as the window would and returns once the approval question is on screen.
    fn start_qa(fixture: &Fixture) -> (QaReply, Value) {
        let window = fixture.window.clone();
        let reply = std::thread::spawn(move || invoke(&window, "qa_run", json!({ "taskId": "t" })));
        let question = wait_for(&fixture.asked, 1).remove(0);
        (reply, question)
    }

    #[test]
    fn an_approved_qa_run_asks_the_window_then_returns_and_stores_the_result() {
        let fixture = fixture();
        let (reply, question) = start_qa(&fixture);
        assert_eq!(question["key"], "t:agent");
        assert_eq!(question["kind"], "qa_run");
        let review = question["review"].as_str().expect("review");
        assert!(review.starts_with("Run QA\ncommand: "), "{review}");
        assert!(fixture.closed.lock().expect("lock").is_empty());

        let id = question["id"].clone();
        let answered = invoke(
            &fixture.window,
            "approval_answer",
            json!({ "id": id, "allow": true }),
        );
        assert_eq!(answered, Ok(json!(true)));

        let result = reply.join().expect("join").expect("qa result");
        assert_eq!(result["outcome"], "passed");
        assert_eq!(result["exitCode"], 0);
        assert_eq!(wait_for(&fixture.closed, 1), [id]);
        let stored = invoke(&fixture.window, "qa_last_result", json!({ "taskId": "t" }));
        assert_eq!(stored, Ok(result));
    }

    #[test]
    fn a_declined_qa_run_closes_the_question_and_leaves_no_result() {
        let fixture = fixture();
        let (reply, question) = start_qa(&fixture);
        let id = question["id"].clone();
        let answered = invoke(
            &fixture.window,
            "approval_answer",
            json!({ "id": id, "allow": false }),
        );
        assert_eq!(answered, Ok(json!(true)));

        assert_eq!(reply.join().expect("join"), Ok(Value::Null));
        assert_eq!(wait_for(&fixture.closed, 1), [id]);
        let stored = invoke(&fixture.window, "qa_last_result", json!({ "taskId": "t" }));
        assert_eq!(stored, Ok(Value::Null));
    }
}
