//! Canonical and compatibility decomposition with canonical ordering.

use core::iter::FusedIterator;

use alloc::vec::Vec;

use crate::buffer::{RunBuffer, INLINE};
use crate::props::{self, hangul, Props};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    Canonical,
    Compatible,
}

/// Streaming NFD or NFKD. Yields `(char, isize)`: the output character and its
/// alignment change count. See the crate docs for the tag contract.
///
/// Created by [`UnicodeNormalization::nfd`](crate::UnicodeNormalization::nfd)
/// and [`UnicodeNormalization::nfkd`](crate::UnicodeNormalization::nfkd).
#[derive(Clone, Debug)]
pub struct Decompositions<I> {
    kind: Kind,
    iter: I,
    /// `(ccc, char, tag)`. Items before `pos` were emitted. Items in
    /// `pos..ready` are sorted and waiting. Items from `ready` on are the
    /// pending run, which gets sorted when a starter closes it.
    buffer: RunBuffer<(u8, char, isize)>,
    ready: usize,
    pos: usize,
    done: bool,
}

impl<I: Iterator<Item = char>> Decompositions<I> {
    pub(crate) fn new(iter: I, kind: Kind) -> Self {
        Decompositions {
            kind,
            iter,
            buffer: RunBuffer::new(),
            ready: 0,
            pos: 0,
            done: false,
        }
    }

    /// Stably sorts the pending run by combining class. A run that fits
    /// the inline buffer uses an insertion sort, stable and allocation
    /// free. A longer run uses a counting sort over the 256 possible
    /// classes, stable and O(n).
    fn sort_pending(&mut self) {
        let start = self.ready;
        let run = match self.buffer.as_mut_slice().get_mut(start..) {
            Some(run) => run,
            None => return,
        };
        if run.len() <= INLINE {
            for i in 1..run.len() {
                let mut j = i;
                while j > 0 && run[j - 1].0 > run[j].0 {
                    run.swap(j - 1, j);
                    j -= 1;
                }
            }
        } else {
            counting_sort(run);
        }
        self.ready = self.buffer.len();
    }

    /// Appends through [`RunBuffer::try_push`], so the buffer only takes
    /// new memory after the slow path reclaims the emitted prefix and live
    /// items still need the space. The buffer therefore stays bounded on
    /// streaming input and never spills on stream-safe text. Tags move
    /// with their items, so alignments are unchanged.
    #[inline]
    fn push_item(&mut self, item: (u8, char, isize)) {
        if !self.buffer.try_push(item) {
            self.push_item_slow(item);
        }
    }

    /// Only called while `pos == ready`, which every push path satisfies:
    /// pushes run under `next_full` pulling input, and pulls happen only
    /// once the sorted items are drained.
    #[cold]
    fn push_item_slow(&mut self, item: (u8, char, isize)) {
        if self.pos > 0 {
            self.buffer.compact_front(self.pos);
            self.ready -= self.pos;
            self.pos = 0;
        }
        if !self.buffer.try_push(item) {
            self.buffer.push(item);
        }
    }

    #[inline]
    fn push(&mut self, ccc: u8, c: char, tag: isize) {
        if ccc == 0 {
            if self.ready != self.buffer.len() {
                self.sort_pending();
            }
            if self.pos == self.buffer.len() {
                self.buffer.clear();
                self.pos = 0;
            }
            self.push_item((0, c, tag));
            self.ready = self.buffer.len();
        } else {
            self.push_item((ccc, c, tag));
        }
    }

    /// `next` plus the combining class the buffer already holds, so the
    /// recomposition stage never repeats the property lookup.
    #[inline]
    pub(crate) fn next_full(&mut self) -> Option<(u8, char, isize)> {
        loop {
            if self.pos < self.ready {
                let &item = self.buffer.get(self.pos)?;
                self.pos += 1;
                return Some(item);
            }
            if self.done {
                return None;
            }
            match self.iter.next() {
                Some(c) => self.push_char(c),
                None => {
                    self.done = true;
                    self.sort_pending();
                }
            }
        }
    }

    fn push_sequence(&mut self, seq: &[char]) {
        for (i, &d) in seq.iter().enumerate() {
            let tag = if i == 0 { 0 } else { 1 };
            self.push(Props::of(d).ccc(), d, tag);
        }
    }

    fn push_char(&mut self, c: char) {
        if (c as u32) < 0x80 {
            self.push(0, c, 0);
            return;
        }
        let p = Props::of(c);
        let mapping = match self.kind {
            Kind::Canonical if p.nfd_no() => Some(Kind::Canonical),
            Kind::Compatible if p.nfkd_no() => Some(self.kind),
            _ => None,
        };
        let kind = match mapping {
            Some(kind) => kind,
            None => {
                self.push(p.ccc(), c, 0);
                return;
            }
        };
        if let Some((l, v, t)) = hangul::decompose(c) {
            self.push(0, l, 0);
            self.push(0, v, 1);
            if let Some(t) = t {
                self.push(0, t, 1);
            }
            return;
        }
        let seq = match kind {
            Kind::Compatible if p.has_compat() => props::compat_distinct(c),
            _ => props::canonical(c),
        };
        match seq {
            Some(seq) => self.push_sequence(seq),
            None => self.push(p.ccc(), c, 0),
        }
    }
}

/// Stably sorts `run` by combining class with a counting pass. Out of line so
/// the short-run path stays small. Allocates one scratch per call. Only
/// runs longer than the inline buffer land here, and those have already
/// spilled, so ordering them was never allocation free.
#[cold]
fn counting_sort(run: &mut [(u8, char, isize)]) {
    let mut next = [0usize; 256];
    for item in run.iter() {
        next[item.0 as usize] += 1;
    }
    let mut total = 0;
    for slot in next.iter_mut() {
        let count = *slot;
        *slot = total;
        total += count;
    }
    let mut scratch = Vec::new();
    scratch.resize(run.len(), (0, '\0', 0));
    for &item in run.iter() {
        let slot = &mut next[item.0 as usize];
        scratch[*slot] = item;
        *slot += 1;
    }
    run.copy_from_slice(&scratch);
}

impl<I: Iterator<Item = char>> Iterator for Decompositions<I> {
    type Item = (char, isize);

    #[inline]
    fn next(&mut self) -> Option<(char, isize)> {
        self.next_full().map(|(_, c, tag)| (c, tag))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, _) = self.iter.size_hint();
        // `saturating_add` because a legal inner iterator can report a
        // `usize::MAX` lower bound.
        (lower.saturating_add(self.ready - self.pos), None)
    }
}

impl<I: Iterator<Item = char>> FusedIterator for Decompositions<I> {}
