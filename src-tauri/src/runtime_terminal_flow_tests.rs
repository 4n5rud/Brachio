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

#[cfg(unix)]
use super::follow_full_screen;
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
        .open(
            key.to_owned(),
            std::future::ready(Ok(world.launch())),
            window.channel(),
            false,
        )
        .await
        .expect("open")
        .expect("the runtime is used")
}

#[tokio::test]
async fn a_login_gate_refuses_new_processes_but_does_not_block_reattachment() {
    let world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let window = Window::default();
    let key = "login1:agent";
    let denied = || std::future::ready(Err(anyhow::anyhow!("sign in first")));
    let refused = terminals
        .open(key.to_owned(), denied(), window.channel(), true)
        .await
        .expect_err("a failed login check stops a new launch");
    assert_eq!(refused.to_string(), "sign in first");
    assert_eq!(world.pid_of(key).await, None);

    let (id, attached) = open_new(&terminals, &world, key, &window).await;
    assert!(!attached);
    assert!(terminals.park(id).expect("terminal is hosted").is_ok());
    let (_, attached) = terminals
        .open(key.to_owned(), denied(), window.channel(), true)
        .await
        .expect("the login gate does not run on reattachment")
        .expect("the runtime is used");
    assert!(attached);
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

/// How the output of [`full_screen_launch`] ends.
#[cfg(unix)]
#[derive(Clone, Copy)]
enum Ending {
    /// Settled: the runtime offers a frame at once.
    Settled,
    /// In the middle of an escape sequence for good, so the runtime never offers a frame.
    Unfinished,
    /// In the middle of an escape sequence until a line is typed, then the sequence is finished
    /// and `brachio-colored` is drawn in red.
    PausedMidSequence,
}

/// A program like Codex: it draws on the full screen, then keeps redrawing one line until it has
/// written far more than the runtime keeps, so the first line is no longer in the kept output.
/// Then it waits for input, which the terminal echoes.
#[cfg(unix)]
fn full_screen_launch(world: &World, ending: Ending) -> Launch {
    let ending = match ending {
        Ending::Settled => "'",
        Ending::Unfinished => "\\033['",
        Ending::PausedMidSequence => {
            "\\033['; stty -echo; read line; stty echo; printf '31mbrachio-colored\\033[0m'"
        }
    };
    let script = format!(
        "printf '\\033[?1049h\\033[H\\033[2Jbrachio-full-screen-ready'; i=0; \
         while [ $i -lt 400 ]; do printf '\\033[2;1H%01000d' $i; i=$((i+1)); done; \
         printf '\\033[3;1Hbrachio-drawn{ending}; exec cat"
    );
    Launch {
        args: vec!["-c".to_owned(), script],
        program: "sh".to_owned(),
        ..world.launch()
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_screen_program_with_long_output_is_redrawn_when_reattached() {
    let world = World::start().await;
    reattach_full_screen(&world, "fullscreen1:agent", Ending::Settled).await;
}

/// Makes the app take the runtime for an older one, and puts that back when dropped, even when the
/// test fails. The flag is shared by the whole test process, so it can only be set while the
/// test holds a [`World`], whose lock keeps every other test with a runtime waiting.
#[cfg(unix)]
struct OldRuntime<'a> {
    _world: &'a World,
}

#[cfg(unix)]
impl<'a> OldRuntime<'a> {
    fn pretend(world: &'a World) -> Self {
        super::PRETEND_OLD_RUNTIME.store(true, std::sync::atomic::Ordering::Relaxed);
        Self { _world: world }
    }
}

#[cfg(unix)]
impl Drop for OldRuntime<'_> {
    fn drop(&mut self) {
        super::PRETEND_OLD_RUNTIME.store(false, std::sync::atomic::Ordering::Relaxed);
    }
}

/// The whole open goes through the fallback for a runtime older than this app, which never
/// offers a full-screen frame.
#[cfg(unix)]
#[tokio::test]
async fn an_older_runtime_full_screen_is_still_redrawn_when_reattached() {
    let world = World::start().await;
    let _old = OldRuntime::pretend(&world);
    reattach_full_screen(&world, "fullscreen3:agent", Ending::Unfinished).await;
}

/// A runtime that offers full-screen frames but refuses one for a moment, in the middle of an
/// escape sequence: the app asks again and draws the settled frame. Following from the byte the
/// refused frame was taken at would draw the rest of the sequence as text.
#[cfg(unix)]
#[tokio::test]
async fn a_frame_refused_for_a_moment_is_asked_for_again_not_replaced() {
    let world = World::start().await;
    let key = "fullscreen4:agent";
    let terminals = RuntimeTerminals::default();
    let (first_window, second_window) = (Window::default(), Window::default());
    let (id, _) = terminals
        .open(
            key.to_owned(),
            std::future::ready(Ok(full_screen_launch(&world, Ending::PausedMidSequence))),
            first_window.channel(),
            true,
        )
        .await
        .expect("open")
        .expect("the runtime is used");
    first_window.shows_eventually("brachio-drawn").await;
    terminals.detach(id).expect("hosted").expect("detached");

    let client = world.client();
    let refused = client
        .follow(Action::FollowFrame {
            key: key.to_owned(),
        })
        .await;
    assert!(
        matches!(refused, Err(super::ClientError::Refused(_))),
        "the frame is refused while the sequence is open"
    );
    // Finishes the sequence while the app is still asking for a frame.
    let finish = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(150)).await;
        client
            .call(Action::Write {
                key: "fullscreen4:agent".to_owned(),
                text: "\n".to_owned(),
            })
            .await
            .expect("write");
    });
    terminals
        .open(
            key.to_owned(),
            std::future::ready(Err(anyhow::anyhow!("a live program is not started again"))),
            second_window.channel(),
            true,
        )
        .await
        .expect("reattach")
        .expect("the runtime is used");
    finish.await.expect("finish the sequence");
    second_window.shows_eventually("brachio-colored").await;
    second_window
        .shows_eventually("brachio-full-screen-ready")
        .await;
    let entries = second_window.entries();
    assert!(
        !entries.iter().any(|entry| entry.starts_with("31m")),
        "the rest of the sequence is drawn as text: {entries:?}"
    );
}

