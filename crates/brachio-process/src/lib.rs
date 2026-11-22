//! Short-lived helper programs (`git`, `gh`, `--version` probes) run for a bounded time.

use std::io::{self, Read};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use process_wrap::std::{ChildWrapper, CommandWrap};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
const POLL_EVERY: Duration = Duration::from_millis(10);
/// Extra time to read the pipes when the deadline already passed as the child exited.
const DRAIN_GRACE: Duration = Duration::from_millis(200);
/// How much standard error is kept. A program that prints more still runs on; the rest is read
/// and dropped, so it cannot fill the pipe and stall or fill memory.
const KEPT_STDERR: usize = 1024 * 1024;

/// Runs `command` and collects its output, stopping it after `limit`.
///
/// The child gets no stdin and, on Windows, no console window, so it can neither wait for a
/// typed answer nor flash a black window in the packaged app.
///
/// # Errors
/// Fails when the program cannot start, or with `ErrorKind::TimedOut` after the child was killed
/// for running longer than `limit`.
pub fn output_within(command: &mut Command, limit: Duration) -> io::Result<Output> {
    output_capped(command, limit, usize::MAX).map(|capped| capped.output)
}

/// Output of [`output_capped`].
pub struct Capped {
    /// What the program printed. Standard output holds at most one byte more than the cap.
    pub output: Output,
    /// The program printed more than the cap and was stopped; the output is only its start.
    pub truncated: bool,
}

/// Like [`output_within`], but stops the program as soon as it has printed more than
/// `max_stdout` bytes, so a huge result never has to fit in memory.
///
/// # Errors
/// Fails when the program cannot start, or with `ErrorKind::TimedOut` after the child was killed
/// for running longer than `limit`.
pub fn output_capped(
    command: &mut Command,
    limit: Duration,
    max_stdout: usize,
) -> io::Result<Capped> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    capture(Running::Single(child), limit, max_stdout)
}

/// Like [`output_capped`], but the program and everything it starts run as one unit: a Job
/// Object on Windows, a process group on Unix. When the time or output limit stops the run, and
/// when it ends for any reason, the whole tree is killed and reaped, so no worker outlives it.
///
/// Limits: a descendant that deliberately leaves the group (`setsid`, a detached service
/// started through another session or job) is out of reach.
///
/// # Errors
/// Fails when the program cannot start, or with `ErrorKind::TimedOut` after the tree was killed
/// for running longer than `limit`.
pub fn output_capped_tree(
    mut command: Command,
    limit: Duration,
    max_stdout: usize,
) -> io::Result<Capped> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    capture(
        Running::Tree(tree_command(command, false).spawn()?),
        limit,
        max_stdout,
    )
}

/// `command` as the leader of a tree: a Job Object on Windows, a process group on Unix.
/// `new_console` gives it a console of its own on Windows instead of none.
fn tree_command(command: Command, new_console: bool) -> CommandWrap {
    let mut wrapped = CommandWrap::from(command);
    #[cfg(windows)]
    {
        use process_wrap::std::{CreationFlags, JobObject};
        use windows::Win32::System::Threading::PROCESS_CREATION_FLAGS;
        let flags = if new_console {
            CREATE_NEW_CONSOLE
        } else {
            CREATE_NO_WINDOW
        };
        wrapped.wrap(CreationFlags(PROCESS_CREATION_FLAGS(flags)));
        wrapped.wrap(JobObject);
    }
    #[cfg(unix)]
    {
        let _ = new_console;
        wrapped.wrap(process_wrap::std::ProcessGroup::leader());
    }
    wrapped
}

/// How a run from [`run_tree_cancellable`] ended.
#[derive(Debug)]
pub enum Ended {
    /// The program finished by itself.
    Exited(ExitStatus),
    /// `cancel` was set and the whole tree was killed.
    Cancelled,
    /// `limit` passed and the whole tree was killed.
    TimedOut,
}

