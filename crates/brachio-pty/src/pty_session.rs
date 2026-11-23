//! One child process running inside a pseudoterminal.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::Arc;
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

/// Variables an agent session sets for the programs it starts. When the app was itself started
/// from such a session, they would reach every terminal: Claude Code there would take itself for a
/// child of that other session, stop saving its transcript, and see the other session's messaging
/// socket. A variable the spawn spec sets on purpose is still applied.
const AGENT_SESSION_VARIABLES: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SSE_PORT",
    "CLAUDE_EFFORT",
    "CLAUDE_PID",
];

/// Drops the inherited markers of an agent session, leaving settings such as
/// `CLAUDE_CODE_USE_BEDROCK` alone.
fn without_agent_session(command: &mut CommandBuilder) {
    for name in AGENT_SESSION_VARIABLES {
        command.env_remove(name);
    }
}

/// How many writes may wait for a child that is not reading its input. Typing and pastes are far
/// fewer; a child that stays deaf past this is not going to read what is queued anyway.
const QUEUED_WRITES: usize = 512;
const MAX_INPUT_WRITE: usize = 64 * 1024;
/// Includes the frame currently being written to the child.
const MAX_PENDING_INPUT: usize = 1024 * 1024;

#[derive(Default)]
struct InputBudget {
    writes: AtomicUsize,
    bytes: AtomicUsize,
}

struct InputReservation {
    budget: Arc<InputBudget>,
    bytes: usize,
}

impl InputBudget {
    fn reserve(self: &Arc<Self>, bytes: usize) -> Result<InputReservation> {
        if bytes > MAX_INPUT_WRITE {
            return Err(anyhow!(
                "pty input exceeds the {MAX_INPUT_WRITE}-byte write limit"
            ));
        }
        self.writes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count < QUEUED_WRITES).then_some(count + 1)
            })
            .map_err(|_| anyhow!("the program is not reading its input"))?;
        let mut reservation = InputReservation {
            budget: Arc::clone(self),
            bytes: 0,
        };
        self.bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count + bytes <= MAX_PENDING_INPUT).then_some(count + bytes)
            })
            .map_err(|_| anyhow!("the program is not reading its input"))?;
        reservation.bytes = bytes;
        Ok(reservation)
    }
}

impl Drop for InputReservation {
    fn drop(&mut self) {
        self.budget.bytes.fetch_sub(self.bytes, Ordering::Relaxed);
        self.budget.writes.fetch_sub(1, Ordering::Relaxed);
    }
}

struct QueuedInput {
    bytes: Vec<u8>,
    _reservation: InputReservation,
}

