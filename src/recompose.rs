//! Canonical composition on top of a decomposition.

use core::iter::FusedIterator;

use crate::buffer::RunBuffer;
use crate::decompose::Decompositions;
use crate::props::{self, hangul, Props};

#[derive(Clone, Copy, Debug)]
enum State {
    Composing,
    Purging(usize),
    Finished(usize),
}

/// Streaming NFC or NFKC. Yields `(char, isize)`: the output character and its
/// alignment change count. See the crate docs for the tag contract.
///
/// Created by [`UnicodeNormalization::nfc`](crate::UnicodeNormalization::nfc)
/// and [`UnicodeNormalization::nfkc`](crate::UnicodeNormalization::nfkc).
#[derive(Clone, Debug)]
pub struct Recompositions<I> {
    inner: Decompositions<I>,
    state: State,
    /// Marks blocked from the current starter, in order.
    buffer: RunBuffer<(char, isize)>,
    composee: Option<(char, isize)>,
    /// Whether the composee can start a composite pair. `None` means not
    /// looked up yet. A composition result defers the lookup until the next
    /// character is a pair second, so most results skip it entirely.
    composee_first: Option<bool>,
    last_ccc: Option<u8>,
}

impl<I: Iterator<Item = char>> Recompositions<I> {
    pub(crate) fn new(inner: Decompositions<I>) -> Self {
        Recompositions {
            inner,
            state: State::Composing,
            buffer: RunBuffer::new(),
            composee: None,
            composee_first: None,
            last_ccc: None,
        }
    }

    #[inline]
    fn set_composee(&mut self, c: char, tag: isize) {
        self.composee = Some((c, tag));
        self.composee_first = None;
    }

    /// Composes the current starter with `c` when the pair is a primary
    /// composite. The composite carries `starter tag + c tag - 1`.
    #[inline]
    fn try_compose(&mut self, starter: (char, isize), c: char, tag: isize) -> bool {
        if !Props::of(c).composes_second() {
            return false;
        }
        let first = *self
            .composee_first
            .get_or_insert_with(|| Props::of(starter.0).composes_first());
        if !first {
            return false;
        }
        let composed = hangul::compose(starter.0, c).or_else(|| props::compose_pair(starter.0, c));
        match composed {
            Some(r) => {
                self.set_composee(r, starter.1 + tag - 1);
                true
            }
            None => false,
        }
    }

    fn block(&mut self, c: char, tag: isize, ccc: u8) {
        self.buffer.push((c, tag));
        self.last_ccc = Some(ccc);
    }

    fn compose_step(&mut self) -> Option<(char, isize)> {
        while let Some((ccc, c, tag)) = self.inner.next_full() {
            let starter = match self.composee {
                Some(k) => k,
                None => {
                    if ccc != 0 {
                        return Some((c, tag));
                    }
                    self.set_composee(c, tag);
                    continue;
                }
            };
            match self.last_ccc {
                None => {
                    if self.try_compose(starter, c, tag) {
                        continue;
                    }
                    if ccc == 0 {
                        self.set_composee(c, tag);
                        return Some(starter);
                    }
                    self.block(c, tag, ccc);
                }
                Some(last) => {
                    if last >= ccc {
                        if ccc == 0 {
                            self.set_composee(c, tag);
                            self.last_ccc = None;
                            self.state = State::Purging(0);
                            return Some(starter);
                        }
                        self.block(c, tag, ccc);
                        continue;
                    }
                    if self.try_compose(starter, c, tag) {
                        continue;
                    }
                    self.block(c, tag, ccc);
                }
            }
        }
        self.state = State::Finished(0);
        self.composee.take()
    }
}

impl<I: Iterator<Item = char>> Iterator for Recompositions<I> {
    type Item = (char, isize);

    #[inline]
    fn next(&mut self) -> Option<(char, isize)> {
        loop {
            match self.state {
                State::Composing => {
                    if let Some(item) = self.compose_step() {
                        return Some(item);
                    }
                }
                State::Purging(i) => {
                    if let Some(&item) = self.buffer.get(i) {
                        self.state = State::Purging(i + 1);
                        return Some(item);
                    }
                    self.buffer.clear();
                    self.state = State::Composing;
                }
                State::Finished(i) => {
                    if let Some(&item) = self.buffer.get(i) {
                        self.state = State::Finished(i + 1);
                        return Some(item);
                    }
                    self.buffer.clear();
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // Composition can shrink the stream. A forwarded inner lower bound
        // can exceed the remaining count, which the Iterator contract forbids.
        (0, None)
    }
}

impl<I: Iterator<Item = char>> FusedIterator for Recompositions<I> {}
