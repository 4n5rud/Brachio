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
#[derive(Debug)]
pub struct OutputBatcher {
    config: BatchConfig,
    buffer: Vec<u8>,
    first_byte_ms: Option<u64>,
}

impl OutputBatcher {
    /// An empty batcher.
    #[must_use]
    pub fn new(config: BatchConfig) -> Self {
        Self {
            config,
            buffer: Vec::with_capacity(config.max_bytes),
            first_byte_ms: None,
        }
    }

    /// Buffers `bytes`. Returns a frame when the size limit is reached.
    pub fn push(&mut self, bytes: &[u8], now_ms: u64) -> Option<Vec<u8>> {
        if bytes.is_empty() {
            return None;
        }
        self.first_byte_ms.get_or_insert(now_ms);
        self.buffer.extend_from_slice(bytes);
        (self.buffer.len() >= self.config.max_bytes).then(|| self.take())
    }

    /// Returns a frame when the oldest buffered byte has waited long enough.
    pub fn tick(&mut self, now_ms: u64) -> Option<Vec<u8>> {
        let first = self.first_byte_ms?;
        (now_ms.saturating_sub(first) >= self.config.max_delay_ms).then(|| self.take())
    }

    fn take(&mut self) -> Vec<u8> {
        self.first_byte_ms = None;
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
    fn flushes_on_size() {
        let mut batcher = OutputBatcher::new(CONFIG);
        assert_eq!(batcher.push(b"abcd", 0), None);
        assert_eq!(batcher.push(b"efgh", 1), Some(b"abcdefgh".to_vec()));
        assert_eq!(batcher.tick(100), None);
    }

    #[test]
    fn flushes_on_delay_measured_from_first_byte() {
        let mut batcher = OutputBatcher::new(CONFIG);
        batcher.push(b"a", 10);
        batcher.push(b"b", 20);
        assert_eq!(batcher.tick(25), None);
        assert_eq!(batcher.tick(26), Some(b"ab".to_vec()));
    }

    #[test]
    fn empty_input_starts_no_timer() {
        let mut batcher = OutputBatcher::new(CONFIG);
        assert_eq!(batcher.push(b"", 0), None);
        assert_eq!(batcher.tick(1_000), None);
    }
}