/// One running PTY. Reading happens on a dedicated thread owned by the
/// caller (see `take_reader`), because PTY reads block.
pub struct PtySession {
    master: Box<dyn MasterPty + Send>,
    child: Box<dyn Child + Send + Sync>,
    /// Input waits here for the writer thread. A child that does not read its input blocks
    /// that thread only, never a caller, the session lock or the output reader.
    input: SyncSender<QueuedInput>,
    input_budget: Arc<InputBudget>,
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
        command.cwd(without_verbatim_prefix(&spec.cwd));
        without_agent_session(&mut command);
        for (name, value) in &spec.env {
            command.env(name, value);
        }
        let child = pair
            .slave
            .spawn_command(command)
            .context("spawn program in pty")?;
        let writer = pair.master.take_writer().context("take pty writer")?;
        let (input, queued) = mpsc::sync_channel(QUEUED_WRITES);
        thread::Builder::new()
            .name("pty-write".to_owned())
            .spawn(move || write_queued(writer, &queued))
            .context("start pty writer")?;
        Ok(Self {
            master: pair.master,
            child,
            input,
            input_budget: Arc::default(),
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
    /// Fails when the PTY is closed, or when so much input is already waiting that the child is
    /// evidently not reading it, or when the write exceeds the per-frame limit.
    pub fn write_input(&self, bytes: &[u8]) -> Result<()> {
        let reservation = self.input_budget.reserve(bytes.len())?;
        self.input
            .try_send(QueuedInput {
                bytes: bytes.to_vec(),
                _reservation: reservation,
            })
            .map_err(|error| match error {
                TrySendError::Full(_) => anyhow!("the program is not reading its input"),
                TrySendError::Disconnected(_) => anyhow!("pty input is closed"),
            })
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
fn write_queued(mut writer: Box<dyn Write + Send>, queued: &Receiver<QueuedInput>) {
    for input in queued {
        if writer.write_all(&input.bytes).is_err() {
            return;
        }
    }
}

/// `canonicalize` on Windows gives `\\?\C:\dir`, which `cmd.exe` and PowerShell refuse as a working
/// directory (they fall back to the Windows folder). Drive and share paths are given back in the
/// ordinary form; every other path is left alone.
#[must_use]
pub fn without_verbatim_prefix(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let mut characters = rest.chars();
        let drive = characters
            .next()
            .is_some_and(|letter| letter.is_ascii_alphabetic());
        if drive && characters.next() == Some(':') {
            return PathBuf::from(rest);
        }
    }
    path.to_path_buf()
}

fn size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[cfg(test)]
mod path_tests {
    use std::path::{Path, PathBuf};

    use super::without_verbatim_prefix;

    #[test]
    fn a_verbatim_drive_path_becomes_an_ordinary_one_and_other_paths_stay() {
        let plain = |text: &str| without_verbatim_prefix(Path::new(text));
        assert_eq!(plain(r"\\?\C:\work\repo"), PathBuf::from(r"C:\work\repo"));
        assert_eq!(
            plain(r"\\?\UNC\server\share\repo"),
            PathBuf::from(r"\\server\share\repo")
        );
        assert_eq!(plain(r"C:\work\repo"), PathBuf::from(r"C:\work\repo"));
        assert_eq!(plain("/home/me/repo"), PathBuf::from("/home/me/repo"));
        assert_eq!(plain(r"\\?\Volume{1}\x"), PathBuf::from(r"\\?\Volume{1}\x"));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn agent_session_markers_from_the_app_do_not_reach_terminals() {
        let mut command = CommandBuilder::new("/bin/sh");
        for name in AGENT_SESSION_VARIABLES {
            command.env(name, "from-the-app");
        }
        command.env("CLAUDE_CODE_USE_BEDROCK", "1");
        without_agent_session(&mut command);
        for name in AGENT_SESSION_VARIABLES {
            assert_eq!(command.get_env(name), None, "{name}");
        }
        assert_eq!(
            command.get_env("CLAUDE_CODE_USE_BEDROCK"),
            Some(std::ffi::OsStr::new("1"))
        );
    }

    #[test]
    fn a_spawned_program_does_not_see_an_inherited_session_marker() {
        let spec = SpawnSpec {
            program: "/bin/sh".to_owned(),
            args: vec![
                "-c".to_owned(),
                "echo \"marker=[${CLAUDE_CODE_CHILD_SESSION}] kept=[${BRACHIO_KEPT}]\"".to_owned(),
            ],
            cwd: std::env::temp_dir(),
            env: vec![("BRACHIO_KEPT".to_owned(), "yes".to_owned())],
            rows: 24,
            cols: 80,
        };
        let session = PtySession::spawn(&spec).expect("spawn sh");
        let mut reader = session.take_reader().expect("take reader");
        let mut output = Vec::new();
        let mut chunk = [0u8; 256];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read]);
            if String::from_utf8_lossy(&output).contains("kept=[yes]") {
                break;
            }
        }
        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("marker=[] kept=[yes]"), "{text}");
    }

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

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn a_stalled_writer_keeps_its_frame_inside_the_budget() {
        let budget = Arc::<InputBudget>::default();
        let (sender, receiver) = mpsc::sync_channel(QUEUED_WRITES);
        let make_frame = || QueuedInput {
            bytes: vec![0; MAX_INPUT_WRITE],
            _reservation: budget.reserve(MAX_INPUT_WRITE).expect("reserve frame"),
        };
        sender.send(make_frame()).expect("queue first frame");
        let in_flight = receiver.recv().expect("writer takes first frame");
        for _ in 1..MAX_PENDING_INPUT / MAX_INPUT_WRITE {
            sender.send(make_frame()).expect("queue frame");
        }
        assert!(budget.reserve(1).is_err());
        assert!(budget.reserve(MAX_INPUT_WRITE + 1).is_err());
        assert!(Arc::new(InputBudget::default())
            .reserve(MAX_INPUT_WRITE)
            .is_ok());
        drop(in_flight);
        sender.send(make_frame()).expect("released budget");
        drop(receiver);
        assert_eq!(budget.bytes.load(Ordering::Relaxed), 0);
        assert_eq!(budget.writes.load(Ordering::Relaxed), 0);
    }
}
