//! The app's terminal host against a real `brachio-runtime` process: what a window gets when it
//! hides, leaves, or meets a runtime that restarted. Each test owns its runtime and state folder.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use brachio_runtime::wire::ResponseResult;
use tauri::ipc::{Channel, InvokeResponseBody};
use tokio::sync::MutexGuard;

use super::{
    launch_action, pending_launch, save_pending_launch, Action, Client, Launch, PendingLaunch,
    RuntimeTerminals, Uuid,
};

use crate::runtime_client::TEST_ENVIRONMENT as ENVIRONMENT;

const MARKER: &str = "brachio-flow-marker";

/// A runtime and the folders it and the app write to, all under a temporary folder.
struct World {
    _lock: MutexGuard<'static, ()>,
    folder: tempfile::TempDir,
    endpoint: String,
    executable: PathBuf,
    runtime: Option<Child>,
    restore: Vec<(&'static str, Option<OsString>)>,
}

impl World {
    async fn start() -> Self {
        let lock = ENVIRONMENT.lock().await;
        let folder = tempfile::tempdir().expect("temp dir");
        let endpoint = format!("brachio-flow-{}", &Uuid::new_v4().simple().to_string()[..8]);
        let executable = runtime_executable();
        let mut restore = Vec::new();
        let mut set = |name: &'static str, value: OsString| {
            restore.push((name, std::env::var_os(name)));
            std::env::set_var(name, value);
        };
        set("BRACHIO_TERMINAL_RUNTIME", "1".into());
        set("BRACHIO_RUNTIME_ENDPOINT", endpoint.clone().into());
        set("BRACHIO_RUNTIME_EXECUTABLE", executable.clone().into());
        let state_base = folder.path().join("app-state").into_os_string();
        #[cfg(windows)]
        set("LOCALAPPDATA", state_base);
        #[cfg(target_os = "macos")]
        set("HOME", state_base);
        #[cfg(all(unix, not(target_os = "macos")))]
        set("XDG_STATE_HOME", state_base);
        let mut world = Self {
            _lock: lock,
            folder,
            endpoint,
            executable,
            runtime: None,
            restore,
        };
        world.start_runtime().await;
        world
    }

    fn client(&self) -> Client {
        Client::new(&self.endpoint)
    }

    async fn start_runtime(&mut self) {
        let child = Command::new(&self.executable)
            .args(["--pipe", &self.endpoint, "--state-dir"])
            .arg(self.folder.path().join("runtime-state"))
            .arg("serve")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("start runtime");
        self.runtime = Some(child);
        let client = self.client();
        eventually("the runtime to answer", || async {
            client.call(Action::Hello).await.is_ok()
        })
        .await;
    }

    /// Ends the runtime without a goodbye, as a crash or a forced update would.
    fn kill_runtime(&mut self) {
        let mut child = self.runtime.take().expect("a runtime to kill");
        child.kill().expect("kill runtime");
        child.wait().expect("reap runtime");
    }

    async fn restart_runtime(&mut self) {
        self.kill_runtime();
        self.start_runtime().await;
    }

    async fn instance(&self) -> String {
        self.client()
            .call(Action::Hello)
            .await
            .expect("hello")
            .instance
    }

    /// The process hosted for `key`, `None` while the runtime hosts nothing for it.
    async fn pid_of(&self, key: &str) -> Option<u32> {
        let reply = self
            .client()
            .call(Action::Inspect {
                key: key.to_owned(),
            })
            .await
            .ok()?;
        let ResponseResult::Success { session, .. } = reply.result else {
            return None;
        };
        Some(session.pid)
    }

