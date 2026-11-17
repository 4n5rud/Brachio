use std::collections::BTreeSet;

/// A contiguous run of ports handed to one task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortBlock {
    pub base: u16,
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

    /// Returns the first block that is neither taken nor busy on the host.
    pub fn allocate(&mut self, is_free: impl Fn(PortBlock) -> bool) -> Option<PortBlock> {
        let block = (0..self.max_blocks)
            .filter_map(|i| self.start.checked_add(i.checked_mul(self.block_len)?))
            .map(|base| PortBlock {
                base,
                len: self.block_len,
            })
            .filter(|b| b.base.checked_add(b.len).is_some())
            .find(|b| !self.taken.contains(&b.base) && is_free(*b))?;
        self.taken.insert(block.base);
        Some(block)
    }

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
        let first = pool.allocate(|b| b.base != 41_000).unwrap();
        assert_eq!(first.base, 41_010);
        pool.release(first);
        assert_eq!(pool.allocate(|b| b.base != 41_000).unwrap().base, 41_010);
    }

    #[test]
    fn returns_none_when_exhausted() {
        let mut pool = PortPool::new(41_000, 10, 1);
        assert!(pool.allocate(|_| true).is_some());
        assert_eq!(pool.allocate(|_| true), None);
    }

    #[test]
    fn never_overflows_u16() {
        let mut pool = PortPool::new(65_530, 10, 3);
        assert_eq!(pool.allocate(|_| true), None);
    }
}
