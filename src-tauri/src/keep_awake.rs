//! Keeps the computer from sleeping while agents work.
//!
//! The hold lives on one dedicated thread: Windows ties the request to the thread that made it,
//! and the operating system drops it when the app exits.

use std::sync::mpsc;

use tauri::State;

type Reply = mpsc::Sender<Result<(), String>>;

/// Sends the wish to hold or release to the thread that owns the hold.
pub struct KeepAwake(mpsc::Sender<(bool, Reply)>);

impl KeepAwake {
    pub fn start() -> Self {
        Self::with(acquire)
    }

    fn with<Hold: Send + 'static>(
        acquire: impl Fn() -> Result<Hold, String> + Send + 'static,
    ) -> Self {
        let (sender, receiver) = mpsc::channel::<(bool, Reply)>();
        std::thread::spawn(move || {
            let mut hold = None;
            for (wanted, reply) in receiver {
                let outcome = match (wanted, hold.is_some()) {
                    (true, false) => acquire().map(|acquired| {
                        hold = Some(acquired);
                    }),
                    (false, true) => {
                        hold = None;
                        Ok(())
                    }
                    _ => Ok(()),
                };
                let _ = reply.send(outcome);
            }
        });
        Self(sender)
    }

    fn set(&self, wanted: bool) -> Result<(), String> {
        let (reply, answer) = mpsc::channel();
        self.0
            .send((wanted, reply))
            .map_err(|_| "The keep-awake thread stopped.".to_owned())?;
        answer
            .recv()
            .map_err(|_| "The keep-awake thread stopped.".to_owned())?
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn acquire() -> Result<keepawake::KeepAwake, String> {
    keepawake::Builder::default()
        .idle(true)
        .sleep(true)
        .reason("An agent is working")
        .app_name("Brachio")
        .app_reverse_domain("io.github.4n5rud.brachio")
        .create()
        .map_err(|error| error.to_string())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn acquire() -> Result<(), String> {
    Err("Keeping the computer awake is not supported on this system.".to_owned())
}

/// Whether this system can be kept awake, so the interface offers the setting only where it works.
#[tauri::command]
pub fn keep_awake_supported() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

/// Holds the system awake while `on` is true and releases the hold otherwise.
#[tauri::command]
pub fn keep_awake_set(keep: State<'_, KeepAwake>, on: bool) -> Result<(), String> {
    keep.set(on)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct Counted(Arc<AtomicUsize>);

    impl Drop for Counted {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn the_hold_is_taken_once_and_dropped_when_no_longer_wanted() {
        let taken = Arc::new(AtomicUsize::new(0));
        let released = Arc::new(AtomicUsize::new(0));
        let (taken_in, released_in) = (taken.clone(), released.clone());
        let keep = KeepAwake::with(move || {
            taken_in.fetch_add(1, Ordering::SeqCst);
            Ok(Counted(released_in.clone()))
        });
        keep.set(true).expect("hold");
        keep.set(true).expect("hold again");
        assert_eq!(
            (
                taken.load(Ordering::SeqCst),
                released.load(Ordering::SeqCst)
            ),
            (1, 0)
        );
        keep.set(false).expect("release");
        keep.set(false).expect("release again");
        assert_eq!(released.load(Ordering::SeqCst), 1);
        keep.set(true).expect("hold after release");
        assert_eq!(taken.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_failed_hold_is_reported_and_can_be_retried() {
        let keep = KeepAwake::with(|| -> Result<(), String> { Err("no power api".to_owned()) });
        assert_eq!(keep.set(true), Err("no power api".to_owned()));
        assert_eq!(keep.set(false), Ok(()));
    }
}
