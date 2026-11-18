//! Coalescing PTY output into frames.

use std::mem;

/// When to flush buffered PTY output to the web view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatchConfig {
    /// Flush after this long since the first unflushed byte.
    pub max_delay_ms: u64,
    /// Flush as soon as this many bytes are buffered.
    pub max_bytes: usize,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_delay_ms: 16,
            max_bytes: 64 * 1024,
        }
    }
}

/// Coalesces PTY output into frames so the IPC channel sends few large
/// messages instead of many small ones. Pure: the caller supplies the clock.
///
/// Output after a quiet period goes out at once, so a keystroke echo is not
/// held back. Output that keeps streaming is held for up to `max_delay_ms`
/// and sent as one frame.
#[derive(Debug)]
pub struct OutputBatcher {
    config: BatchConfig,
    buffer: Vec<u8>,
    first_byte_ms: Option<u64>,
    last_flush_ms: Option<u64>,
}

impl OutputBatcher {
    /// An empty batcher.
    #[must_use]
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            buffer: Vec::with_capacity(config.max_bytes),
            first_byte_ms: None,
            last_flush_ms: None,
        }
    }

    /// Buffers `bytes`. Returns a frame right away after a quiet period, or
    /// when the size limit is reached.
    pub fn push(&mut self, bytes: &[u8], now_ms: u64) -> Option<Vec<u8>> {
        if bytes.is_empty() {
            return None;
        }
        let was_empty = self.buffer.is_empty();
        self.first_byte_ms.get_or_insert(now_ms);
        self.buffer.extend_from_slice(bytes);
        let full = self.buffer.len() >= self.config.max_bytes;
        let after_quiet = was_empty && self.quiet_since_last_flush(now_ms);
        (full || after_quiet).then(|| self.take(now_ms))
    }

    /// Returns a frame when the oldest buffered byte has waited long enough.
    pub fn tick(&mut self, now_ms: u64) -> Option<Vec<u8>> {
        let first = self.first_byte_ms?;
        let waited = now_ms.saturating_sub(first) >= self.config.max_delay_ms;
        waited.then(|| self.take(now_ms))
    }

    fn quiet_since_last_flush(&self, now_ms: u64) -> bool {
        match self.last_flush_ms {
            Some(last) => now_ms.saturating_sub(last) >= self.config.max_delay_ms,
            None => true,
        }
    }

    fn take(&mut self, now_ms: u64) -> Vec<u8> {
        self.first_byte_ms = None;
        self.last_flush_ms = Some(now_ms);
        let empty = Vec::with_capacity(self.config.max_bytes);
        mem::replace(&mut self.buffer, empty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: BatchConfig = BatchConfig {
        max_delay_ms: 16,
        max_bytes: 8,
    };

    #[test]
    fn output_after_quiet_goes_out_at_once() {
        let mut batcher = OutputBatcher::new(CONFIG);
        assert_eq!(batcher.push(b"a", 0), Some(b"a".to_vec()));
        assert_eq!(batcher.push(b"b", 100), Some(b"b".to_vec()));
    }

    #[test]
    fn streaming_output_is_held_until_the_delay() {
        let mut batcher = OutputBatcher::new(CONFIG);
        batcher.push(b"a", 0);

        assert_eq!(batcher.push(b"b", 5), None);
        assert_eq!(batcher.push(b"c", 10), None);
        assert_eq!(batcher.tick(20), None);
        assert_eq!(batcher.tick(21), Some(b"bc".to_vec()));
    }

    #[test]
    fn flushes_on_size() {
        let mut batcher = OutputBatcher::new(CONFIG);
        batcher.push(b"x", 0);

        assert_eq!(batcher.push(b"abcd", 1), None);
        assert_eq!(batcher.push(b"efgh", 2), Some(b"abcdefgh".to_vec()));
        assert_eq!(batcher.tick(100), None);
    }

    #[test]
    fn empty_input_starts_no_timer() {
        let mut batcher = OutputBatcher::new(CONFIG);
        assert_eq!(batcher.push(b"", 0), None);
        assert_eq!(batcher.tick(1_000), None);
    }
}
