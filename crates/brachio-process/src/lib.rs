//! Short-lived helper programs (`git`, `gh`, `--version` probes) run for a bounded time.

use std::io::{self, Read};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const POLL_EVERY: Duration = Duration::from_millis(10);

/// Runs `command` and collects its output, stopping it after `limit`.
///
/// The child gets no stdin and, on Windows, no console window, so it can neither wait for a
/// typed answer nor flash a black window in the packaged app.
///
/// # Errors
/// Fails when the program cannot start, or with `ErrorKind::TimedOut` after the child was killed
/// for running longer than `limit`.
pub fn output_within(command: &mut Command, limit: Duration) -> io::Result<Output> {
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
    let stdout = child.stdout.take().map(drain);
    let stderr = child.stderr.take().map(drain);
    let deadline = Instant::now() + limit;
    let status = loop {
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
    Ok(Output {
        status,
        stdout: collect(stdout),
        stderr: collect(stderr),
    })
}

fn drain(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        bytes
    })
}

fn collect(reader: Option<thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default()
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
