//! One child process running inside a pseudoterminal.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use anyhow::{anyhow, Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};

/// What to run inside a new PTY.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    /// Program to execute: a path, or a name looked up on `PATH`.
    pub program: String,
    /// Arguments, one element per argument.
    pub args: Vec<String>,
    /// Working directory of the child.
    pub cwd: PathBuf,
    /// Extra environment variables on top of the app's own.
    pub env: Vec<(String, String)>,
    /// Initial height in rows.
    pub rows: u16,
    /// Initial width in columns.
    pub cols: u16,
}

/// One running PTY. Reading happens on a dedicated thread owned by the
/// caller (see `take_reader`), because PTY reads block.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Input waits here for the writer thread. A child that does not read its input blocks
    /// that thread only, never a caller, the session lock or the output reader.
    input: Sender<Vec<u8>>,
}

impl PtySession {
    /// Operating-system identifier for the child, when available.
    #[must_use]
    pub fn process_id(&self) -> Option<u32> {
        self.child.process_id()
    }

    /// Spawns `spec.program` with an argument vector. No shell string is built.
    ///
    /// # Errors
    /// Fails when the PTY cannot be opened or the program cannot be started.
    pub fn spawn(spec: &SpawnSpec) -> Result<Self> {
        let pair = portable_pty::native_pty_system()
            .openpty(size(spec.rows, spec.cols))
            .context("open pty")?;
        let mut command = CommandBuilder::new(&spec.program);
        command.args(&spec.args);
        command.cwd(&spec.cwd);
        for (name, value) in &spec.env {
            command.env(name, value);
        }
        let child = pair
            .slave
            .spawn_command(command)
            .context("spawn program in pty")?;
        let writer = pair.master.take_writer().context("take pty writer")?;
        let (input, queued) = mpsc::channel();
        thread::Builder::new()
            .name("pty-write".to_owned())
            .spawn(move || write_queued(writer, &queued))
            .context("start pty writer")?;
        Ok(Self {
            master: pair.master,
            child,
            input,
        })
    }

    /// Returns a reader for the output thread. Call once.
    ///
    /// # Errors
    /// Fails when the PTY handle cannot be duplicated.
    pub fn take_reader(&self) -> Result<Box<dyn Read + Send>> {
        self.master.try_clone_reader().context("clone pty reader")
    }

    /// Queues keyboard input for the child and returns at once. Input reaches the child in the
    /// order it was queued, whenever the child reads it.
    ///
    /// # Errors
    /// Fails when the PTY is closed.
    pub fn write_input(&self, bytes: &[u8]) -> Result<()> {
        self.input
            .send(bytes.to_vec())
            .map_err(|_| anyhow!("pty input is closed"))
    }

    /// Changes the terminal size the child sees.
    ///
    /// # Errors
    /// Fails when the PTY is closed.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.master.resize(size(rows, cols)).context("resize pty")
    }

    /// `Some(code)` once the child has exited.
    ///
    /// # Errors
    /// Fails when the platform cannot query the child.
    pub fn try_exit_code(&mut self) -> Result<Option<u32>> {
        let status = self.child.try_wait().context("poll child")?;
        Ok(status.map(|status| status.exit_code()))
    }

    /// Stops the child.
    ///
    /// # Errors
    /// Fails when the child cannot be signalled.
    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("kill child")
    }
}

/// Writes queued input until the PTY refuses it or every sender is gone.
fn write_queued(mut writer: Box<dyn Write + Send>, queued: &Receiver<Vec<u8>>) {
    for bytes in queued {
        if writer.write_all(&bytes).is_err() {
            return;
        }
    }
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn runs_a_program_and_reads_its_output() {
        let spec = SpawnSpec {
            program: "/bin/echo".to_owned(),
            args: vec!["hello-brachio".to_owned()],
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            rows: 24,
            cols: 80,
        };
        let session = PtySession::spawn(&spec).expect("spawn echo");
        let mut reader = session.take_reader().expect("take reader");

        let mut output = Vec::new();
        let mut chunk = [0u8; 256];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read]);
            if String::from_utf8_lossy(&output).contains("hello-brachio") {
                break;
            }
        }

        let text = String::from_utf8_lossy(&output);
        assert!(
            text.contains("hello-brachio"),
            "echo output missing: {text:?}"
        );
    }
}
