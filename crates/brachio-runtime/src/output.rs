//! Bounded byte history with an absolute offset for reconnection.

use std::collections::VecDeque;

const CAPACITY: usize = 256 * 1024;
const PAGE_SIZE: usize = 8 * 1024;

#[derive(Default)]
pub(crate) struct OutputHistory {
    bytes: VecDeque<u8>,
    end: u64,
}

pub(crate) struct Page {
    pub(crate) start: u64,
    pub(crate) next: u64,
    pub(crate) end: u64,
    pub(crate) truncated: bool,
    pub(crate) bytes: Vec<u8>,
}

impl OutputHistory {
    pub(crate) fn push(&mut self, bytes: &[u8]) {
        self.end += bytes.len() as u64;
        let bytes = &bytes[bytes.len().saturating_sub(CAPACITY)..];
        let excess = (self.bytes.len() + bytes.len()).saturating_sub(CAPACITY);
        self.bytes.drain(..excess);
        self.bytes.extend(bytes);
    }

    pub(crate) fn end(&self) -> u64 {
        self.end
    }

    pub(crate) fn read(&self, after: u64) -> Option<Page> {
        if after > self.end {
            return None;
        }
        let retained = self.end - self.bytes.len() as u64;
        let start = after.max(retained);
        let bytes: Vec<u8> = self
            .bytes
            .range((start - retained) as usize..)
            .take(PAGE_SIZE)
            .copied()
            .collect();
        Some(Page {
            start,
            next: start + bytes.len() as u64,
            end: self.end,
            truncated: after < retained,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{OutputHistory, CAPACITY, PAGE_SIZE};

    #[test]
    fn resumes_without_repeating_or_splitting_away_raw_bytes() {
        let mut history = OutputHistory::default();
        let first = vec![b'a'; PAGE_SIZE - 1];
        history.push(&first);
        history.push(&[0xeb]);
        let page = history.read(0).expect("first page");
        assert_eq!(page.bytes.last(), Some(&0xeb));
        history.push(&[0xb0, 0x94, 0x1b, b'[']);
        history.push(b"31m");
        let following = history.read(page.next).expect("following page");
        assert_eq!(following.bytes, b"\xb0\x94\x1b[31m");
        assert_eq!(following.next, history.end());
        assert_eq!(following.end, following.next);
        assert!(!following.truncated);
    }

    #[test]
    fn reports_history_lost_before_the_retention_window() {
        let mut history = OutputHistory::default();
        history.push(b"old");
        history.push(&vec![b'x'; CAPACITY + 7]);
        let page = history.read(0).expect("retained page");
        assert_eq!(page.start, 10);
        assert_eq!(page.next, 10 + PAGE_SIZE as u64);
        assert!(page.truncated);
        assert_eq!(page.bytes, vec![b'x'; PAGE_SIZE]);
        assert!(history.read(history.end() + 1).is_none());
    }
}
