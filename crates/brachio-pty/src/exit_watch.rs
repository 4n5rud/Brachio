//! Noticing that a PTY child has exited, on every platform.

use std::thread;
use std::time::{Duration, Instant};

use crate::session_manager::{lock_session, SharedSession};

const POLL_EVERY: Duration = Duration::from_millis(100);
const GRACE_PERIOD: Duration = Duration::from_secs(2);
const REAP_ATTEMPTS: u32 = 50;
const REAP_INTERVAL: Duration = Duration::from_millis(20);

/// Tracks child exit from the flusher thread.
///
/// On Windows the ConPTY reader does not see EOF when the child exits. It
/// ends only once the pseudoconsole is closed. So when a poll finds the child
/// gone, the watch drops its session handle, which closes the master and lets
/// the reader finish. If the reader still has not hung up after a grace
/// period, the flusher stops waiting for it.
pub(crate) struct ExitWatch {
    session: Option<SharedSession>,
    forget: Option<Box<dyn FnOnce() + Send>>,
    code: Option<u32>,
    exited_at: Option<Instant>,
    last_poll: Instant,
}

impl ExitWatch {
    /// `forget` removes the session from the manager's table.
    pub(crate) fn new(session: SharedSession, forget: Box<dyn FnOnce() + Send>) -> Self {
        Self {
            session: Some(session),
            forget: Some(forget),
            code: None,
            exited_at: None,
            last_poll: Instant::now(),
        }
    }

    /// True when the flusher should stop waiting for more output.
    pub(crate) fn should_stop(&mut self) -> bool {
        if let Some(exited_at) = self.exited_at {
            return exited_at.elapsed() >= GRACE_PERIOD;
        }
        if self.last_poll.elapsed() >= POLL_EVERY {
            self.last_poll = Instant::now();
            self.check_exit();
        }
        false
    }

    fn check_exit(&mut self) {
        let Some(code) = self.session.as_ref().and_then(read_exit_code) else {
            return;
        };
        self.code = Some(code);
        self.exited_at = Some(Instant::now());
        self.release();
        // Dropping the last handle closes the master, which ends the reader.
        self.session = None;
    }

    /// Removes the session from the manager's table, so this watch holds the
    /// last handle. Safe to call more than once.
    pub(crate) fn release(&mut self) {
        if let Some(forget) = self.forget.take() {
            forget();
        }
    }

    /// The exit code. When the reader hung up first (the usual case on
    /// Unix), waits briefly for the child to be reaped.
    pub(crate) fn finish(mut self) -> Option<u32> {
        let Some(session) = self.session.take() else {
            return self.code;
        };
        for _ in 0..REAP_ATTEMPTS {
            if let Some(code) = read_exit_code(&session) {
                return Some(code);
            }
            thread::sleep(REAP_INTERVAL);
        }
        None
    }
}

fn read_exit_code(session: &SharedSession) -> Option<u32> {
    let mut session = lock_session(session).ok()?;
    session.try_exit_code().ok().flatten()
}