/// Runs `command` and everything it starts as one tree until it exits, `cancel` is set, or
/// `limit` passes; in the last two cases, and once it exits, the whole tree is killed and
/// reaped. Standard streams are left as `command` configures them.
///
/// Limits: a descendant that deliberately leaves the group (`setsid`, a detached service) is out
/// of reach, as for [`output_capped_tree`].
///
/// # Errors
/// Fails when the program cannot start or when waiting on it fails.
pub fn run_tree_cancellable(
    command: Command,
    new_console: bool,
    limit: Duration,
    cancel: &AtomicBool,
) -> io::Result<Ended> {
    let mut child = Running::Tree(tree_command(command, new_console).spawn()?);
    let deadline = Instant::now() + limit;
    loop {
        if cancel.load(Ordering::Acquire) {
            child.stop()?;
            return Ok(Ended::Cancelled);
        }
        if let Some(status) = child.try_wait()? {
            let _ = child.stop();
            return Ok(Ended::Exited(status));
        }
        if Instant::now() >= deadline {
            child.stop()?;
            return Ok(Ended::TimedOut);
        }
        thread::sleep(POLL_EVERY);
    }
}

/// Result of [`run_tree_reporting`].
#[derive(Debug)]
pub struct Reported {
    /// How the run ended.
    pub ended: Ended,
    /// The end of what the program printed on standard error, for an error message.
    pub stderr_tail: Vec<u8>,
}

/// Standard error kept by [`run_tree_reporting`]: the last bytes, where a failure is explained.
const KEPT_STDERR_TAIL: usize = 16 * 1024;

/// Like [`run_tree_cancellable`], but standard input and output are closed and every piece of
/// standard error goes to `on_stderr` as it arrives, so a long step such as `git clone` can show
/// progress. The tree is killed on exit, cancel and timeout alike.
///
/// # Errors
/// Fails when the program cannot start or when waiting on it fails.
pub fn run_tree_reporting(
    mut command: Command,
    limit: Duration,
    cancel: &AtomicBool,
    mut on_stderr: impl FnMut(&[u8]) + Send + 'static,
) -> io::Result<Reported> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = Running::Tree(tree_command(command, false).spawn()?);
    let (_, stderr) = child.take_pipes();
    let reader = stderr.map(|mut pipe| {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let mut tail: Vec<u8> = Vec::new();
            let mut chunk = [0u8; 4096];
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                on_stderr(&chunk[..read]);
                tail.extend_from_slice(&chunk[..read]);
                if tail.len() > KEPT_STDERR_TAIL {
                    tail.drain(..tail.len() - KEPT_STDERR_TAIL);
                }
            }
            let _ = sender.send(tail);
        });
        receiver
    });
    let deadline = Instant::now() + limit;
    let ended = loop {
        if cancel.load(Ordering::Acquire) {
            child.stop()?;
            break Ended::Cancelled;
        }
        if let Some(status) = child.try_wait()? {
            let _ = child.stop();
            break Ended::Exited(status);
        }
        if Instant::now() >= deadline {
            child.stop()?;
            break Ended::TimedOut;
        }
        thread::sleep(POLL_EVERY);
    };
    // The tree is dead, so the pipe closes; a stray descendant is not waited for.
    let stderr_tail = reader
        .and_then(|receiver| receiver.recv_timeout(DRAIN_GRACE * 5).ok())
        .unwrap_or_default();
    Ok(Reported { ended, stderr_tail })
}

/// A started program, alone or together with the tree it leads.
enum Running {
    Single(Child),
    Tree(Box<dyn ChildWrapper>),
}

impl Running {
    fn take_pipes(
        &mut self,
    ) -> (
        Option<std::process::ChildStdout>,
        Option<std::process::ChildStderr>,
    ) {
        match self {
            Self::Single(child) => (child.stdout.take(), child.stderr.take()),
            Self::Tree(child) => (child.stdout().take(), child.stderr().take()),
        }
    }

    fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        match self {
            Self::Single(child) => child.try_wait(),
            Self::Tree(child) => child.try_wait(),
        }
    }

    /// Kills the program, and its descendants when it leads a tree, then reaps it.
    fn stop(&mut self) -> io::Result<ExitStatus> {
        match self {
            Self::Single(child) => {
                let _ = child.kill();
                child.wait()
            }
            Self::Tree(child) => {
                let _ = child.start_kill();
                child.wait()
            }
        }
    }
}