#[cfg(unix)]
async fn reattach_full_screen(world: &World, key: &str, ending: Ending) {
    let terminals = RuntimeTerminals::default();
    let (first_window, second_window) = (Window::default(), Window::default());

    let (id, _) = terminals
        .open(
            key.to_owned(),
            std::future::ready(Ok(full_screen_launch(world, ending))),
            first_window.channel(),
            true,
        )
        .await
        .expect("open")
        .expect("the runtime is used");
    first_window.shows_eventually("brachio-drawn").await;
    terminals.detach(id).expect("hosted").expect("detached");

    let (again, attached) = terminals
        .open(
            key.to_owned(),
            std::future::ready(Err(anyhow::anyhow!("a live program is not started again"))),
            second_window.channel(),
            true,
        )
        .await
        .expect("reattach")
        .expect("the runtime is used");
    assert!(attached);
    second_window
        .shows_eventually("brachio-full-screen-ready")
        .await;
    terminals
        .write(again, "brachio-typed-after\n".to_owned())
        .await
        .expect("hosted")
        .expect("write");
    second_window.shows_eventually("brachio-typed-after").await;
}

/// What an app does with a runtime that never offers a frame of a full screen: it draws the
/// current frame and goes on from the byte the frame was taken at.
#[cfg(unix)]
#[tokio::test]
async fn a_full_screen_frame_is_followed_from_the_byte_it_was_taken_at() {
    let world = World::start().await;
    let terminals = RuntimeTerminals::default();
    let window = Window::default();
    let key = "fullscreen2:agent";
    let (id, _) = terminals
        .open(
            key.to_owned(),
            std::future::ready(Ok(full_screen_launch(&world, Ending::Unfinished))),
            window.channel(),
            true,
        )
        .await
        .expect("open")
        .expect("the runtime is used");
    window.shows_eventually("brachio-drawn").await;

    let client = world.client();
    let (replies, mut following) = follow_full_screen(&client, key)
        .await
        .expect("a full screen is followed from its frame");
    let [frame, page] = <[_; 2]>::try_from(replies).expect("a frame, then the output after it");
    let ResponseResult::Snapshot { session, screen } = frame.result else {
        panic!("the frame comes first");
    };
    assert!(screen.alternate_screen);
    assert!(screen.ansi.contains("brachio-full-screen-ready"));
    assert_eq!(screen.resume_offset, None, "no frame to follow from");
    let ResponseResult::Success {
        output: Some(page), ..
    } = page.result
    else {
        panic!("the output follows the frame");
    };
    assert_eq!(page.start, session.output_bytes);
    assert!(!page.truncated);
    assert!(page.start > 256 * 1024, "the kept output starts later");

    terminals
        .write(id, "brachio-typed-after\n".to_owned())
        .await
        .expect("hosted")
        .expect("write");
    let mut seen = page.bytes;
    while !String::from_utf8_lossy(&seen).contains("brachio-typed-after") {
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
    assert!(!String::from_utf8_lossy(&seen).contains("brachio-full-screen-ready"));
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
            // Only a finished record counts: the runtime's scratch file holds the new screen
            // before the rename, and a kill in between would leave the older record behind.
            entries.flatten().any(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|extension| extension == "json")
                    && std::fs::read_to_string(entry.path())
                        .is_ok_and(|saved| saved.contains(MARKER))
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
            .open(
                key.to_owned(),
                std::future::ready(Ok(world.launch())),
                window.channel(),
                false,
            )
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

    terminals
        .open(
            key.to_owned(),
            std::future::ready(Err(anyhow::anyhow!("saved account removed"))),
            window.channel(),
            false,
        )
        .await
        .expect("pending launch is reconciled without preparing a new process");
    assert_eq!(world.pid_of(key).await, Some(pid), "no second program");
    window.shows_eventually(MARKER).await;
    assert!(
        pending_launch(key).expect("read").is_none(),
        "the record is settled"
    );
}