    fn launch(&self) -> Launch {
        let (program, args): (&str, &[&str]) = if cfg!(windows) {
            ("cmd.exe", &["/d", "/k", "echo brachio-flow-marker"])
        } else {
            ("sh", &["-c", "echo brachio-flow-marker; exec cat"])
        };
        Launch {
            cwd: self.folder.path().to_owned(),
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if let Some(mut child) = self.runtime.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        for (name, value) in self.restore.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

/// The runtime build next to the test executables, built first when a narrower test run left it out.
fn runtime_executable() -> PathBuf {
    let name = format!("brachio-runtime{}", std::env::consts::EXE_SUFFIX);
    let beside = std::env::current_exe()
        .expect("test executable")
        .parent()
        .and_then(std::path::Path::parent)
        .expect("target folder")
        .join(name);
    if !beside.exists() {
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let status = Command::new(cargo)
            .args(["build", "--locked", "-p", "brachio-runtime"])
            .status()
            .expect("run cargo");
        assert!(status.success() && beside.exists(), "build brachio-runtime");
    }
    beside
}

async fn eventually<F, Fut>(what: &str, mut check: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    for _ in 0..2400 {
        if check().await {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("timed out waiting for {what}");
}

/// What one window channel was sent, in order: output as text, messages as their JSON.
#[derive(Clone, Default)]
struct Window(Arc<Mutex<Vec<String>>>);

impl Window {
    fn channel(&self) -> Channel<InvokeResponseBody> {
        let log = Arc::clone(&self.0);
        Channel::new(move |body| {
            let entry = match body {
                InvokeResponseBody::Raw(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                InvokeResponseBody::Json(json) => format!("<{json}>"),
            };
            log.lock().expect("lock").push(entry);
            Ok(())
        })
    }

    fn entries(&self) -> Vec<String> {
        self.0.lock().expect("lock").clone()
    }

    fn shows(&self, text: &str) -> bool {
        self.entries().iter().any(|entry| entry.contains(text))
    }

    async fn shows_eventually(&self, text: &str) {
        eventually(text, || async { self.shows(text) }).await;
    }
}

async fn open_new(
    terminals: &RuntimeTerminals,
    world: &World,
    key: &str,
    window: &Window,
) -> (brachio_pty::SessionId, bool) {
    terminals
        .open(key.to_owned(), world.launch(), window.channel(), false)
        .await
        .expect("open")
        .expect("the runtime is used")
}

#[tokio::test]
async fn a_parked_terminal_gets_the_screen_it_missed_when_it_is_opened_again() {
    let world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let (first_window, second_window) = (Window::default(), Window::default());
    let key = "park1:agent";

    let (parked, attached) = open_new(&terminals, &world, key, &first_window).await;
    assert!(!attached);
    first_window.shows_eventually(MARKER).await;
    let pid = world.pid_of(key).await.expect("a hosted terminal");
    terminals.park(parked).expect("hosted").expect("parked");
    terminals
        .write(parked, "echo brachio-while-hidden\r".to_owned())
        .await
        .expect("hosted")
        .expect("write");
    eventually("the hidden output to reach the runtime", || async {
        let listing = world.client().call(Action::Snapshot { key: key.to_owned() }).await;
        matches!(
            listing.map(|reply| reply.result),
            Ok(ResponseResult::Snapshot { screen, .. }) if screen.ansi.contains("brachio-while-hidden")
        )
    })
    .await;
    assert!(!first_window.shows("brachio-while-hidden"));

    let (again, attached) = open_new(&terminals, &world, key, &second_window).await;
    assert!(
        attached,
        "the running program is reattached, not started again"
    );
    assert_ne!(again, parked);
    assert_eq!(world.pid_of(key).await, Some(pid));
    second_window.shows_eventually(MARKER).await;
    second_window.shows_eventually("brachio-while-hidden").await;
    assert!(!second_window.shows("restored"));
}

#[tokio::test]
async fn a_detached_terminal_keeps_running_and_shows_its_screen_when_reattached() {
    let world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let (first_window, second_window) = (Window::default(), Window::default());
    let key = "detach1:agent";

    let (id, _) = open_new(&terminals, &world, key, &first_window).await;
    first_window.shows_eventually(MARKER).await;
    let pid = world.pid_of(key).await.expect("a hosted terminal");
    terminals.detach(id).expect("hosted").expect("detached");
    assert_eq!(
        world.pid_of(key).await,
        Some(pid),
        "detaching leaves the program running"
    );

    let (_, attached) = open_new(&terminals, &world, key, &second_window).await;
    assert!(attached);
    assert_eq!(world.pid_of(key).await, Some(pid));
    second_window.shows_eventually(MARKER).await;
}

#[tokio::test]
async fn a_restarted_runtime_shows_the_last_screen_before_the_terminal_starts_again() {
    let mut world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let (first_window, second_window) = (Window::default(), Window::default());
    let key = "restart1:agent";

    open_new(&terminals, &world, key, &first_window).await;
    first_window.shows_eventually(MARKER).await;
    let before = world.pid_of(key).await.expect("a hosted terminal");
    let journal = world.folder.path().join("runtime-state");
    eventually("the screen to be saved", || async {
        std::fs::read_dir(&journal).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                std::fs::read_to_string(entry.path()).is_ok_and(|saved| saved.contains(MARKER))
            })
        })
    })
    .await;
    world.restart_runtime().await;
    assert_eq!(
        world.pid_of(key).await,
        None,
        "the new runtime hosts nothing yet"
    );

    let (_, attached) = open_new(&terminals, &world, key, &second_window).await;
    assert!(!attached, "a new program starts after the restored screen");
    eventually(
        "the saved screen and the restored notice to arrive",
        || async {
            let entries = second_window.entries();
            entries.iter().any(|entry| entry.contains(MARKER))
                && entries
                    .iter()
                    .any(|entry| entry == r#"<{"restored":true}>"#)
        },
    )
    .await;
    let entries = second_window.entries();
    let screen = entries
        .iter()
        .position(|entry| entry.contains(MARKER))
        .expect("the saved screen is sent");
    let restored = entries
        .iter()
        .position(|entry| entry == r#"<{"restored":true}>"#)
        .expect("the window is told the screen is a restored one");
    assert!(screen < restored, "{entries:?}");
    assert_ne!(world.pid_of(key).await.expect("a new terminal"), before);
}

#[tokio::test]
async fn a_launch_the_new_runtime_cannot_account_for_stays_uncertain_until_the_user_decides() {
    let mut world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let window = Window::default();
    let key = "uncertain1:agent";
    let record = PendingLaunch {
        key: key.to_owned(),
        instance: world.instance().await,
        operation_id: Uuid::new_v4().to_string(),
    };
    save_pending_launch(&record).expect("record the launch before the crash");
    world.restart_runtime().await;

    let expected = format!("BRACHIO_LAUNCH_UNCERTAIN:{}", record.operation_id);
    for _ in 0..2 {
        let refused = terminals
            .open(key.to_owned(), world.launch(), window.channel(), false)
            .await
            .expect_err("an unknown launch is not repeated");
        assert_eq!(refused.to_string(), expected);
        assert_eq!(world.pid_of(key).await, None, "nothing was started");
    }

    let wrong = terminals
        .forget_uncertain_launch(key, &Uuid::new_v4().to_string())
        .await;
    assert!(wrong.is_err());
    assert!(pending_launch(key).expect("read").is_some());
    terminals
        .forget_uncertain_launch(key, &record.operation_id)
        .await
        .expect("forget");
    assert!(pending_launch(key).expect("read").is_none());

    let (_, attached) = open_new(&terminals, &world, key, &window).await;
    assert!(!attached);
    window.shows_eventually(MARKER).await;
}

#[tokio::test]
async fn a_launch_the_runtime_finished_before_the_app_noticed_is_adopted_not_repeated() {
    let world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let window = Window::default();
    let key = "adopt1:agent";
    let record = PendingLaunch {
        key: key.to_owned(),
        instance: world.instance().await,
        operation_id: Uuid::new_v4().to_string(),
    };
    save_pending_launch(&record).expect("record the launch");
    world
        .client()
        .call(launch_action(key, world.launch(), &record))
        .await
        .expect("the runtime launches before the app is told");
    let pid = world.pid_of(key).await.expect("a hosted terminal");

    open_new(&terminals, &world, key, &window).await;
    assert_eq!(world.pid_of(key).await, Some(pid), "no second program");
    window.shows_eventually(MARKER).await;
    assert!(
        pending_launch(key).expect("read").is_none(),
        "the record is settled"
    );
}