fn capture(mut child: Running, limit: Duration, max_stdout: usize) -> io::Result<Capped> {
    let over = Arc::new(AtomicBool::new(false));
    let (stdout, stderr) = child.take_pipes();
    let stdout = stdout.map(|pipe| drain(pipe, max_stdout, Some(Arc::clone(&over))));
    let stderr = stderr.map(|pipe| drain_keeping(pipe, KEPT_STDERR));
    let started = Instant::now();
    let deadline = started + limit;
    let status = loop {
        if over.load(Ordering::Acquire) {
            break child.stop()?;
        }
        if let Some(status) = child.try_wait()? {
            if matches!(child, Running::Tree(_)) {
                let _ = child.stop();
            }
            break status;
        }
        if Instant::now() >= deadline {
            // Readers are left to finish by themselves: a grandchild may still hold the pipes.
            let _ = child.stop();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("timed out after {} seconds", limit.as_secs()),
            ));
        }
        thread::sleep(POLL_EVERY);
    };
    let now = Instant::now();
    let drain_until = if now >= deadline {
        now + DRAIN_GRACE
    } else {
        deadline
    };
    let stdout = collect(stdout, drain_until, limit)?;
    let stderr = collect(stderr, drain_until, limit)?;
    Ok(Capped {
        output: Output {
            status,
            stdout,
            stderr,
        },
        truncated: over.load(Ordering::Acquire),
    })
}

/// How a program run by [`stream_stdout`] ended.
pub struct Streamed {
    /// The exit status.
    pub status: std::process::ExitStatus,
    /// The start of what it printed on standard error.
    pub stderr: Vec<u8>,
}

/// Runs `command` and hands its standard output to `on_chunk` piece by piece, so a huge result
/// is never held in memory. The program is killed once `limit` passes.
///
/// # Errors
/// Fails when the program cannot start, or with `ErrorKind::TimedOut` after the child was killed
/// for running longer than `limit`.
pub fn stream_stdout(
    command: &mut Command,
    limit: Duration,
    mut on_chunk: impl FnMut(&[u8]),
) -> io::Result<Streamed> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child
        .stderr
        .take()
        .map(|pipe| drain_keeping(pipe, KEPT_STDERR));
    let deadline = Instant::now() + limit;
    let timed_out = || {
        io::Error::new(
            io::ErrorKind::TimedOut,
            format!("timed out after {} seconds", limit.as_secs()),
        )
    };
    if let Some(mut pipe) = child.stdout.take() {
        // A small queue keeps the reader at most a few chunks ahead of the consumer. The reader
        // is abandoned on timeout: it ends once the pipe closes and its send finds no receiver.
        let (sender, receiver) = mpsc::sync_channel::<Vec<u8>>(4);
        thread::spawn(move || {
            let mut chunk = [0u8; 64 * 1024];
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 || sender.send(chunk[..read].to_vec()).is_err() {
                    break;
                }
            }
        });
        loop {
            match receiver.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(chunk) => on_chunk(&chunk),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(timed_out());
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    }
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(timed_out());
        }
        thread::sleep(POLL_EVERY);
    };
    let drain_until = Instant::now().max(deadline) + DRAIN_GRACE;
    Ok(Streamed {
        status,
        stderr: collect(stderr, drain_until, limit)?,
    })
}

fn drain(
    mut pipe: impl Read + Send + 'static,
    cap: usize,
    over: Option<Arc<AtomicBool>>,
) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => bytes.extend_from_slice(&chunk[..read]),
            }
            if bytes.len() > cap {
                bytes.truncate(cap.saturating_add(1));
                if let Some(over) = &over {
                    over.store(true, Ordering::Release);
                }
                break;
            }
        }
        let _ = sender.send(bytes);
    });
    receiver
}

fn drain_keeping(mut pipe: impl Read + Send + 'static, keep: usize) -> Receiver<Vec<u8>> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut chunk = [0u8; 16 * 1024];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => {
                    let room = keep.saturating_sub(bytes.len()).min(read);
                    bytes.extend_from_slice(&chunk[..room]);
                }
            }
        }
        let _ = sender.send(bytes);
    });
    receiver
}

