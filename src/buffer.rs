//! Pending-run buffer: 32 inline slots, with a heap spill that is allocated
//! once and then reused for the rest of the iterator's life.

use alloc::vec::Vec;

/// Inline capacity. A run head, the 30 non-starters UAX15-D4 stream-safe
/// text allows after it, and the starter that closes the run all fit.
pub(crate) const INLINE: usize = 32;

#[derive(Clone, Debug)]
pub(crate) struct RunBuffer<T> {
    inline: [T; INLINE],
    len: usize,
    spill: Vec<T>,
}

impl<T: Copy + Default> RunBuffer<T> {
    pub(crate) fn new() -> Self {
        RunBuffer {
            inline: [T::default(); INLINE],
            len: 0,
            spill: Vec::new(),
        }
    }

    #[inline]
    fn spilled(&self) -> bool {
        self.spill.capacity() != 0
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        if self.spilled() {
            self.spill.len()
        } else {
            self.len
        }
    }

    #[inline]
    pub(crate) fn push(&mut self, item: T) {
        if self.spilled() {
            self.spill.push(item);
        } else if let Some(slot) = self.inline.get_mut(self.len) {
            *slot = item;
            self.len += 1;
        } else {
            let mut spill = Vec::with_capacity(INLINE * 2);
            spill.extend_from_slice(&self.inline);
            spill.push(item);
            self.spill = spill;
        }
    }

    /// Pushes unless that would take new memory: a full inline buffer, or
    /// a spill at capacity. Returns false in that case, so the caller can
    /// reclaim dead items first and only grow when live items need the
    /// space.
    #[inline]
    pub(crate) fn try_push(&mut self, item: T) -> bool {
        if self.spilled() {
            if self.spill.len() == self.spill.capacity() {
                return false;
            }
            self.spill.push(item);
        } else if let Some(slot) = self.inline.get_mut(self.len) {
            *slot = item;
            self.len += 1;
        } else {
            return false;
        }
        true
    }

    /// Drops the first `n` items and moves the rest to the front. The
    /// caller has already emitted those items. Never allocates.
    #[inline]
    pub(crate) fn compact_front(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        if self.spilled() {
            let len = self.spill.len();
            self.spill.copy_within(n.., 0);
            self.spill.truncate(len - n);
        } else {
            self.inline.copy_within(n..self.len, 0);
            self.len -= n;
        }
    }

    /// Forgets every item. Keeps the spill allocation.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.len = 0;
        self.spill.clear();
    }

    #[inline]
    pub(crate) fn get(&self, i: usize) -> Option<&T> {
        self.as_slice().get(i)
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[T] {
        if self.spilled() {
            &self.spill
        } else {
            self.inline.get(..self.len).unwrap_or(&[])
        }
    }

    #[inline]
    pub(crate) fn as_mut_slice(&mut self) -> &mut [T] {
        if self.spilled() {
            &mut self.spill
        } else {
            self.inline.get_mut(..self.len).unwrap_or(&mut [])
        }
    }
}
