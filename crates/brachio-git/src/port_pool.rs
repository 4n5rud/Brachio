//! Port blocks for parallel dev servers.

use std::collections::BTreeSet;

/// A contiguous run of ports handed to one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortBlock {
    /// First port of the block.
    pub base: u16,
    /// Number of ports in the block.
    pub len: u16,
}

/// Hands out port blocks so parallel dev servers never collide.
///
/// `is_free` is injected: production code tries to bind every port in the
/// block; tests pass a closure.
#[derive(Debug, Clone)]
pub struct PortPool {
    start: u16,
    block_len: u16,
    max_blocks: u16,
    taken: BTreeSet<u16>,
}

impl PortPool {
    /// A pool of `max_blocks` blocks of `block_len` ports from `start`.
    #[must_use]
    pub fn new(start: u16, block_len: u16, max_blocks: u16) -> Self {
        Self {
            start,
            block_len,
            max_blocks,
            taken: BTreeSet::new(),
        }
    }

    /// Marks a block from storage as taken (on app start).
    pub fn restore(&mut self, block: PortBlock) {
        self.taken.insert(block.base);
    }

    /// Returns the first nonempty block that is neither taken nor busy on the host.
    pub fn allocate(&mut self, is_free: impl Fn(PortBlock) -> bool) -> Option<PortBlock> {
        if self.block_len == 0 {
            return None;
        }
        let block = (0..self.max_blocks)
            .filter_map(|index| self.start.checked_add(index.checked_mul(self.block_len)?))
            .map(|base| PortBlock {
                base,
                len: self.block_len,
            })
            .filter(|block| block.base.checked_add(block.len - 1).is_some())
            .find(|block| !self.taken.contains(&block.base) && is_free(*block))?;
        self.taken.insert(block.base);
        Some(block)
    }

    /// Returns a block to the pool.
    pub fn release(&mut self, block: PortBlock) {
        self.taken.remove(&block.base);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_consecutive_blocks() {
        let mut pool = PortPool::new(41_000, 10, 4);
        assert_eq!(
            pool.allocate(|_| true),
            Some(PortBlock {
                base: 41_000,
                len: 10
            })
        );
        assert_eq!(
            pool.allocate(|_| true),
            Some(PortBlock {
                base: 41_010,
                len: 10
            })
        );
    }

    #[test]
    fn skips_blocks_busy_on_host_and_reuses_released() {
        let mut pool = PortPool::new(41_000, 10, 4);
        let first = pool
            .allocate(|block| block.base != 41_000)
            .expect("a later block is free");
        assert_eq!(first.base, 41_010);
        pool.release(first);
        let again = pool
            .allocate(|block| block.base != 41_000)
            .expect("the released block is free again");
        assert_eq!(again.base, 41_010);
    }

    #[test]
    fn returns_none_when_exhausted() {
        let mut pool = PortPool::new(41_000, 10, 1);
        assert!(pool.allocate(|_| true).is_some(), "the only block is free");
        assert_eq!(pool.allocate(|_| true), None);
    }

    #[test]
    fn accepts_block_ending_at_max_port() {
        let mut pool = PortPool::new(u16::MAX, 1, 1);
        assert_eq!(
            pool.allocate(|_| true),
            Some(PortBlock {
                base: u16::MAX,
                len: 1,
            })
        );
    }

    #[test]
    fn rejects_zero_length_blocks() {
        let mut pool = PortPool::new(41_000, 0, 3);
        assert_eq!(pool.allocate(|_| true), None);
    }

    #[test]
    fn rejects_block_extending_past_max_port() {
        let mut pool = PortPool::new(u16::MAX, 2, 1);
        assert_eq!(pool.allocate(|_| true), None);
    }

    #[test]
    fn never_overflows_u16() {
        let mut pool = PortPool::new(65_530, 10, 3);
        assert_eq!(pool.allocate(|_| true), None);
    }
}
