//! Short-lived helper programs (`git`, `gh`, `--version` probes) run for a bounded time.

use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
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
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let over = Arc::new(AtomicBool::new(false));
    let stdout = child
        .stdout
        .take()
        .map(|pipe| drain(pipe, max_stdout, Some(Arc::clone(&over))));
    let stderr = child
        .stderr
        .take()
        .map(|pipe| drain_keeping(pipe, KEPT_STDERR));
    let started = Instant::now();
    let deadline = started + limit;
    let status = loop {
        if over.load(Ordering::Acquire) {
            let _ = child.kill();
            break child.wait()?;
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            // Readers are left to finish by themselves: a grandchild may still hold the pipes.
            let _ = child.kill();
            let _ = child.wait();
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
        let script = if cfg!(windows) {
            "for /L %i in (1,1,150000) do @echo flooding standard error line %i 1>&2"
        } else {
            "i=0; while [ $i -lt 150000 ]; do echo flooding standard error line $i >&2; i=$((i+1)); done"
        };
        let output = output_within(&mut shell(script), Duration::from_secs(60)).expect("finishes");
        assert!(output.status.success());
        assert_eq!(output.stderr.len(), KEPT_STDERR);
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
}