/// Waits for a reader only until `until`. A descendant that inherited the pipe keeps it open
/// after the child exits, so the reader is abandoned instead of joined; it ends by itself when
/// the pipe finally closes and its send then goes nowhere.
fn collect(
    reader: Option<Receiver<Vec<u8>>>,
    until: Instant,
    limit: Duration,
) -> io::Result<Vec<u8>> {
    let Some(reader) = reader else {
        return Ok(Vec::new());
    };
    match reader.recv_timeout(until.saturating_duration_since(Instant::now())) {
        Ok(bytes) => Ok(bytes),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "timed out after {} seconds: a background process kept the output open",
                limit.as_secs()
            ),
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(script: &str) -> Command {
        if cfg!(windows) {
            let mut command = Command::new("cmd.exe");
            command.args(["/d", "/c", script]);
            command
        } else {
            let mut command = Command::new("sh");
            command.args(["-c", script]);
            command
        }
    }

    #[test]
    fn collects_output_of_a_command_that_finishes() {
        let output = output_within(&mut shell("echo brachio"), Duration::from_secs(20))
            .expect("command runs");
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("brachio"));
    }

    #[test]
    fn stops_a_command_that_runs_too_long() {
        let script = if cfg!(windows) {
            "ping -n 30 127.0.0.1 >nul"
        } else {
            "sleep 30"
        };
        let started = Instant::now();
        let error = output_within(&mut shell(script), Duration::from_millis(300))
            .expect_err("must time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn returns_in_time_when_a_descendant_keeps_the_pipes_open() {
        let script = if cfg!(windows) {
            "start /b ping -n 4 127.0.0.1 >nul & echo hi"
        } else {
            "sleep 3 & echo hi"
        };
        let started = Instant::now();
        let result = output_within(&mut shell(script), Duration::from_millis(500));
        assert!(started.elapsed() < Duration::from_secs(2));
        if let Err(error) = result {
            assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        }
    }

    #[test]
    fn stops_a_command_that_prints_more_than_the_cap() {
        let script = if cfg!(windows) {
            "for /L %i in (1,1,200000) do @echo line %i"
        } else {
            "yes line"
        };
        let started = Instant::now();
        let capped = output_capped(&mut shell(script), Duration::from_secs(20), 1_000)
            .expect("command runs");
        assert!(capped.truncated);
        assert!(capped.output.stdout.len() <= 1_001 + 16 * 1024);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn a_command_that_floods_standard_error_finishes_and_only_a_start_of_it_is_kept() {
        // Long lines keep the iteration count low, so run time barely depends on machine load.
        let pad = "x".repeat(500);
        let script = if cfg!(windows) {
            format!("for /L %i in (1,1,6000) do @echo {pad} 1>&2")
        } else {
            format!("i=0; while [ $i -lt 6000 ]; do echo {pad} >&2; i=$((i+1)); done")
        };
        let output =
            output_within(&mut shell(&script), Duration::from_secs(120)).expect("finishes");
        assert!(output.status.success());
        assert_eq!(output.stderr.len(), KEPT_STDERR);
    }

    #[test]
    fn streams_a_large_output_in_pieces_without_losing_any_of_it() {
        let script = if cfg!(windows) {
            "for /L %i in (1,1,30000) do @echo line %i"
        } else {
            "i=1; while [ $i -le 30000 ]; do echo line $i; i=$((i+1)); done"
        };
        let mut seen = Vec::new();
        let mut largest = 0;
        let streamed = stream_stdout(&mut shell(script), Duration::from_secs(60), |chunk| {
            largest = largest.max(chunk.len());
            seen.extend_from_slice(chunk);
        })
        .expect("command runs");
        assert!(streamed.status.success());
        assert!(largest <= 64 * 1024);
        let text = String::from_utf8_lossy(&seen);
        assert_eq!(text.lines().count(), 30000);
        assert!(text.contains("line 30000"));
    }

    #[test]
    fn a_streamed_command_that_runs_too_long_is_stopped() {
        let script = if cfg!(windows) {
            "ping -n 30 127.0.0.1 >nul"
        } else {
            "sleep 30"
        };
        let error = stream_stdout(&mut shell(script), Duration::from_millis(300), |_| {})
            .err()
            .expect("must time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }

    #[test]
    fn a_command_waiting_for_input_ends_instead_of_hanging() {
        let script = if cfg!(windows) {
            "set /p answer="
        } else {
            "read answer"
        };
        output_within(&mut shell(script), Duration::from_secs(20))
            .expect("returns once stdin is closed");
    }

    #[test]
    fn reports_standard_error_as_it_arrives_and_keeps_its_end() {
        use std::sync::Mutex;
        let script = if cfg!(windows) {
            "echo first 1>&2 & echo last 1>&2"
        } else {
            "echo first >&2; echo last >&2"
        };
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let reported = run_tree_reporting(
            shell(script),
            Duration::from_secs(20),
            &AtomicBool::new(false),
            move |chunk| sink.lock().expect("lock").extend_from_slice(chunk),
        )
        .expect("command runs");
        assert!(matches!(reported.ended, Ended::Exited(status) if status.success()));
        let text = String::from_utf8_lossy(&reported.stderr_tail).into_owned();
        assert!(text.contains("first") && text.contains("last"));
        assert_eq!(
            seen.lock().expect("lock").as_slice(),
            reported.stderr_tail.as_slice()
        );
    }

    #[test]
    fn a_reported_run_stops_when_cancelled() {
        let script = if cfg!(windows) {
            "ping -n 30 127.0.0.1 >nul"
        } else {
            "sleep 30"
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(200));
            flag.store(true, Ordering::Release);
        });
        let started = Instant::now();
        let reported = run_tree_reporting(shell(script), Duration::from_secs(60), &cancel, |_| {})
            .expect("command runs");
        assert!(matches!(reported.ended, Ended::Cancelled));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    fn process_is_alive(pid: &str) -> bool {
        if cfg!(windows) {
            let listing = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output()
                .expect("tasklist runs");
            String::from_utf8_lossy(&listing.stdout).contains(pid)
        } else {
            Command::new("kill")
                .args(["-0", pid])
                .stderr(Stdio::null())
                .status()
                .expect("kill runs")
                .success()
        }
    }

    fn wait_for_pid_file(path: &std::path::Path) -> String {
        // PowerShell can take many seconds to start on a busy CI runner; this only detects a stall.
        let deadline = Instant::now() + Duration::from_secs(90);
        loop {
            if let Ok(text) = std::fs::read_to_string(path) {
                let pid = text.trim().trim_start_matches('\u{feff}').to_owned();
                if !pid.is_empty() {
                    return pid;
                }
            }
            assert!(Instant::now() < deadline, "the grandchild never started");
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn cancelling_kills_the_whole_tree() {
        let pid_file =
            std::env::temp_dir().join(format!("brachio-process-cancel-{}", std::process::id()));
        let _ = std::fs::remove_file(&pid_file);
        let path = pid_file.display().to_string();
        let command = if cfg!(windows) {
            let script = format!(
                "$p = Start-Process -PassThru -WindowStyle Hidden ping -ArgumentList '-n','60','127.0.0.1'; Set-Content -Path '{path}' -Value $p.Id; Wait-Process -Id $p.Id"
            );
            let mut command = Command::new("powershell.exe");
            command.args(["-NoProfile", "-Command", &script]);
            command
        } else {
            let script = format!("sh -c 'sleep 60 & echo $! > \"{path}\"; wait'");
            let mut command = Command::new("sh");
            command.args(["-c", &script]);
            command
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let runner = {
            let cancel = Arc::clone(&cancel);
            thread::spawn(move || {
                run_tree_cancellable(command, false, Duration::from_secs(120), &cancel)
            })
        };
        let pid = wait_for_pid_file(&pid_file);
        cancel.store(true, Ordering::Release);
        let ended = runner.join().expect("runner thread").expect("run");
        assert!(matches!(ended, Ended::Cancelled), "{ended:?}");
        let deadline = Instant::now() + Duration::from_secs(10);
        while process_is_alive(&pid) {
            assert!(
                Instant::now() < deadline,
                "grandchild {pid} survived the cancel"
            );
            thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_file(&pid_file);
    }

    #[test]
    fn a_timeout_kills_the_grandchild_too() {
        let pid_file =
            std::env::temp_dir().join(format!("brachio-process-tree-{}", std::process::id()));
        let _ = std::fs::remove_file(&pid_file);
        let path = pid_file.display().to_string();
        let (command, limit) = if cfg!(windows) {
            let script = format!(
                "$p = Start-Process -PassThru -WindowStyle Hidden ping -ArgumentList '-n','60','127.0.0.1'; Set-Content -Path '{path}' -Value $p.Id; Wait-Process -Id $p.Id"
            );
            let mut command = Command::new("powershell.exe");
            command.args(["-NoProfile", "-Command", &script]);
            (command, Duration::from_secs(8))
        } else {
            let script = format!("sh -c 'sleep 60 & echo $! > \"{path}\"; wait'");
            let mut command = Command::new("sh");
            command.args(["-c", &script]);
            (command, Duration::from_secs(2))
        };
        let error = output_capped_tree(command, limit, 1024)
            .err()
            .expect("must time out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        let pid = wait_for_pid_file(&pid_file);
        let deadline = Instant::now() + Duration::from_secs(10);
        while process_is_alive(&pid) {
            assert!(
                Instant::now() < deadline,
                "grandchild {pid} survived the timeout"
            );
            thread::sleep(Duration::from_millis(100));
        }
        let _ = std::fs::remove_file(&pid_file);
    }
}
