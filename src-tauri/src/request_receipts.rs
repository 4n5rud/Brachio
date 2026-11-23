//! Short-lived receipts for commands that create things, so a double click, a retry or a window
//! that reconnects runs the work once and every repeat gets the first answer.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::OnceCell;

/// How long a finished answer is kept for repeats.
const KEEP: Duration = Duration::from_secs(120);
/// Receipts kept at most, so a runaway client cannot grow the map.
const MAX_RECEIPTS: usize = 512;

struct Slot<T> {
    cell: Arc<OnceCell<T>>,
    created: Instant,
}

/// Remembers the answer for each client-chosen request id.
pub struct RequestReceipts<T> {
    slots: Mutex<HashMap<String, Slot<T>>>,
}

impl<T> Default for RequestReceipts<T> {
    fn default() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
        }
    }
}

impl<T: Clone> RequestReceipts<T> {
    /// Runs `work` for the first caller with this id. A repeat that arrives while it runs waits
    /// for that run, and a repeat that arrives after it succeeded gets its answer. A failed run
    /// leaves nothing behind, so retrying the same id runs the work again.
    ///
    /// # Errors
    /// Whatever `work` returns, or a message when the receipt table is poisoned.
    pub async fn once<F, Fut>(&self, request_id: &str, work: F) -> Result<T, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let cell = self.slot(request_id)?;
        let result = cell.get_or_try_init(work).await.cloned();
        if result.is_err() {
            self.forget_if_empty(request_id, &cell);
        }
        result
    }

    fn slot(&self, request_id: &str) -> Result<Arc<OnceCell<T>>, String> {
        let mut slots = self
            .slots
            .lock()
            .map_err(|_| "The request table is unavailable.".to_owned())?;
        let now = Instant::now();
        slots.retain(|_, slot| now.duration_since(slot.created) < KEEP);
        if slots.len() >= MAX_RECEIPTS && !slots.contains_key(request_id) {
            if let Some(oldest) = slots
                .iter()
                .min_by_key(|(_, slot)| slot.created)
                .map(|(key, _)| key.clone())
            {
                slots.remove(&oldest);
            }
        }
        let slot = slots.entry(request_id.to_owned()).or_insert_with(|| Slot {
            cell: Arc::new(OnceCell::new()),
            created: now,
        });
        Ok(Arc::clone(&slot.cell))
    }

    fn forget_if_empty(&self, request_id: &str, cell: &Arc<OnceCell<T>>) {
        if let Ok(mut slots) = self.slots.lock() {
            if slots
                .get(request_id)
                .is_some_and(|slot| Arc::ptr_eq(&slot.cell, cell) && !cell.initialized())
            {
                slots.remove(request_id);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[tokio::test]
    async fn a_repeated_request_runs_once_and_gets_the_first_answer() {
        let receipts = RequestReceipts::<u32>::default();
        let runs = AtomicUsize::new(0);
        let first = receipts
            .once("a", || async {
                runs.fetch_add(1, Ordering::SeqCst);
                Ok(7)
            })
            .await
            .unwrap();
        let again = receipts
            .once("a", || async {
                runs.fetch_add(1, Ordering::SeqCst);
                Ok(8)
            })
            .await
            .unwrap();
        assert_eq!((first, again, runs.load(Ordering::SeqCst)), (7, 7, 1));
    }

    #[tokio::test]
    async fn two_clicks_at_once_share_one_run() {
        let receipts = RequestReceipts::<u32>::default();
        let runs = AtomicUsize::new(0);
        let slow = || async {
            runs.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(50)).await;
            Ok(1)
        };
        let (one, two) = tokio::join!(receipts.once("a", slow), receipts.once("a", slow));
        assert_eq!((one.unwrap(), two.unwrap()), (1, 1));
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_request_can_be_retried_with_the_same_id() {
        let receipts = RequestReceipts::<u32>::default();
        let failed = receipts
            .once("a", || async { Err("disk full".to_owned()) })
            .await;
        assert_eq!(failed, Err("disk full".to_owned()));
        let retried = receipts.once("a", || async { Ok(3) }).await;
        assert_eq!(retried, Ok(3));
    }

    #[tokio::test]
    async fn different_ids_do_not_share_answers() {
        let receipts = RequestReceipts::<u32>::default();
        assert_eq!(receipts.once("a", || async { Ok(1) }).await, Ok(1));
        assert_eq!(receipts.once("b", || async { Ok(2) }).await, Ok(2));
    }
}
