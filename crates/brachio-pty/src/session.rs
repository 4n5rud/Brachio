use std::io::{Read, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};

/// What to run inside a new PTY.
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
}

/// One running PTY. Reading happens on a dedicated thread owned by the
/// caller (see `take_reader`), because PTY reads block.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
}

impl PtySession {
    /// Spawns `spec.program` with an argument vector. No shell string is built.
    pub fn spawn(spec: &SpawnSpec) -> Result<Self> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: spec.rows,
                cols: spec.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("open pty")?;
        let mut cmd = CommandBuilder::new(&spec.program);
        cmd.args(&spec.args);
        cmd.cwd(&spec.cwd);
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        let child = pair
            .slave
            .spawn_command(cmd)
            .context("spawn agent in pty")?;
        let writer = pair.master.take_writer().context("take pty writer")?;
        Ok(Self {
            master: pair.master,
            child,
            writer,
        })
    }

    /// Returns a reader for the output thread. Call once.
    pub fn take_reader(&self) -> Result<Box<dyn Read + Send>> {
        self.master.try_clone_reader().context("clone pty reader")
    }

    pub fn write_input(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes).context("write pty input")
    }

    pub fn resize(&self, rows: u16, cols: u16) -> Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("resize pty")
    }

    /// `Some(code)` once the child has exited.
    pub fn try_exit_code(&mut self) -> Result<Option<u32>> {
        Ok(self
            .child
            .try_wait()
            .context("poll child")?
            .map(|s| s.exit_code()))
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child.kill().context("kill child")
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn runs_a_program_and_reads_its_output() {
        let spec = SpawnSpec {
            program: "/bin/echo".into(),
            args: vec!["hello-brachio".into()],
            cwd: std::env::temp_dir(),
            env: vec![],
            rows: 24,
            cols: 80,
        };
        let session = PtySession::spawn(&spec).unwrap();
        let mut reader = session.take_reader().unwrap();
        let mut out = Vec::new();
        let mut buf = [0u8; 256];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 {
                break;
            }
            out.extend_from_slice(&buf[..n]);
            if String::from_utf8_lossy(&out).contains("hello-brachio") {
                break;
            }
        }
        assert!(String::from_utf8_lossy(&out).contains("hello-brachio"));
    }
}
